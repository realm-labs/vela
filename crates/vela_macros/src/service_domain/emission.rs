use proc_macro2::{Ident, TokenStream};
use quote::quote;

use crate::service_domain_input::ServiceField;

pub(super) struct DomainEmission {
    pub(super) marker_uses: Vec<TokenStream>,
    pub(super) registration_entries: Vec<TokenStream>,
    pub(super) schema_entries: Vec<TokenStream>,
    pub(super) generation_fields: Vec<TokenStream>,
    pub(super) defaults_fields: Vec<TokenStream>,
    pub(super) builder_fields: Vec<TokenStream>,
    pub(super) empty_builder_fields: Vec<TokenStream>,
    pub(super) builder_setters: Vec<TokenStream>,
    pub(super) required_defaults: Vec<TokenStream>,
    pub(super) defaults_initializers: Vec<TokenStream>,
    pub(super) initial_generation_fields: Vec<TokenStream>,
    pub(super) rust_snapshot_fields: Vec<TokenStream>,
    pub(super) dispatcher_fields: Vec<TokenStream>,
    pub(super) dispatcher_initializers: Vec<TokenStream>,
    pub(super) dispatcher_rust_branches: Vec<TokenStream>,
    pub(super) dispatcher_async_rust_branches: Vec<TokenStream>,
    pub(super) composed_services: Vec<TokenStream>,
    pub(super) generation_initializers: Vec<TokenStream>,
    pub(super) generation_accessors: Vec<TokenStream>,
    pub(super) root_accessors: Vec<TokenStream>,
}

impl DomainEmission {
    pub(super) fn new(services: &[ServiceField], set_ident: &Ident) -> Self {
        let shared_fields = services
            .iter()
            .map(|service| {
                let field = &service.field;
                let trait_path = service.dispatch_trait_path();
                quote! { #field: ::std::sync::Arc<dyn #trait_path> }
            })
            .collect::<Vec<_>>();
        let default_generation_fields = default_generation_fields(services);

        Self {
            marker_uses: marker_uses(services),
            registration_entries: registration_entries(services),
            schema_entries: schema_entries(services),
            generation_fields: shared_fields.clone(),
            defaults_fields: shared_fields.clone(),
            builder_fields: builder_fields(services),
            empty_builder_fields: empty_builder_fields(services),
            builder_setters: builder_setters(services),
            required_defaults: required_defaults(services, set_ident),
            defaults_initializers: field_names(services),
            initial_generation_fields: default_generation_fields.clone(),
            rust_snapshot_fields: default_generation_fields,
            dispatcher_fields: shared_fields,
            dispatcher_initializers: dispatcher_initializers(services),
            dispatcher_rust_branches: dispatcher_rust_branches(services),
            dispatcher_async_rust_branches: dispatcher_async_rust_branches(services),
            composed_services: composed_services(services),
            generation_initializers: field_names(services),
            generation_accessors: generation_accessors(services),
            root_accessors: root_accessors(services),
        }
    }
}

pub(super) fn schema_factory(
    set_ident: &Ident,
    schema_factory_ident: &Ident,
    schema_entries: &[TokenStream],
) -> TokenStream {
    quote! {
        #[doc(hidden)]
        fn #schema_factory_ident(
            registry: &::vela_engine::type_binding::TypeBindingRegistry,
            patch_effect_ceiling: ::vela_engine::native::EffectSet,
        ) -> ::std::result::Result<
            ::vela_engine::service::ServiceSetSchema,
            ::vela_engine::service::ServiceSchemaError,
        > {
            type __VelaServiceSchemaFactory = fn(
                &::vela_engine::type_binding::TypeBindingRegistry,
                ::vela_engine::native::EffectSet,
            ) -> ::std::result::Result<
                ::vela_engine::service::ServiceSchema,
                ::vela_engine::service::ServiceSchemaError,
            >;
            const __VELA_SERVICE_SCHEMAS: &[
                (&str, __VelaServiceSchemaFactory)
            ] = &[#(#schema_entries),*];

            #[inline(never)]
            fn __vela_push_service_schema(
                services: &mut ::std::vec::Vec<(
                    ::std::string::String,
                    ::vela_engine::service::ServiceSchema,
                )>,
                name: &'static str,
                schema: fn(
                    &::vela_engine::type_binding::TypeBindingRegistry,
                    ::vela_engine::native::EffectSet,
                ) -> ::std::result::Result<
                    ::vela_engine::service::ServiceSchema,
                    ::vela_engine::service::ServiceSchemaError,
                >,
                registry: &::vela_engine::type_binding::TypeBindingRegistry,
                patch_effect_ceiling: ::vela_engine::native::EffectSet,
            ) -> ::std::result::Result<
                (),
                ::vela_engine::service::ServiceSchemaError,
            > {
                let schema = schema(registry, patch_effect_ceiling)?;
                services.push((name.to_owned(), schema));
                Ok(())
            }

            let path = ::std::concat!(
                ::std::module_path!(),
                "::",
                ::std::stringify!(#set_ident),
            );
            let id = ::vela_common::ServiceSetId::new(
                u128::from(::vela_common::stable_id("vela_service_domain", "", path)),
            );
            let mut services = ::std::vec::Vec::with_capacity(
                __VELA_SERVICE_SCHEMAS.len()
            );
            for &(name, schema) in __VELA_SERVICE_SCHEMAS {
                __vela_push_service_schema(
                    &mut services,
                    name,
                    schema,
                    registry,
                    patch_effect_ceiling,
                )?;
            }
            ::vela_engine::service::ServiceSetSchema::new_named(
                id,
                path,
                services,
                registry,
                patch_effect_ceiling,
            )
        }
    }
}

fn marker_uses(services: &[ServiceField]) -> Vec<TokenStream> {
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

fn registration_entries(services: &[ServiceField]) -> Vec<TokenStream> {
    services
        .iter()
        .map(|service| {
            let function = service.registration_path();
            quote! { #function }
        })
        .collect()
}

fn schema_entries(services: &[ServiceField]) -> Vec<TokenStream> {
    services
        .iter()
        .map(|service| {
            let field = &service.field;
            let function = service.schema_path();
            quote! {
                (::std::stringify!(#field), #function)
            }
        })
        .collect()
}

fn default_generation_fields(services: &[ServiceField]) -> Vec<TokenStream> {
    services
        .iter()
        .map(|service| {
            let field = &service.field;
            quote! { #field: ::std::sync::Arc::clone(&defaults.#field) }
        })
        .collect()
}

fn empty_builder_fields(services: &[ServiceField]) -> Vec<TokenStream> {
    services
        .iter()
        .map(|service| {
            let field = &service.field;
            quote! { #field: None }
        })
        .collect()
}

fn builder_fields(services: &[ServiceField]) -> Vec<TokenStream> {
    services
        .iter()
        .map(|service| {
            let field = &service.field;
            let trait_path = service.dispatch_trait_path();
            quote! { #field: ::std::option::Option<::std::sync::Arc<dyn #trait_path>> }
        })
        .collect()
}

fn builder_setters(services: &[ServiceField]) -> Vec<TokenStream> {
    services
        .iter()
        .map(|service| {
            let field = &service.field;
            let trait_path = &service.trait_path;
            let dispatch_trait_path = service.dispatch_trait_path();
            quote! {
                #[must_use]
                pub fn #field<__VelaDefault>(mut self, implementation: __VelaDefault) -> Self
                where
                    __VelaDefault:
                        #trait_path
                        + ::std::marker::Send
                        + ::std::marker::Sync
                        + 'static,
                {
                    let implementation: ::std::sync::Arc<dyn #dispatch_trait_path> =
                        ::std::sync::Arc::new(implementation);
                    self.state.#field = Some(implementation);
                    self
                }
            }
        })
        .collect()
}

fn required_defaults(services: &[ServiceField], set_ident: &Ident) -> Vec<TokenStream> {
    services
        .iter()
        .map(|service| {
            let field = &service.field;
            quote! {
                let #field = self.state.#field.take().ok_or(
                    ::vela_engine::service::ServiceDomainBuildError::MissingDefault {
                        domain: ::std::stringify!(#set_ident),
                        service: ::std::stringify!(#field),
                    },
                )?;
            }
        })
        .collect()
}

fn field_names(services: &[ServiceField]) -> Vec<TokenStream> {
    services
        .iter()
        .map(|service| {
            let field = &service.field;
            quote! { #field }
        })
        .collect()
}

fn dispatcher_initializers(services: &[ServiceField]) -> Vec<TokenStream> {
    services
        .iter()
        .map(|service| {
            let field = &service.field;
            quote! { #field: ::std::sync::Arc::clone(&defaults.#field) }
        })
        .collect()
}

fn dispatcher_rust_branches(services: &[ServiceField]) -> Vec<TokenStream> {
    services
        .iter()
        .map(|service| {
            let field = &service.field;
            let service_id = service.service_id_path();
            let dispatch = service.rust_dispatch_path();
            quote! {
                if __vela_target.service == #service_id() {
                    return #dispatch(
                        self.#field.as_ref(),
                        __vela_target.method,
                        __vela_args,
                        __vela_context,
                    );
                }
            }
        })
        .collect()
}

fn dispatcher_async_rust_branches(services: &[ServiceField]) -> Vec<TokenStream> {
    services
        .iter()
        .map(|service| {
            let field = &service.field;
            let service_id = service.service_id_path();
            let dispatch = service.rust_async_dispatch_path();
            quote! {
                if __vela_target.service == #service_id() {
                    return #dispatch(
                        self.#field.as_ref(),
                        __vela_target.method,
                        __vela_args,
                        __vela_leases,
                    );
                }
            }
        })
        .collect()
}

fn composed_services(services: &[ServiceField]) -> Vec<TokenStream> {
    services
        .iter()
        .map(|service| {
            let field = &service.field;
            let trait_path = service.dispatch_trait_path();
            let composition = service.composition_path();
            quote! {
                let #field: ::std::sync::Arc<dyn #trait_path> = #composition(
                    ::std::sync::Arc::clone(&defaults.#field),
                    __vela_execution.clone(),
                    &selections,
                );
            }
        })
        .collect()
}

fn generation_accessors(services: &[ServiceField]) -> Vec<TokenStream> {
    services
        .iter()
        .map(|service| {
            let field = &service.field;
            let trait_path = service.dispatch_trait_path();
            quote! {
                #[must_use]
                pub fn #field(&self) -> &(dyn #trait_path + 'static) {
                    self.#field.as_ref()
                }
            }
        })
        .collect()
}

fn root_accessors(services: &[ServiceField]) -> Vec<TokenStream> {
    services
        .iter()
        .map(|service| {
            let field = &service.field;
            let trait_path = service.dispatch_trait_path();
            quote! {
                #[must_use]
                pub fn #field(&self) -> &(dyn #trait_path + 'static) {
                    self.root.services().#field()
                }
            }
        })
        .collect()
}
