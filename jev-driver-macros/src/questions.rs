//! `JevQuestions`: a struct of questions becomes schema + request builder
//! + criteria customization + typed answers.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields};

use crate::util::{companion_ident, jev_string_attrs, reject_generics};

pub(crate) fn expand(input: &DeriveInput) -> syn::Result<TokenStream> {
    reject_generics(input, "JevQuestions")?;
    let vis = &input.vis;
    let name = &input.ident;
    let attrs = jev_string_attrs(&input.attrs)?;
    let state_ty = attrs.get("state");

    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            input,
            "JevQuestions requires a struct",
        ));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(syn::Error::new_spanned(
            input,
            "JevQuestions requires named fields",
        ));
    };
    if fields.named.is_empty() {
        return Err(syn::Error::new_spanned(
            input,
            "JevQuestions requires at least one question field",
        ));
    }

    let fidents: Vec<_> = fields
        .named
        .iter()
        .filter_map(|field| field.ident.clone())
        .collect();
    let ftypes: Vec<_> = fields.named.iter().map(|field| &field.ty).collect();

    let answers = companion_ident(name, "Answers");
    let customization = companion_ident(name, "Customization");
    let state_type: proc_macro2::TokenStream = match state_ty {
        Some(ty) => {
            let parsed: syn::Type = syn::parse_str(ty).map_err(|_| {
                syn::Error::new_spanned(input, format!("invalid state type `{ty}`"))
            })?;
            quote! { #parsed }
        }
        None => quote! { ::jev_driver::JsonValue },
    };
    let state_chain = match state_ty {
        Some(ty) => {
            let parsed: syn::Type = syn::parse_str(ty).map_err(|_| {
                syn::Error::new_spanned(input, format!("invalid state type `{ty}`"))
            })?;
            quote! { .with_state(<#parsed>::state_spec()) }
        }
        None => quote! {},
    };
    let validate_chain = match state_ty {
        Some(ty) => {
            let parsed: syn::Type = syn::parse_str(ty).map_err(|_| {
                syn::Error::new_spanned(input, format!("invalid state type `{ty}`"))
            })?;
            quote! {
                ::jev_driver::refs::validate_state_refs(
                    &questions,
                    <#parsed as ::jev_driver::StateKeys>::STATE_KEYS,
                )?;
            }
        }
        None => quote! {},
    };

    Ok(quote! {
        #[automatically_derived]
        impl #name {
            /// The canonical schema IR: export it with `to_json_pretty()`
            /// and diff the artifact in review, DMMF-style.
            pub fn schema() -> ::jev_driver::DecisionSchema {
                ::jev_driver::DecisionSchema::new(::core::option::Option::None)
                    #(
                        .with_question(
                            <#ftypes as ::jev_driver::QuestionType>::question_id(),
                            <#ftypes as ::jev_driver::QuestionType>::spec(),
                        )
                    )*
                    #state_chain
            }

            /// Starts a typed request; `state()` accepts exactly the
            /// paired state type.
            pub fn request() -> ::jev_driver::RequestBuilder<#name> {
                ::jev_driver::RequestBuilder::new()
            }
        }

        #[derive(::core::fmt::Debug, ::core::clone::Clone, ::core::default::Default)]
        #vis struct #customization {
            #(
                pub #fidents: ::core::option::Option<
                    <#ftypes as ::jev_driver::QuestionType>::CriteriaEditor,
                >,
            )*
        }

        #[automatically_derived]
        impl ::jev_driver::QuestionSet for #name {
            type State = #state_type;
            type Customization = #customization;

            fn schema() -> ::jev_driver::DecisionSchema {
                <#name>::schema()
            }

            fn compose(
                customization: &Self::Customization,
            ) -> ::jev_driver::JevResult<
                ::std::collections::BTreeMap<String, ::jev_driver::WireQuestion>,
            > {
                let mut questions = ::std::collections::BTreeMap::new();
                #(
                    if questions
                        .insert(
                            <#ftypes as ::jev_driver::QuestionType>::question_id().to_owned(),
                            <#ftypes as ::jev_driver::QuestionType>::compose_wire(
                                customization.#fidents.as_ref(),
                            )?,
                        )
                        .is_some()
                    {
                        return ::core::result::Result::Err(::jev_driver::JevError::Schema(
                            ::std::format!(
                                "duplicate question id `{}`",
                                <#ftypes as ::jev_driver::QuestionType>::question_id()
                            ),
                        ));
                    }
                )*
                #validate_chain
                ::core::result::Result::Ok(questions)
            }
        }

        #[derive(::core::fmt::Debug, ::core::clone::Clone)]
        #vis struct #answers {
            #(pub #fidents: <#ftypes as ::jev_driver::QuestionType>::Decision,)*
        }

        #[automatically_derived]
        impl ::jev_driver::FromAnswers for #answers {
            fn from_answers(
                answers: &::jev_driver::Answers,
            ) -> ::jev_driver::JevResult<Self> {
                ::core::result::Result::Ok(Self {
                    #(
                        #fidents: <#ftypes as ::jev_driver::QuestionType>::parse(
                            <#ftypes as ::jev_driver::QuestionType>::question_id(),
                            answers.get(
                                <#ftypes as ::jev_driver::QuestionType>::question_id(),
                            )?,
                        )?,
                    )*
                })
            }
        }
    })
}
