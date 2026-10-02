//! `JevScore`: ordered enum variants become rubric levels.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields};

use crate::util::{companion_ident, crate_path, jev_string_attrs, reject_generics, required};

pub(crate) fn expand(input: &DeriveInput) -> syn::Result<TokenStream> {
    reject_generics(input, "JevScore")?;
    let vis = &input.vis;
    let name = &input.ident;
    let attrs = jev_string_attrs(&input.attrs)?;
    let id = required(&attrs, "id", "JevScore", input)?;
    let instructions = required(&attrs, "instructions", "JevScore", input)?;
    let jev = crate_path()?;

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
            quote! { #jev::Instructions::text(#text) }
        });
        quote! { ::std::vec![#(#entries),*] }
    } else {
        quote! { ::std::vec::Vec::new() }
    };
    let source = if all_static {
        quote! { #jev::CriteriaSource::Static }
    } else {
        quote! { #jev::CriteriaSource::Runtime }
    };
    let levels_lit = variants.len() as u8;

    Ok(quote! {
        #[automatically_derived]
        impl #jev::ScoreLevels for #name {
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
        #vis struct #editor(pub ::std::vec::Vec<#jev::Instructions>);

        #[automatically_derived]
        impl #jev::QuestionType for #name {
            const KIND: #jev::QuestionKind = #jev::QuestionKind::Score;
            type Decision = #jev::ScoreDecision<#name>;
            type CriteriaEditor = #editor;

            fn question_id() -> &'static str {
                #id_lit
            }

            fn spec() -> #jev::QuestionSpec {
                #jev::QuestionSpec::Score {
                    instructions: #jev::Instructions::text(#instructions_lit),
                    criteria: #static_vec,
                    criteria_source: #source,
                }
            }

            fn compose_wire(
                editor: ::core::option::Option<&#editor>,
            ) -> #jev::JevResult<#jev::WireQuestion> {
                match editor {
                    ::core::option::Option::Some(ed) => {
                        #jev::__private::score_editor_levels(
                            #id_lit,
                            #levels_lit,
                            &ed.0,
                        )?;
                        ::core::result::Result::Ok(#jev::WireQuestion::Score {
                            instructions: #jev::Instructions::text(#instructions_lit),
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
                answer: &#jev::Answer,
            ) -> #jev::JevResult<Self::Decision> {
                #jev::ScoreDecision::<#name>::parse(id, answer)
            }
        }
    })
}
