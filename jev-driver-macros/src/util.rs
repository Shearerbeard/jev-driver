//! Shared helpers for the jev-driver derive macros.

use std::collections::BTreeMap;

use proc_macro2::TokenStream;
use quote::ToTokens;
use quote::format_ident;
use syn::DeriveInput;

/// Parses every `#[jev(key = value)]` attribute into a map. Values are
/// string literals or bare type paths (e.g. `state = TicketState`).
#[allow(
    clippy::wildcard_enum_match_arm,
    reason = "the catch-all arms exist to reject every other expr/lit shape with a clear error"
)]
pub(crate) fn jev_string_attrs(attrs: &[syn::Attribute]) -> syn::Result<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    for attr in attrs {
        if !attr.path().is_ident("jev") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            let Some(key) = meta.path.get_ident() else {
                return Err(meta.error("compound jev attribute keys are not supported"));
            };
            let value: syn::Expr = meta.value()?.parse()?;
            let text = match &value {
                syn::Expr::Lit(lit) => match &lit.lit {
                    syn::Lit::Str(s) => s.value(),
                    other => {
                        return Err(syn::Error::new_spanned(
                            other,
                            "jev attribute values must be string literals or type paths",
                        ));
                    }
                },
                syn::Expr::Path(path) => path.to_token_stream().to_string(),
                other => {
                    return Err(syn::Error::new_spanned(
                        other,
                        "jev attribute values must be string literals or type paths",
                    ));
                }
            };
            out.insert(key.to_string(), text);
            Ok(())
        })?;
    }
    Ok(out)
}

/// Fetches a required attribute or points at the derive that missed it.
pub(crate) fn required<'a>(
    attrs: &'a BTreeMap<String, String>,
    key: &str,
    derive: &str,
    span: &dyn ToTokens,
) -> syn::Result<&'a String> {
    attrs.get(key).ok_or_else(|| {
        syn::Error::new_spanned(span, format!("{derive} requires #[jev({key} = \"...\")]"))
    })
}

/// Rejects generic types with a clear message (only `JevInstructions`
/// supports generics in 0.1.0).
pub(crate) fn reject_generics(input: &DeriveInput, derive: &str) -> syn::Result<()> {
    if input.generics.params.is_empty() {
        return Ok(());
    }
    Err(syn::Error::new_spanned(
        input,
        format!("{derive} does not support generic types in 0.1.0"),
    ))
}

/// Snake-cases a variant ident for the default option key: `Billing` ->
/// `billing`. Override with `#[jev(option = "...")]` when unsure.
pub(crate) fn to_snake_case(ident: &str) -> String {
    let mut out = String::with_capacity(ident.len() + 4);
    for (index, ch) in ident.chars().enumerate() {
        if ch.is_uppercase() {
            if index != 0 {
                out.push('_');
            }
            out.extend(ch.to_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

/// Extracts the `` `backtick` `` reference tokens from a question string,
/// skipping empty pairs (prose artifacts, not references).
pub(crate) fn backtick_refs(question: &str) -> Vec<String> {
    question
        .split('`')
        .skip(1)
        .step_by(2)
        .filter(|token| !token.is_empty())
        .map(|token| token.to_owned())
        .collect()
}

/// Names the generated companion type (`FooCriteria`).
pub(crate) fn companion_ident(name: &proc_macro2::Ident, suffix: &str) -> proc_macro2::Ident {
    format_ident!("{name}{suffix}")
}

/// Parses and runs an expansion, reporting errors as `compile_error!`.
pub(crate) fn guarded(
    input: TokenStream,
    expand: impl FnOnce(&DeriveInput) -> syn::Result<TokenStream>,
) -> TokenStream {
    let parsed: DeriveInput = match syn::parse2(input) {
        Ok(parsed) => parsed,
        Err(err) => return err.to_compile_error(),
    };
    match expand(&parsed) {
        Ok(tokens) => tokens,
        Err(err) => err.to_compile_error(),
    }
}
