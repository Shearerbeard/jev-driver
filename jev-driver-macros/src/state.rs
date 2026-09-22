//! `JevState`: documents a state type in the schema IR.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields};

use crate::util::jev_string_attrs;

pub(crate) fn expand(input: &DeriveInput) -> syn::Result<TokenStream> {
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
    })
}
