//! `JevQuestions`: a struct of questions becomes schema + request builder
//! + criteria customization + typed answers.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields};

use crate::util::{companion_ident, crate_path, jev_string_attrs, reject_generics};

pub(crate) fn expand(input: &DeriveInput) -> syn::Result<TokenStream> {
    reject_generics(input, "JevQuestions")?;
    let vis = &input.vis;
    let name = &input.ident;
    let attrs = jev_string_attrs(&input.attrs)?;
    let state_ty = attrs.get("state");
    let jev = crate_path()?;

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
        None => quote! { #jev::JsonValue },
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
                #jev::__private::validate_state_refs(
                    &questions,
                    <#parsed as #jev::StateKeys>::STATE_KEYS,
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
            pub fn schema() -> #jev::DecisionSchema {
                #jev::DecisionSchema::new(::core::option::Option::None)
                    #(
                        .with_question(
                            <#ftypes as #jev::QuestionType>::question_id(),
                            <#ftypes as #jev::QuestionType>::spec(),
                        )
                    )*
                    #state_chain
            }

            /// Starts a typed request; `state()` accepts exactly the
            /// paired state type.
            pub fn request() -> #jev::RequestBuilder<#name> {
                #jev::RequestBuilder::new()
            }
        }

        #[derive(::core::fmt::Debug, ::core::clone::Clone, ::core::default::Default)]
        #vis struct #customization {
            #(
                pub #fidents: ::core::option::Option<
                    <#ftypes as #jev::QuestionType>::CriteriaEditor,
                >,
            )*
        }

        #[automatically_derived]
        impl #jev::QuestionSet for #name {
            type State = #state_type;
            type Customization = #customization;

            fn schema() -> #jev::DecisionSchema {
                <#name>::schema()
            }

            fn compose(
                customization: &Self::Customization,
            ) -> #jev::JevResult<
                ::std::collections::BTreeMap<String, #jev::WireQuestion>,
            > {
                let mut questions = ::std::collections::BTreeMap::new();
                #(
                    #jev::__private::insert_question(
                        &mut questions,
                        <#ftypes as #jev::QuestionType>::question_id(),
                        <#ftypes as #jev::QuestionType>::compose_wire(
                            customization.#fidents.as_ref(),
                        )?,
                    )?;
                )*
                #validate_chain
                ::core::result::Result::Ok(questions)
            }
        }

        #[derive(::core::fmt::Debug, ::core::clone::Clone)]
        #vis struct #answers {
            #(pub #fidents: <#ftypes as #jev::QuestionType>::Decision,)*
        }

        #[automatically_derived]
        impl #jev::FromAnswers for #answers {
            fn from_answers(
                answers: &#jev::Answers,
            ) -> #jev::JevResult<Self> {
                ::core::result::Result::Ok(Self {
                    #(
                        #fidents: <#ftypes as #jev::QuestionType>::parse(
                            <#ftypes as #jev::QuestionType>::question_id(),
                            answers.get(
                                <#ftypes as #jev::QuestionType>::question_id(),
                            )?,
                        )?,
                    )*
                })
            }
        }
    })
}
