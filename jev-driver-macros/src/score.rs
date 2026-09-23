//! `JevScore`: ordered enum variants become rubric levels.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields};

use crate::util::{companion_ident, jev_string_attrs, reject_generics, required};

pub(crate) fn expand(input: &DeriveInput) -> syn::Result<TokenStream> {
    reject_generics(input, "JevScore")?;
    let vis = &input.vis;
    let name = &input.ident;
    let attrs = jev_string_attrs(&input.attrs)?;
    let id = required(&attrs, "id", "JevScore", input)?;
    let instructions = required(&attrs, "instructions", "JevScore", input)?;

    let Data::Enum(data) = &input.data else {
        return Err(syn::Error::new_spanned(input, "JevScore requires an enum"));
    };

    let mut variants = Vec::new();
    for variant in &data.variants {
        if !matches!(variant.fields, Fields::Unit) {
            return Err(syn::Error::new_spanned(
                variant,
                "JevScore variants must be unit variants",
            ));
        }
        let vattrs = jev_string_attrs(&variant.attrs)?;
        variants.push((variant.ident.clone(), vattrs.get("criteria").cloned()));
    }

    let level_count = variants.len();
    if !(2..=10).contains(&level_count) {
        return Err(syn::Error::new_spanned(
            input,
            format!("JevScore needs 2..=10 levels, found {level_count}"),
        ));
    }
    let all_static = variants.iter().all(|(_, c)| c.is_some());
    let none_static = variants.iter().all(|(_, c)| c.is_none());
    if !all_static && !none_static {
        return Err(syn::Error::new_spanned(
            input,
            "JevScore criteria must be present on every variant or on none (runtime-supplied via the editor)",
        ));
    }

    let id_lit = id.as_str();
    let instructions_lit = instructions.as_str();
    let idents: Vec<_> = variants.iter().map(|(ident, _)| ident).collect();
    let levels: Vec<_> = (0..variants.len() as u8).collect();
    let editor = companion_ident(name, "Criteria");

    let static_vec = if all_static {
        let entries = variants.iter().map(|(_, criteria)| {
            let text = criteria.clone().unwrap_or_default();
            quote! { ::jev_driver::Instructions::text(#text) }
        });
        quote! { ::std::vec![#(#entries),*] }
    } else {
        quote! { ::std::vec::Vec::new() }
    };
    let source = if all_static {
        quote! { ::jev_driver::CriteriaSource::Static }
    } else {
        quote! { ::jev_driver::CriteriaSource::Runtime }
    };
    let levels_lit = variants.len() as u8;

    Ok(quote! {
        #[automatically_derived]
        impl ::jev_driver::ScoreLevels for #name {
            const ID: &'static str = #id_lit;
            const LEVELS: u8 = #levels_lit;

            fn from_level(level: u8) -> ::core::option::Option<Self> {
                match level {
                    #(#levels => ::core::option::Option::Some(#name::#idents),)*
                    _ => ::core::option::Option::None,
                }
            }
        }

        #[derive(::core::fmt::Debug, ::core::clone::Clone)]
        #vis struct #editor(pub ::std::vec::Vec<::jev_driver::Instructions>);

        #[automatically_derived]
        impl ::jev_driver::QuestionType for #name {
            const KIND: ::jev_driver::QuestionKind = ::jev_driver::QuestionKind::Score;
            type Decision = ::jev_driver::ScoreDecision<#name>;
            type CriteriaEditor = #editor;

            fn question_id() -> &'static str {
                #id_lit
            }

            fn spec() -> ::jev_driver::QuestionSpec {
                ::jev_driver::QuestionSpec::Score {
                    instructions: ::jev_driver::Instructions::text(#instructions_lit),
                    criteria: #static_vec,
                    criteria_source: #source,
                }
            }

            fn compose_wire(
                editor: ::core::option::Option<&#editor>,
            ) -> ::jev_driver::JevResult<::jev_driver::WireQuestion> {
                match editor {
                    ::core::option::Option::Some(ed) => {
                        ::jev_driver::question::score_editor_levels(
                            #id_lit,
                            #levels_lit,
                            &ed.0,
                        )?;
                        ::core::result::Result::Ok(::jev_driver::WireQuestion::Score {
                            instructions: ::jev_driver::Instructions::text(#instructions_lit),
                            criteria: ed.0.clone(),
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
                ::jev_driver::ScoreDecision::<#name>::parse(id, answer)
            }
        }
    })
}
