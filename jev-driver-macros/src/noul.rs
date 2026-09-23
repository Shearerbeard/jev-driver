//! `JevNoul`: a unit struct carrying a yes/no question. Instruction
//! backtick references are state paths validated at compose time.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields};

use crate::util::{jev_string_attrs, reject_generics, required};

pub(crate) fn expand(input: &DeriveInput) -> syn::Result<TokenStream> {
    reject_generics(input, "JevNoul")?;
    let name = &input.ident;
    let attrs = jev_string_attrs(&input.attrs)?;
    let id = required(&attrs, "id", "JevNoul", input)?;
    let instructions = required(&attrs, "instructions", "JevNoul", input)?;
    let yes = attrs.get("yes");
    let no = attrs.get("no");
    match (yes, no) {
        (Some(_), None) | (None, Some(_)) => {
            return Err(syn::Error::new_spanned(
                input,
                "JevNoul criteria need both `yes` and `no`, or neither",
            ));
        }
        _ => {}
    }

    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            input,
            "JevNoul requires a unit struct",
        ));
    };
    if !matches!(data.fields, Fields::Unit) {
        return Err(syn::Error::new_spanned(
            input,
            "JevNoul requires a unit struct (no fields)",
        ));
    }

    let id_lit = id.as_str();
    let instructions_lit = instructions.as_str();
    let criteria_expr = match (yes, no) {
        (Some(yes), Some(no)) => quote! {
            ::core::option::Option::Some(::jev_driver::NoulCriteria {
                yes: ::jev_driver::Instructions::text(#yes),
                no: ::jev_driver::Instructions::text(#no),
            })
        },
        _ => quote! { ::core::option::Option::None },
    };

    Ok(quote! {
        #[automatically_derived]
        impl ::jev_driver::QuestionType for #name {
            const KIND: ::jev_driver::QuestionKind = ::jev_driver::QuestionKind::Noul;
            type Decision = ::jev_driver::NoulDecision;
            type CriteriaEditor = ();

            fn question_id() -> &'static str {
                #id_lit
            }

            fn spec() -> ::jev_driver::QuestionSpec {
                ::jev_driver::QuestionSpec::Noul {
                    instructions: ::jev_driver::Instructions::text(#instructions_lit),
                    criteria: #criteria_expr,
                }
            }

            fn parse(
                id: &str,
                answer: &::jev_driver::Answer,
            ) -> ::jev_driver::JevResult<Self::Decision> {
                ::jev_driver::NoulDecision::parse(id, answer)
            }
        }
    })
}
