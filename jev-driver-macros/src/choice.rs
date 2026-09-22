//! `JevChoice`: enum variants become choice options.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields};

use crate::util::{
    backtick_refs, companion_ident, jev_string_attrs, reject_generics, required, to_snake_case,
};

pub(crate) fn expand(input: &DeriveInput) -> syn::Result<TokenStream> {
    reject_generics(input, "JevChoice")?;
    let vis = &input.vis;
    let name = &input.ident;
    let attrs = jev_string_attrs(&input.attrs)?;
    let id = required(&attrs, "id", "JevChoice", input)?;
    let instructions = required(&attrs, "instructions", "JevChoice", input)?;

    let Data::Enum(data) = &input.data else {
        return Err(syn::Error::new_spanned(input, "JevChoice requires an enum"));
    };

    let mut variants = Vec::new();
    let mut seen_keys = std::collections::BTreeSet::new();
    for variant in &data.variants {
        if !matches!(variant.fields, Fields::Unit) {
            return Err(syn::Error::new_spanned(
                variant,
                "JevChoice variants must be unit variants",
            ));
        }
        let vattrs = jev_string_attrs(&variant.attrs)?;
        let key = vattrs
            .get("option")
            .cloned()
            .unwrap_or_else(|| to_snake_case(&variant.ident.to_string()));
        if !seen_keys.insert(key.clone()) {
            return Err(syn::Error::new_spanned(
                variant,
                format!("JevChoice option key `{key}` is used by more than one variant"),
            ));
        }
        let criteria = vattrs.get("criteria").cloned();
        variants.push((variant.ident.clone(), key, criteria));
    }

    if variants.is_empty() {
        return Err(syn::Error::new_spanned(
            input,
            "JevChoice requires at least one variant",
        ));
    }
    if variants.len() > 255 {
        return Err(syn::Error::new_spanned(
            input,
            "JevChoice supports at most 255 options",
        ));
    }
    let all_static = variants.iter().all(|(_, _, c)| c.is_some());
    let none_static = variants.iter().all(|(_, _, c)| c.is_none());
    if !all_static && !none_static {
        return Err(syn::Error::new_spanned(
            input,
            "JevChoice criteria must be present on every variant or on none (runtime-supplied via the editor)",
        ));
    }
    for token in backtick_refs(instructions) {
        if !token.starts_with("state.") && !variants.iter().any(|(_, key, _)| *key == token) {
            return Err(syn::Error::new_spanned(
                input,
                format!("JevChoice instructions reference `{token}` but no option has that key"),
            ));
        }
    }

    let id_lit = id.as_str();
    let instructions_lit = instructions.as_str();
    let idents: Vec<_> = variants.iter().map(|(ident, _, _)| ident).collect();
    let keys: Vec<_> = variants.iter().map(|(_, key, _)| key.as_str()).collect();
    let field_idents: Vec<_> = variants
        .iter()
        .map(|(ident, _, _)| format_ident!("{}", to_snake_case(&ident.to_string())))
        .collect();
    let editor = companion_ident(name, "Criteria");

    let static_map = if all_static {
        let entries = variants.iter().map(|(_, key, criteria)| {
            let text = criteria.clone().unwrap_or_default();
            quote! {
                (
                    #key.to_owned(),
                    ::core::option::Option::Some(::jev_driver::Instructions::text(#text)),
                )
            }
        });
        quote! { ::std::collections::BTreeMap::from([#(#entries),*]) }
    } else {
        quote! { ::std::collections::BTreeMap::new() }
    };
    let source = if all_static {
        quote! { ::jev_driver::CriteriaSource::Static }
    } else {
        quote! { ::jev_driver::CriteriaSource::Runtime }
    };

    Ok(quote! {
        #[automatically_derived]
        impl ::jev_driver::ChoiceOptions for #name {
            const OPTIONS: &'static [&'static str] = &[#(#keys),*];

            fn from_option(option: &str) -> ::jev_driver::JevResult<Self> {
                match option {
                    #(#keys => ::core::result::Result::Ok(#name::#idents),)*
                    _ => ::core::result::Result::Err(::jev_driver::JevError::UnknownOption {
                        option: ::core::convert::From::from(option),
                        id: <#name as ::jev_driver::QuestionType>::question_id().to_owned(),
                    }),
                }
            }

            fn option_name(&self) -> &'static str {
                match self {
                    #(#name::#idents => #keys,)*
                }
            }
        }

        #[derive(::core::fmt::Debug, ::core::clone::Clone)]
        #vis struct #editor {
            #(pub #field_idents: ::jev_driver::Instructions,)*
        }

        #[automatically_derived]
        impl ::jev_driver::QuestionType for #name {
            const KIND: ::jev_driver::QuestionKind = ::jev_driver::QuestionKind::Choice;
            type Decision = ::jev_driver::ChoiceDecision<#name>;
            type CriteriaEditor = #editor;

            fn question_id() -> &'static str {
                #id_lit
            }

            fn spec() -> ::jev_driver::QuestionSpec {
                ::jev_driver::QuestionSpec::Choice {
                    instructions: ::jev_driver::Instructions::text(#instructions_lit),
                    criteria: #static_map,
                    criteria_source: #source,
                }
            }

            fn compose_wire(
                editor: ::core::option::Option<&#editor>,
            ) -> ::jev_driver::JevResult<::jev_driver::WireQuestion> {
                match editor {
                    ::core::option::Option::Some(ed) => {
                        ::core::result::Result::Ok(::jev_driver::WireQuestion::Choice {
                            instructions: ::jev_driver::Instructions::text(#instructions_lit),
                            criteria: [
                                #(
                                    (
                                        #keys.to_owned(),
                                        ::core::option::Option::Some(ed.#field_idents.clone()),
                                    ),
                                )*
                            ]
                            .into_iter()
                            .collect(),
                        })
                    }
                    ::core::option::Option::None => {
                        Self::spec().into_wire(Self::question_id())
                    }
                }
            }

            fn parse(
                id: &str,
                answer: &::jev_driver::Answer,
            ) -> ::jev_driver::JevResult<Self::Decision> {
                ::jev_driver::ChoiceDecision::<#name>::parse(id, answer)
            }
        }
    })
}
