use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Attribute, Fields, ItemStruct, Path, Result, Type, TypeParamBound, Visibility, parse::Parser,
    parse_quote, parse2,
};

use crate::service::{
    composition_function_ident, dispatch_module_ident, registration_function_ident,
    rust_async_dispatch_function_ident, rust_dispatch_function_ident, schema_function_ident,
    service_id_function_ident,
};
use crate::signature::reject_generic_signature;

pub(crate) fn expand(attr: TokenStream, input: TokenStream) -> TokenStream {
    match expand_result(attr, input) {
        Ok(tokens) => tokens,
        Err(error) => error.to_compile_error(),
    }
}

fn expand_result(attr: TokenStream, input: TokenStream) -> Result<TokenStream> {
    let item = parse2::<ItemStruct>(input)?;
    validate_struct(&item)?;
    let context = parse_context(attr)?;
    let Fields::Named(fields) = &item.fields else {
        unreachable!("validated service set has named fields");
    };
    let services = fields
        .named
        .iter()
        .map(parse_service_field)
        .collect::<Result<Vec<_>>>()?;
    if services.is_empty() {
        return Err(syn::Error::new_spanned(
            &item,
            "#[vela_macros::service_set] requires at least one service field",
        ));
    }

    let set_ident = &item.ident;
    let generation_ident = format_ident!("{set_ident}Generation");
    let root_ident = format_ident!("{set_ident}Root");
    let candidate_ident = format_ident!("{set_ident}Candidate");
    let rollback_ident = format_ident!("{set_ident}Rollback");
    let dispatcher_ident = format_ident!("__VelaServiceDispatcher{set_ident}");
    let register_calls = services.iter().map(|service| {
        let function = service.registration_path();
        quote! {
            let builder = #function(builder);
        }
    });
    let schema_calls = services
        .iter()
        .map(|service| {
            let field = &service.field;
            let function = service.schema_path();
            quote! {
                (
                    ::std::stringify!(#field).to_owned(),
                    #function(registry)?,
                )
            }
        })
        .collect::<Vec<_>>();
    let generation_fields = services.iter().map(|service| {
        let field = &service.field;
        let trait_path = service.dispatch_trait_path();
        quote! {
            #field: ::std::sync::Arc<dyn #trait_path>
        }
    });
    let composed_defaults = services.iter().map(|service| {
        let field = &service.field;
        let trait_path = service.dispatch_trait_path();
        let default = &service.default;
        let default_ident = format_ident!("__vela_default_{field}");
        quote! {
            let #default_ident: ::std::sync::Arc<dyn #trait_path> =
                ::std::sync::Arc::new(#default);
        }
    });
    let dispatcher_fields = services.iter().map(|service| {
        let field = &service.field;
        let trait_path = service.dispatch_trait_path();
        quote! {
            #field: ::std::sync::Arc<dyn #trait_path>
        }
    });
    let dispatcher_initializers = services.iter().map(|service| {
        let field = &service.field;
        let default_ident = format_ident!("__vela_default_{field}");
        quote! {
            #field: ::std::sync::Arc::clone(&#default_ident)
        }
    });
    let dispatcher_rust_branches = services.iter().map(|service| {
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
    });
    let dispatcher_async_rust_branches = services.iter().map(|service| {
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
    });
    let composed_services = services.iter().map(|service| {
        let field = &service.field;
        let trait_path = service.dispatch_trait_path();
        let composition = service.composition_path();
        let default_ident = format_ident!("__vela_default_{field}");
        quote! {
            let #field: ::std::sync::Arc<dyn #trait_path> = #composition(
                ::std::sync::Arc::clone(&#default_ident),
                runtime,
                options.clone(),
                ::std::sync::Arc::clone(&__vela_dispatcher),
                &selections,
            );
        }
    });
    let generation_arguments = services.iter().map(|service| {
        let field = &service.field;
        let trait_path = service.dispatch_trait_path();
        quote! {
            #field: ::std::sync::Arc<dyn #trait_path>
        }
    });
    let generation_initializers = services
        .iter()
        .map(|service| &service.field)
        .collect::<Vec<_>>();
    let default_initializers = services.iter().map(|service| {
        let field = &service.field;
        let default = &service.default;
        quote! {
            #field: ::std::sync::Arc::new(#default)
        }
    });
    let generation_accessors = services.iter().map(|service| {
        let field = &service.field;
        let trait_path = service.dispatch_trait_path();
        quote! {
            #[must_use]
            pub fn #field(&self) -> &(dyn #trait_path + 'static) {
                self.#field.as_ref()
            }
        }
    });
    let root_accessors = services.iter().map(|service| {
        let field = &service.field;
        let trait_path = service.dispatch_trait_path();
        quote! {
            #[must_use]
            pub fn #field(&self) -> &(dyn #trait_path + 'static) {
                self.root.services().#field()
            }
        }
    });
    let doc_attrs = item
        .attrs
        .iter()
        .filter(|attribute| attribute.path().is_ident("doc"));
    let vis = &item.vis;
    let schema_factory_ident = format_ident!("__vela_service_set_schema_{set_ident}");

    Ok(quote! {
        #[doc(hidden)]
        fn #schema_factory_ident(
            registry: &::vela_engine::type_binding::TypeBindingRegistry,
        ) -> ::std::result::Result<
            ::vela_engine::service::ServiceSetSchema,
            ::vela_engine::service::ServiceSchemaError,
        > {
            let path = ::std::concat!(
                ::std::module_path!(),
                "::",
                ::std::stringify!(#set_ident),
            );
            let id = ::vela_common::ServiceSetId::new(
                u128::from(::vela_common::stable_id("vela_service_set", "", path)),
            );
            ::vela_engine::service::ServiceSetSchema::new_named(
                id,
                path,
                vec![#(#schema_calls),*],
                registry,
            )
        }

        #(#doc_attrs)*
        #vis struct #generation_ident {
            #(#generation_fields,)*
            selections: ::std::option::Option<
                ::vela_engine::service::ServiceSelectionTable<
                    ::vela_engine::service::LinkedVelaServiceMethod
                >
            >,
            artifact: ::std::option::Option<
                ::std::sync::Arc<::vela_engine::service::LinkedArtifact>
            >,
        }

        impl #generation_ident {
            #[must_use]
            pub fn new(#(#generation_arguments),*) -> Self {
                Self {
                    #(#generation_initializers,)*
                    selections: None,
                    artifact: None,
                }
            }

            #[must_use]
            pub fn defaults() -> Self {
                Self {
                    #(#default_initializers,)*
                    selections: None,
                    artifact: None,
                }
            }

            fn __vela_composed(
                runtime: ::vela_engine::service::ServiceRuntimeBinding,
                options: ::vela_engine::runtime::CallOptions,
                selections: ::vela_engine::service::ServiceSelectionTable<
                    ::vela_engine::service::LinkedVelaServiceMethod
                >,
                artifact: ::std::option::Option<
                    ::std::sync::Arc<::vela_engine::service::LinkedArtifact>
                >,
            ) -> Self {
                #(#composed_defaults)*
                let __vela_dispatcher: ::std::sync::Arc<
                    dyn ::vela_engine::service::ServiceCallDispatcher
                > = ::std::sync::Arc::new(#dispatcher_ident {
                    #(#dispatcher_initializers,)*
                    selections: selections.clone(),
                });
                #(#composed_services)*
                Self {
                    #(#generation_initializers,)*
                    selections: Some(selections),
                    artifact,
                }
            }

            #[must_use]
            pub fn selections(
                &self,
            ) -> ::std::option::Option<
                &::vela_engine::service::ServiceSelectionTable<
                    ::vela_engine::service::LinkedVelaServiceMethod
                >
            > {
                self.selections.as_ref()
            }

            #[must_use]
            pub fn artifact_checksum(
                &self,
            ) -> ::std::option::Option<::vela_engine::service::ArtifactChecksum> {
                self.artifact.as_ref().map(|artifact| artifact.checksum())
            }

            #[must_use]
            pub fn artifact(
                &self,
            ) -> ::std::option::Option<
                &::std::sync::Arc<::vela_engine::service::LinkedArtifact>
            > {
                self.artifact.as_ref()
            }

            #(#generation_accessors)*
        }

        struct #dispatcher_ident {
            #(#dispatcher_fields,)*
            selections: ::vela_engine::service::ServiceSelectionTable<
                ::vela_engine::service::LinkedVelaServiceMethod
            >,
        }

        impl #dispatcher_ident {
            fn __vela_dispatch_rust(
                &self,
                __vela_target: ::vela_engine::service::ServiceCallTarget,
                __vela_args: &[::vela_vm::owned_value::OwnedValue],
                __vela_context: &mut
                    ::vela_engine::context::NativeCallContext<'_, '_>,
            ) -> ::vela_vm::error::VmResult<
                ::vela_vm::owned_value::OwnedValue
            > {
                #(#dispatcher_rust_branches)*
                Err(::vela_vm::error::VmError::new(
                    ::vela_vm::error::VmErrorKind::UnknownMethod {
                        method: ::std::format!(
                            "service {} method {}",
                            __vela_target.service.get(),
                            __vela_target.method.get(),
                        ),
                    },
                ))
            }

            fn __vela_dispatch_rust_async<'__vela_call, '__vela_lease>(
                &'__vela_call self,
                __vela_target: ::vela_engine::service::ServiceCallTarget,
                __vela_args:
                    &'__vela_call [::vela_vm::owned_value::OwnedValue],
                __vela_leases: &'__vela_call mut [
                    ::vela_host::lease::ErasedHostLease<'__vela_lease>
                ],
            ) -> ::vela_engine::service::ServiceFuture<
                '__vela_call,
                ::vela_vm::error::VmResult<
                    ::vela_vm::owned_value::OwnedValue
                >,
            >
            where
                '__vela_lease: '__vela_call,
            {
                #(#dispatcher_async_rust_branches)*
                ::std::boxed::Box::pin(async move {
                    Err(::vela_vm::error::VmError::new(
                        ::vela_vm::error::VmErrorKind::UnknownMethod {
                            method: ::std::format!(
                                "service {} method {}",
                                __vela_target.service.get(),
                                __vela_target.method.get(),
                            ),
                        },
                    ))
                })
            }
        }

        impl ::vela_engine::service::ServiceCallDispatcher for #dispatcher_ident {
            fn dispatch(
                &self,
                __vela_target: ::vela_engine::service::ServiceCallTarget,
                __vela_args: &[::vela_vm::owned_value::OwnedValue],
                __vela_context: &mut
                    ::vela_engine::context::NativeCallContext<'_, '_>,
            ) -> ::vela_vm::error::VmResult<
                ::vela_vm::owned_value::OwnedValue
            > {
                match __vela_target.mode {
                    ::vela_common::ServiceCallMode::Base => self.__vela_dispatch_rust(
                        __vela_target,
                        __vela_args,
                        __vela_context,
                    ),
                    ::vela_common::ServiceCallMode::Pinned => {
                        match self.selections.get(
                            __vela_target.service,
                            __vela_target.method,
                        ) {
                            Some(
                                ::vela_engine::service::ServiceMethodSelection::RustDefault
                            ) => self.__vela_dispatch_rust(
                                __vela_target,
                                __vela_args,
                                __vela_context,
                            ),
                            Some(
                                ::vela_engine::service::ServiceMethodSelection::Vela(
                                    __vela_method,
                                )
                            ) => __vela_method.call_in_context(
                                __vela_context,
                                __vela_args,
                            ),
                            None => Err(::vela_vm::error::VmError::new(
                                ::vela_vm::error::VmErrorKind::UnknownMethod {
                                    method: ::std::format!(
                                        "service {} method {}",
                                        __vela_target.service.get(),
                                        __vela_target.method.get(),
                                    ),
                                },
                            )),
                        }
                    }
                }
            }

            fn dispatch_async<'__vela_call, '__vela_host, '__vela_lease>(
                &'__vela_call self,
                __vela_target: ::vela_engine::service::ServiceCallTarget,
                __vela_args:
                    &'__vela_call [::vela_vm::owned_value::OwnedValue],
                __vela_leases: &'__vela_call mut [
                    ::vela_host::lease::ErasedHostLease<'__vela_lease>
                ],
                __vela_context: &'__vela_call mut
                    ::vela_engine::context::NativeCallContext<
                        '_,
                        '__vela_host
                    >,
            ) -> ::vela_engine::service::ServiceFuture<
                '__vela_call,
                ::vela_vm::error::VmResult<
                    ::vela_vm::owned_value::OwnedValue
                >,
            >
            where
                '__vela_lease: '__vela_call,
            {
                match __vela_target.mode {
                    ::vela_common::ServiceCallMode::Base => {
                        self.__vela_dispatch_rust_async(
                            __vela_target,
                            __vela_args,
                            __vela_leases,
                        )
                    }
                    ::vela_common::ServiceCallMode::Pinned => {
                        match self.selections.get(
                            __vela_target.service,
                            __vela_target.method,
                        ) {
                            Some(
                                ::vela_engine::service::ServiceMethodSelection::RustDefault
                            ) => self.__vela_dispatch_rust_async(
                                __vela_target,
                                __vela_args,
                                __vela_leases,
                            ),
                            Some(
                                ::vela_engine::service::ServiceMethodSelection::Vela(
                                    __vela_method,
                                )
                            ) => __vela_method.call_in_context_async(
                                __vela_context,
                                __vela_args,
                            ),
                            None => ::std::boxed::Box::pin(async move {
                                Err(::vela_vm::error::VmError::new(
                                    ::vela_vm::error::VmErrorKind::UnknownMethod {
                                        method: ::std::format!(
                                            "service {} method {}",
                                            __vela_target.service.get(),
                                            __vela_target.method.get(),
                                        ),
                                    },
                                ))
                            }),
                        }
                    }
                }
            }
        }

        #vis struct #set_ident {
            controller: ::vela_engine::service::ServiceController<#generation_ident>,
            schema: ::vela_engine::service::ServiceSetSchema,
            _context: ::std::marker::PhantomData<fn(&mut #context)>,
        }

        impl #set_ident {
            #[must_use]
            pub fn register(
                builder: ::vela_engine::builder::EngineBuilder,
            ) -> ::vela_engine::builder::EngineBuilder {
                let builder = builder;
                #(#register_calls)*
                builder.register_service_set_schema(#schema_factory_ident)
            }

            pub fn new(
                registry: &::vela_engine::type_binding::TypeBindingRegistry,
            ) -> ::std::result::Result<
                Self,
                ::vela_engine::service::ServiceSchemaError,
            > {
                let schema = #schema_factory_ident(registry)?;
                let id = schema.id();
                Ok(Self {
                    controller: ::vela_engine::service::ServiceController::new(
                        id,
                        #generation_ident::defaults(),
                    ),
                    schema,
                    _context: ::std::marker::PhantomData,
                })
            }

            #[must_use]
            pub fn schema(&self) -> &::vela_engine::service::ServiceSetSchema {
                &self.schema
            }

            #[must_use]
            pub fn pin(&self) -> #root_ident {
                #root_ident {
                    root: self.controller.pin(),
                    _context: ::std::marker::PhantomData,
                }
            }

            pub fn stage_rust(
                &self,
                base: &#root_ident,
                generation: #generation_ident,
            ) -> ::std::result::Result<
                #candidate_ident,
                ::vela_engine::service::ServicePublicationError,
            > {
                self.controller
                    .stage(&base.root, generation)
                    .map(|candidate| #candidate_ident { candidate })
            }

            pub fn stage_snapshot(
                &self,
                base: &#root_ident,
                update: ::vela_engine::service::LinkedServiceSourceManifest,
                runtime: ::vela_engine::service::ServiceRuntimeBinding,
                options: ::vela_engine::runtime::CallOptions,
            ) -> ::std::result::Result<
                #candidate_ident,
                ::vela_engine::service::ServiceStagingError,
            > {
                if !runtime.matches::<#context>() {
                    return Err(
                        ::vela_engine::service::ServiceStagingError::ContextTypeMismatch {
                            expected: ::core::any::type_name::<#context>(),
                            actual: runtime.context_name(),
                        }
                    );
                }
                let artifact = update.artifact().cloned();
                let selections = update.into_snapshot(&self.schema)?;
                let generation = #generation_ident::__vela_composed(
                    runtime,
                    options,
                    selections,
                    artifact,
                );
                self.controller
                    .stage(&base.root, generation)
                    .map(|candidate| #candidate_ident { candidate })
                    .map_err(::vela_engine::service::ServiceStagingError::from)
            }

            pub fn stage_delta(
                &self,
                base: &#root_ident,
                update: ::vela_engine::service::LinkedServiceSourceManifest,
                runtime: ::vela_engine::service::ServiceRuntimeBinding,
                options: ::vela_engine::runtime::CallOptions,
            ) -> ::std::result::Result<
                #candidate_ident,
                ::vela_engine::service::ServiceStagingError,
            > {
                if !runtime.matches::<#context>() {
                    return Err(
                        ::vela_engine::service::ServiceStagingError::ContextTypeMismatch {
                            expected: ::core::any::type_name::<#context>(),
                            actual: runtime.context_name(),
                        }
                    );
                }
                let base_selections = match base.selections() {
                    Some(selections) => selections.clone(),
                    None => ::vela_engine::service::ServiceSelectionTable::snapshot(
                        &self.schema,
                        ::core::iter::empty::<
                            ::vela_engine::service::ServiceMethodUpdate<
                                ::vela_engine::service::LinkedVelaServiceMethod
                            >
                        >(),
                    )?,
                };
                let artifact = update
                    .artifact()
                    .cloned()
                    .or_else(|| base.artifact().cloned());
                let selections = update.into_delta(
                    &self.schema,
                    base.generation_id(),
                    base.generation_id(),
                    &base_selections,
                )?;
                let generation = #generation_ident::__vela_composed(
                    runtime,
                    options,
                    selections,
                    artifact,
                );
                self.controller
                    .stage(&base.root, generation)
                    .map(|candidate| #candidate_ident { candidate })
                    .map_err(::vela_engine::service::ServiceStagingError::from)
            }

            #[must_use]
            pub fn dry_run_bundle(
                &self,
                base: &#root_ident,
                bundle: &::vela_engine::service::ServiceUpdateBundle,
            ) -> ::vela_engine::service::ServiceDryRunReport {
                let base_selections = match base.selections() {
                    Some(selections) => selections.clone(),
                    None => ::vela_engine::service::ServiceSelectionTable::snapshot(
                        &self.schema,
                        ::core::iter::empty::<
                            ::vela_engine::service::ServiceMethodUpdate<
                                ::vela_engine::service::LinkedVelaServiceMethod
                            >
                        >(),
                    ).expect("generated Rust defaults match their service schema"),
                };
                bundle.dry_run(
                    &self.schema,
                    base.generation_id(),
                    base.artifact_checksum(),
                    &base_selections,
                )
            }

            pub fn stage_bundle(
                &self,
                base: &#root_ident,
                bundle: ::vela_engine::service::ServiceUpdateBundle,
                runtime: ::vela_engine::service::ServiceRuntimeBinding,
                options: ::vela_engine::runtime::CallOptions,
            ) -> ::std::result::Result<
                #candidate_ident,
                ::vela_engine::service::ServiceStagingError,
            > {
                if !runtime.matches::<#context>() {
                    return Err(
                        ::vela_engine::service::ServiceStagingError::ContextTypeMismatch {
                            expected: ::core::any::type_name::<#context>(),
                            actual: runtime.context_name(),
                        }
                    );
                }
                let artifact = ::std::sync::Arc::clone(bundle.artifact());
                let base_selections = match base.selections() {
                    Some(selections) => selections.clone(),
                    None => ::vela_engine::service::ServiceSelectionTable::snapshot(
                        &self.schema,
                        ::core::iter::empty::<
                            ::vela_engine::service::ServiceMethodUpdate<
                                ::vela_engine::service::LinkedVelaServiceMethod
                            >
                        >(),
                    )?,
                };
                let (selections, _) = bundle.into_selection(
                    &self.schema,
                    base.generation_id(),
                    base.artifact_checksum(),
                    &base_selections,
                )?;
                let generation = #generation_ident::__vela_composed(
                    runtime,
                    options,
                    selections,
                    Some(artifact),
                );
                self.controller
                    .stage(&base.root, generation)
                    .map(|candidate| #candidate_ident { candidate })
                    .map_err(::vela_engine::service::ServiceStagingError::from)
            }

            pub fn activate_if_current(
                &self,
                candidate: #candidate_ident,
            ) -> ::std::result::Result<
                #rollback_ident,
                ::vela_engine::service::ServicePublicationError,
            > {
                self.controller
                    .activate_if_current(candidate.candidate)
                    .map(|token| #rollback_ident { token })
            }

            pub fn rollback_if_current(
                &self,
                rollback: #rollback_ident,
            ) -> ::std::result::Result<
                #root_ident,
                ::vela_engine::service::ServicePublicationError,
            > {
                self.controller
                    .rollback_if_current(rollback.token)
                    .map(|root| #root_ident {
                        root,
                        _context: ::std::marker::PhantomData,
                    })
            }
        }

        #vis struct #root_ident {
            root: ::vela_engine::service::ServiceRoot<#generation_ident>,
            _context: ::std::marker::PhantomData<fn(&mut #context)>,
        }

        impl ::std::clone::Clone for #root_ident {
            fn clone(&self) -> Self {
                Self {
                    root: self.root.clone(),
                    _context: ::std::marker::PhantomData,
                }
            }
        }

        impl #root_ident {
            #[must_use]
            pub fn generation_id(&self) -> ::vela_common::ServiceGenerationId {
                self.root.generation_id()
            }

            #[must_use]
            pub fn service_set_id(&self) -> ::vela_common::ServiceSetId {
                self.root.service_set_id()
            }

            #[must_use]
            pub fn selections(
                &self,
            ) -> ::std::option::Option<
                &::vela_engine::service::ServiceSelectionTable<
                    ::vela_engine::service::LinkedVelaServiceMethod
                >
            > {
                self.root.services().selections()
            }

            #[must_use]
            pub fn artifact_checksum(
                &self,
            ) -> ::std::option::Option<::vela_engine::service::ArtifactChecksum> {
                self.root.services().artifact_checksum()
            }

            #[must_use]
            pub fn artifact(
                &self,
            ) -> ::std::option::Option<
                &::std::sync::Arc<::vela_engine::service::LinkedArtifact>
            > {
                self.root.services().artifact()
            }

            #(#root_accessors)*
        }

        #vis struct #candidate_ident {
            candidate: ::vela_engine::service::ServiceGenerationCandidate<#generation_ident>,
        }

        impl #candidate_ident {
            #[must_use]
            pub fn generation_id(&self) -> ::vela_common::ServiceGenerationId {
                self.candidate.generation_id()
            }

            #[must_use]
            pub fn base_generation_id(&self) -> ::vela_common::ServiceGenerationId {
                self.candidate.base_generation_id()
            }

            #[must_use]
            pub fn artifact_checksum(
                &self,
            ) -> ::std::option::Option<::vela_engine::service::ArtifactChecksum> {
                self.candidate.generation().services().artifact_checksum()
            }
        }

        #vis struct #rollback_ident {
            token: ::vela_engine::service::ServiceRollbackToken<#generation_ident>,
        }

        impl #rollback_ident {
            #[must_use]
            pub fn replaced_generation_id(&self) -> ::vela_common::ServiceGenerationId {
                self.token.replaced_generation_id()
            }

            #[must_use]
            pub fn installed_generation_id(&self) -> ::vela_common::ServiceGenerationId {
                self.token.installed_generation_id()
            }
        }
    })
}

fn validate_struct(item: &ItemStruct) -> Result<()> {
    if !matches!(item.vis, Visibility::Public(_)) {
        return Err(syn::Error::new_spanned(
            &item.vis,
            "#[vela_macros::service_set] requires a public Rust struct",
        ));
    }
    reject_generic_signature(&item.generics, "#[vela_macros::service_set]")?;
    if !matches!(item.fields, Fields::Named(_)) {
        return Err(syn::Error::new_spanned(
            &item.fields,
            "#[vela_macros::service_set] requires named service fields",
        ));
    }
    Ok(())
}

fn parse_context(attr: TokenStream) -> Result<Type> {
    let mut context = None;
    let parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("context") {
            if context.is_some() {
                return Err(meta.error("service-set context is duplicated"));
            }
            context = Some(meta.value()?.parse::<Type>()?);
            return Ok(());
        }
        Err(meta.error("unsupported service_set attribute"))
    });
    parser.parse2(attr)?;
    context.ok_or_else(|| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            "#[vela_macros::service_set] requires context = HostContext",
        )
    })
}

struct ServiceField {
    field: syn::Ident,
    trait_path: Path,
    default: Path,
}

impl ServiceField {
    fn dispatch_trait_path(&self) -> Path {
        let mut path = replace_trait_ident(
            &self.trait_path,
            dispatch_module_ident(
                &self
                    .trait_path
                    .segments
                    .last()
                    .expect("service trait path is non-empty")
                    .ident,
            ),
        );
        path.segments.push(parse_quote!(Dispatch));
        path
    }

    fn registration_path(&self) -> Path {
        replace_trait_ident(
            &self.trait_path,
            registration_function_ident(
                &self
                    .trait_path
                    .segments
                    .last()
                    .expect("service trait path is non-empty")
                    .ident,
            ),
        )
    }

    fn schema_path(&self) -> Path {
        replace_trait_ident(
            &self.trait_path,
            schema_function_ident(
                &self
                    .trait_path
                    .segments
                    .last()
                    .expect("service trait path is non-empty")
                    .ident,
            ),
        )
    }

    fn composition_path(&self) -> Path {
        replace_trait_ident(
            &self.trait_path,
            composition_function_ident(
                &self
                    .trait_path
                    .segments
                    .last()
                    .expect("service trait path is non-empty")
                    .ident,
            ),
        )
    }

    fn service_id_path(&self) -> Path {
        replace_trait_ident(
            &self.trait_path,
            service_id_function_ident(
                &self
                    .trait_path
                    .segments
                    .last()
                    .expect("service trait path is non-empty")
                    .ident,
            ),
        )
    }

    fn rust_dispatch_path(&self) -> Path {
        replace_trait_ident(
            &self.trait_path,
            rust_dispatch_function_ident(
                &self
                    .trait_path
                    .segments
                    .last()
                    .expect("service trait path is non-empty")
                    .ident,
            ),
        )
    }

    fn rust_async_dispatch_path(&self) -> Path {
        replace_trait_ident(
            &self.trait_path,
            rust_async_dispatch_function_ident(
                &self
                    .trait_path
                    .segments
                    .last()
                    .expect("service trait path is non-empty")
                    .ident,
            ),
        )
    }
}

fn parse_service_field(field: &syn::Field) -> Result<ServiceField> {
    if !matches!(field.vis, Visibility::Public(_)) {
        return Err(syn::Error::new_spanned(
            &field.vis,
            "service-set fields must be public",
        ));
    }
    let field_ident = field.ident.clone().expect("named field");
    let Type::TraitObject(object) = &field.ty else {
        return Err(syn::Error::new_spanned(
            &field.ty,
            "service-set fields must use `dyn ServiceTrait`",
        ));
    };
    let [TypeParamBound::Trait(bound)] = object.bounds.iter().collect::<Vec<_>>().as_slice() else {
        return Err(syn::Error::new_spanned(
            object,
            "service-set fields must name exactly one `dyn ServiceTrait`",
        ));
    };
    if !matches!(bound.modifier, syn::TraitBoundModifier::None)
        || bound.lifetimes.is_some()
        || !bound
            .path
            .segments
            .iter()
            .all(|segment| matches!(segment.arguments, syn::PathArguments::None))
    {
        return Err(syn::Error::new_spanned(
            bound,
            "service-set trait paths cannot use modifiers, binders, or generic arguments",
        ));
    }
    let default = parse_default(&field.attrs)?;
    Ok(ServiceField {
        field: field_ident,
        trait_path: bound.path.clone(),
        default,
    })
}

fn parse_default(attrs: &[Attribute]) -> Result<Path> {
    let mut default = None;
    for attr in attrs {
        if !attr.path().is_ident("vela") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if !meta.path.is_ident("default") {
                return Err(meta.error("unsupported service-set field attribute"));
            }
            if default.is_some() {
                return Err(meta.error("service default is duplicated"));
            }
            default = Some(meta.value()?.parse::<Path>()?);
            Ok(())
        })?;
    }
    default.ok_or_else(|| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            "service-set field requires #[vela(default = RustService)]",
        )
    })
}

fn replace_trait_ident(path: &Path, ident: syn::Ident) -> Path {
    let mut path = path.clone();
    path.segments
        .last_mut()
        .expect("service trait path is non-empty")
        .ident = ident;
    path
}

#[cfg(test)]
mod tests {
    use quote::quote;

    use super::expand_result;

    #[test]
    fn service_set_generates_one_whole_generation_controller() {
        let output = expand_result(
            quote! { context = RequestContext },
            quote! {
                pub struct GameServices {
                    #[vela(default = RustRewardService)]
                    pub reward: dyn RewardService,
                    #[vela(default = RustInventoryService)]
                    pub inventory: dyn InventoryService,
                }
            },
        )
        .expect("service set should expand")
        .to_string();

        assert!(output.contains("GameServicesGeneration"));
        assert!(output.contains("ServiceController < GameServicesGeneration >"));
        assert!(output.contains("stage_snapshot"));
        assert!(output.contains("stage_delta"));
        assert!(output.contains("__vela_compose_service_RewardService"));
        assert!(output.contains("register_service_set_schema"));
        assert_eq!(output.matches("ServiceController <").count(), 1);
        assert!(!output.contains("HostRef"));
        assert!(!output.contains("runtime : :: vela_engine :: runtime :: Runtime"));
    }

    #[test]
    fn service_set_requires_explicit_default() {
        let error = expand_result(
            quote! { context = RequestContext },
            quote! {
                pub struct GameServices {
                    pub reward: dyn RewardService,
                }
            },
        )
        .expect_err("missing default must fail");

        assert!(error.to_string().contains("requires #[vela(default"));
    }
}
