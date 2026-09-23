//! `JevState`: documents a state type in the schema IR and publishes its
//! serialized keys for compose-time reference validation.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields};

use crate::serde_names;
use crate::util::{jev_string_attrs, reject_generics};

pub(crate) fn expand(input: &DeriveInput) -> syn::Result<TokenStream> {
    reject_generics(input, "JevState")?;
    let name = &input.ident;
    let attrs = jev_string_attrs(&input.attrs)?;
    let describe = attrs.get("describe");

    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(input, "JevState requires a struct"));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(syn::Error::new_spanned(
            input,
            "JevState requires named fields",
        ));
    };

    let described: Vec<_> = fields
        .named
        .iter()
        .filter_map(|field| {
            let ident = field.ident.as_ref()?;
            let fattrs = jev_string_attrs(&field.attrs).ok()?;
            fattrs
                .get("describe")
                .map(|text| (ident.to_string(), text.clone()))
        })
        .collect();

    let resolved = serde_names::resolve_fields(&input.attrs, fields.named.iter());
    let keys_expr = match serde_names::serialized_keys(&resolved) {
        Some(keys) => {
            let literals = keys.iter().map(|k| k.as_str());
            quote! { ::core::option::Option::Some(&[#(#literals),*]) }
        }
        // Opaque shape (flatten / conditional skip / custom serialize):
        // root checking is exempt rather than wrong.
        None => quote! { ::core::option::Option::None },
    };

    let describe_expr = match describe {
        Some(text) => quote! { ::core::option::Option::Some(#text.to_owned()) },
        None => quote! { ::core::option::Option::None },
    };
    let fields_expr = if described.is_empty() {
        quote! { ::std::collections::BTreeMap::new() }
    } else {
        let entries = described
            .iter()
            .map(|(ident, text)| quote! { (#ident.to_owned(), #text.to_owned()) });
        quote! { ::std::collections::BTreeMap::from([#(#entries),*]) }
    };

    Ok(quote! {
        #[automatically_derived]
        impl #name {
            /// The state documentation contributed to the schema artifact.
            pub fn state_spec() -> ::jev_driver::StateSpec {
                ::jev_driver::StateSpec {
                    describe: #describe_expr,
                    fields: #fields_expr,
                }
            }
        }

        #[automatically_derived]
        impl ::jev_driver::StateKeys for #name {
            const STATE_KEYS: ::core::option::Option<&'static [&'static str]> = #keys_expr;
        }
    })
}
