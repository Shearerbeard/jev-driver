//! `JevInstructions`: struct-to-string question contracts with
//! compile-time backtick validation against serde's serialized keys.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields};

use crate::serde_names::{self, KeyOutcome};
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

    // The model sees the serialized payload, so references are checked
    // against serde's keys (rename / rename_all), never Rust idents.
    let resolved = serde_names::resolve_fields(&input.attrs, fields.named.iter());
    let serialized = serde_names::serialized_keys(&resolved);
    for token in backtick_refs(question) {
        if let Some(field) = resolved.iter().find(|f| f.matches(&token)) {
            if let KeyOutcome::NeverSerialized = field.outcome {
                return Err(syn::Error::new_spanned(
                    input,
                    format!(
                        "JevInstructions question references `{token}` but field `{}` is skipped by serde and never serialized",
                        field.ident
                    ),
                ));
            }
        }
        if let Some(available) = &serialized {
            if !available.iter().any(|key| key == &token) {
                let rename_hint = resolved
                    .iter()
                    .find(|f| f.ident == token)
                    .map(|f| match &f.outcome {
                        KeyOutcome::Key(k) => {
                            format!(" (field `{}` serializes as `{k}`)", f.ident)
                        }
                        KeyOutcome::NeverSerialized | KeyOutcome::Opaque => String::new(),
                    })
                    .unwrap_or_default();
                return Err(syn::Error::new_spanned(
                    input,
                    format!(
                        "JevInstructions question references `{token}` but the serialized data carries keys [{}]{}",
                        available.join(", "),
                        rename_hint,
                    ),
                ));
            }
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
