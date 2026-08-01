use proc_macro2::TokenStream;
use quote::quote;

use crate::service_domain_input::ServiceField;

pub(super) fn marker_uses(services: &[ServiceField]) -> Vec<TokenStream> {
    services
        .iter()
        .map(|service| {
            let marker = &service.marker;
            quote! {
                const _: usize = ::core::mem::size_of::<#marker<()>>();
            }
        })
        .collect()
}

pub(super) fn register_calls(services: &[ServiceField]) -> Vec<TokenStream> {
    services
        .iter()
        .map(|service| {
            let function = service.registration_path();
            quote! {
                let builder = #function(builder);
            }
        })
        .collect()
}

pub(super) fn schema_calls(services: &[ServiceField]) -> Vec<TokenStream> {
    services
        .iter()
        .map(|service| {
            let field = &service.field;
            let function = service.schema_path();
            quote! {
                (
                    ::std::stringify!(#field).to_owned(),
                    #function(registry, patch_effect_ceiling)?,
                )
            }
        })
        .collect()
}

pub(super) fn default_generation_fields(services: &[ServiceField]) -> Vec<TokenStream> {
    services
        .iter()
        .map(|service| {
            let field = &service.field;
            quote! { #field: ::std::sync::Arc::clone(&defaults.#field) }
        })
        .collect()
}

pub(super) fn empty_builder_fields(services: &[ServiceField]) -> Vec<TokenStream> {
    services
        .iter()
        .map(|service| {
            let field = &service.field;
            quote! { #field: None }
        })
        .collect()
}
