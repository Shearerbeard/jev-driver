//! `JevInstructions`: struct-to-string question contracts with
//! compile-time backtick validation.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields};

use crate::util::{backtick_refs, jev_string_attrs, required};

pub(crate) fn expand(input: &DeriveInput) -> syn::Result<TokenStream> {
    let name = &input.ident;
    let attrs = jev_string_attrs(&input.attrs)?;
    let question = required(&attrs, "question", "JevInstructions", input)?;

    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            input,
            "JevInstructions requires a struct",
        ));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(syn::Error::new_spanned(
            input,
            "JevInstructions requires named fields",
        ));
    };

    let field_names: Vec<String> = fields
        .named
        .iter()
        .filter_map(|field| {
            let ident = field.ident.as_ref()?;
            Some(ident.to_string())
        })
        .collect();
    for token in backtick_refs(question) {
        if token.starts_with("state.") {
            continue;
        }
        if !field_names.contains(&token) {
            return Err(syn::Error::new_spanned(
                input,
                format!(
                    "JevInstructions question references `{token}` but the struct has no such field"
                ),
            ));
        }
    }

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let question_lit = question.as_str();

    Ok(quote! {
        #[automatically_derived]
        impl #impl_generics #name #ty_generics #where_clause {
            /// The question text; backtick references are compile-checked
            /// against this struct's fields.
            pub const QUESTION: &'static str = #question_lit;

            /// Serializes into the API's structured `instructions` form:
            /// this struct's fields become backtick-referenceable data
            /// alongside the embedded question.
            pub fn to_instructions(&self) -> ::jev_driver::JevResult<::jev_driver::Instructions> {
                ::jev_driver::Instructions::from_question(Self::QUESTION, self)
            }
        }
    })
}
