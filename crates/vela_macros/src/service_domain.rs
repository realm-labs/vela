use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Fields, ItemStruct, Result, parse2};

use crate::service_domain_input::{parse_context, parse_service_field, validate_struct};

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
            "#[vela_macros::service_domain] requires at least one service field",
        ));
    }

    let set_ident = &item.ident;
    let generation_ident = format_ident!("__{set_ident}Generation");
    let defaults_ident = format_ident!("__{set_ident}Defaults");
    let builder_ident = format_ident!("{set_ident}Builder");
    let app_ident = format_ident!("{set_ident}App");
    let patches_ident = format_ident!("{set_ident}Patches");
    let staged_patch_ident = format_ident!("{set_ident}StagedPatch");
    let call_ident = format_ident!("{set_ident}Call");
    let root_ident = format_ident!("{set_ident}Root");
    let candidate_ident = format_ident!("{set_ident}Candidate");
    let rollback_ident = format_ident!("{set_ident}Rollback");
    let dispatcher_ident = format_ident!("__VelaServiceDispatcher{set_ident}");
    let marker_uses = services.iter().map(|service| {
        let marker = &service.marker;
        quote! {
            const _: usize = ::core::mem::size_of::<#marker<()>>();
        }
    });
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
    let defaults_fields = services.iter().map(|service| {
        let field = &service.field;
        let trait_path = service.dispatch_trait_path();
        quote! {
            #field: ::std::sync::Arc<dyn #trait_path>
        }
    });
    let builder_fields = services.iter().map(|service| {
        let field = &service.field;
        let trait_path = service.dispatch_trait_path();
        quote! {
            #field: ::std::option::Option<::std::sync::Arc<dyn #trait_path>>
        }
    });
    let empty_builder_fields = services.iter().map(|service| {
        let field = &service.field;
        quote! {
            #field: None
        }
    });
    let builder_setters = services.iter().map(|service| {
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
                self.#field = Some(implementation);
                self
            }
        }
    });
    let required_defaults = services.iter().map(|service| {
        let field = &service.field;
        quote! {
            let #field = self.#field.ok_or(
                ::vela_engine::service::ServiceDomainBuildError::MissingDefault {
                    domain: ::std::stringify!(#set_ident),
                    service: ::std::stringify!(#field),
                },
            )?;
        }
    });
    let defaults_initializers = services.iter().map(|service| {
        let field = &service.field;
        quote! {
            #field
        }
    });
    let initial_generation_fields = services.iter().map(|service| {
        let field = &service.field;
        quote! {
            #field: ::std::sync::Arc::clone(&defaults.#field)
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
        quote! {
            #field: ::std::sync::Arc::clone(&defaults.#field)
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
        quote! {
            let #field: ::std::sync::Arc<dyn #trait_path> = #composition(
                ::std::sync::Arc::clone(&defaults.#field),
                runtime,
                options.clone(),
                ::std::sync::Arc::clone(&__vela_dispatcher),
                &selections,
            );
        }
    });
    let generation_initializers = services
        .iter()
        .map(|service| &service.field)
        .collect::<Vec<_>>();
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
    let vis = &item.vis;
    let schema_factory_ident = format_ident!("__vela_service_domain_schema_{set_ident}");

    Ok(quote! {
        #(#marker_uses)*

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
                u128::from(::vela_common::stable_id("vela_service_domain", "", path)),
            );
            ::vela_engine::service::ServiceSetSchema::new_named(
                id,
                path,
                vec![#(#schema_calls),*],
                registry,
            )
        }

        struct #defaults_ident {
            #(#defaults_fields,)*
        }

        struct #generation_ident {
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
            fn from_defaults(defaults: &#defaults_ident) -> Self {
                Self {
                    #(#initial_generation_fields,)*
                    selections: None,
                    artifact: None,
                }
            }

            fn __vela_composed(
                defaults: &#defaults_ident,
                runtime: ::vela_engine::service::ServiceRuntimeBinding,
                options: ::vela_engine::runtime::CallOptions,
                selections: ::vela_engine::service::ServiceSelectionTable<
                    ::vela_engine::service::LinkedVelaServiceMethod
                >,
                artifact: ::std::option::Option<
                    ::std::sync::Arc<::vela_engine::service::LinkedArtifact>
                >,
            ) -> Self {
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
            defaults: #defaults_ident,
            _context: ::std::marker::PhantomData<fn(&mut #context)>,
        }

        impl #set_ident {
            #[must_use]
            pub fn builder(
                builder: ::vela_engine::builder::EngineBuilder,
            ) -> #builder_ident {
                let builder = builder;
                #(#register_calls)*
                #builder_ident {
                    engine: builder.register_service_set_schema(#schema_factory_ident),
                    call_options: ::vela_engine::runtime::CallOptions::new(
                        1_000_000,
                        16 * 1024 * 1024,
                        256,
                    ),
                    runtime: None,
                    #(#empty_builder_fields,)*
                }
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
                    &self.defaults,
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
                    &self.defaults,
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
                    &self.defaults,
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
                    .map(|token| #rollback_ident {
                        token,
                        patch: ::std::option::Option::None,
                    })
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

        #vis struct #builder_ident {
            engine: ::vela_engine::builder::EngineBuilder,
            call_options: ::vela_engine::runtime::CallOptions,
            runtime: ::std::option::Option<
                ::vela_engine::service::ServiceRuntimeBinding
            >,
            #(#builder_fields,)*
        }

        impl #builder_ident {
            #[must_use]
            pub fn call_options(
                mut self,
                options: ::vela_engine::runtime::CallOptions,
            ) -> Self {
                self.call_options = options;
                self
            }

            #[must_use]
            pub fn actor_runtime<__VelaContext>(mut self) -> Self
            where
                __VelaContext:
                    ::vela_engine::service::ServiceRuntimeAuthority + 'static,
            {
                self.runtime = Some(
                    ::vela_engine::service::ServiceRuntimeBinding::
                        for_context::<__VelaContext>()
                );
                self
            }

            #(#builder_setters)*

            pub fn build(
                self,
            ) -> ::std::result::Result<
                #app_ident,
                ::vela_engine::service::ServiceDomainBuildError,
            > {
                #(#required_defaults)*
                if let Some(runtime) = self.runtime
                    && !runtime.matches::<#context>()
                {
                    return Err(
                        ::vela_engine::service::ServiceDomainBuildError::
                            ContextTypeMismatch {
                                expected: ::core::any::type_name::<#context>(),
                                actual: runtime.context_name(),
                            }
                    );
                }
                let engine = self.engine.build()?;
                let schema = #schema_factory_ident(engine.type_bindings().as_ref())?;
                let id = schema.id();
                let defaults = #defaults_ident {
                    #(#defaults_initializers,)*
                };
                let initial = #generation_ident::from_defaults(&defaults);
                let domain = #set_ident {
                    controller: ::vela_engine::service::ServiceController::new(id, initial),
                    schema,
                    defaults,
                    _context: ::std::marker::PhantomData,
                };
                Ok(#app_ident {
                    engine,
                    domain,
                    call_options: self.call_options,
                    runtime: self.runtime,
                    patch_state: ::vela_engine::service::ServicePatchState::new(
                        ::vela_common::ServiceGenerationId::new(1),
                    ),
                })
            }
        }

        #vis struct #app_ident {
            engine: ::vela_engine::engine::Engine,
            domain: #set_ident,
            call_options: ::vela_engine::runtime::CallOptions,
            runtime: ::std::option::Option<
                ::vela_engine::service::ServiceRuntimeBinding
            >,
            patch_state: ::vela_engine::service::ServicePatchState,
        }

        impl #app_ident {
            #[must_use]
            pub fn engine(&self) -> &::vela_engine::engine::Engine {
                &self.engine
            }

            #[must_use]
            pub fn domain(&self) -> &#set_ident {
                &self.domain
            }

            #[must_use]
            pub fn patches(&self) -> #patches_ident<'_> {
                #patches_ident {
                    engine: &self.engine,
                    domain: &self.domain,
                    call_options: &self.call_options,
                    runtime: self.runtime,
                    patch_state: &self.patch_state,
                }
            }

            #[must_use]
            pub fn begin<'context>(
                &self,
                context: &'context mut #context,
            ) -> #call_ident<'context> {
                #call_ident {
                    root: self.domain.pin(),
                    context,
                }
            }

            pub fn with_request<'context, __VelaOutput>(
                &self,
                context: &'context mut #context,
                call: impl ::core::ops::FnOnce(
                    &#root_ident,
                    &'context mut #context,
                ) -> __VelaOutput,
            ) -> __VelaOutput {
                let root = self.domain.pin();
                call(&root, context)
            }

            pub async fn with_request_async<__VelaCall, __VelaOutput>(
                &self,
                context: &mut #context,
                call: __VelaCall,
            ) -> __VelaOutput
            where
                __VelaCall: ::core::ops::AsyncFnOnce(
                    &#root_ident,
                    &mut #context,
                ) -> __VelaOutput,
            {
                let root = self.domain.pin();
                call(&root, context).await
            }

            #[must_use]
            pub fn into_parts(self) -> (::vela_engine::engine::Engine, #set_ident) {
                (self.engine, self.domain)
            }
        }

        #vis struct #call_ident<'context> {
            root: #root_ident,
            context: &'context mut #context,
        }

        impl #call_ident<'_> {
            #[must_use]
            pub fn generation_id(&self) -> ::vela_common::ServiceGenerationId {
                self.root.generation_id()
            }

            #[must_use]
            pub fn parts(&mut self) -> (&#root_ident, &mut #context) {
                (&self.root, self.context)
            }
        }

        #vis struct #patches_ident<'app> {
            engine: &'app ::vela_engine::engine::Engine,
            domain: &'app #set_ident,
            call_options: &'app ::vela_engine::runtime::CallOptions,
            runtime: ::std::option::Option<
                ::vela_engine::service::ServiceRuntimeBinding
            >,
            patch_state: &'app ::vela_engine::service::ServicePatchState,
        }

        impl<'app> #patches_ident<'app> {
            pub fn revision(
                &self,
            ) -> ::std::result::Result<
                ::std::sync::Arc<::vela_engine::service::PatchRevision>,
                ::vela_engine::service::ServicePatchError,
            > {
                let active = self.domain.pin();
                self.patch_state
                    .revision(active.generation_id())
                    .map_err(::vela_engine::service::ServicePatchError::from)
            }

            pub fn stage(
                self,
                patch: impl ::core::convert::Into<
                    ::vela_engine::service::ServicePatch
                >,
            ) -> ::std::result::Result<
                #staged_patch_ident<'app>,
                ::vela_engine::service::ServicePatchError,
            > {
                let runtime = self.runtime.ok_or(
                    ::vela_engine::service::ServicePatchError::
                        MissingRuntimeAuthority {
                            domain: ::std::stringify!(#set_ident),
                        }
                )?;
                let base = self.domain.pin();
                let revision = self.patch_state.prepare(
                    base.generation_id(),
                    patch.into(),
                )?;
                let bundle = self.engine.compile_service_patch(
                    self.domain.schema(),
                    &revision,
                )?;
                let candidate = self.domain.stage_bundle(
                    &base,
                    bundle,
                    runtime,
                    self.call_options.clone(),
                )?;
                Ok(#staged_patch_ident {
                    domain: self.domain,
                    candidate,
                    patch_state: self.patch_state,
                    revision: ::std::option::Option::Some(revision),
                })
            }

            pub fn apply(
                self,
                patch: impl ::core::convert::Into<
                    ::vela_engine::service::ServicePatch
                >,
            ) -> ::std::result::Result<
                #rollback_ident,
                ::vela_engine::service::ServicePatchError,
            > {
                self.stage(patch)?.activate()
            }

            #[must_use]
            pub fn dry_run_bundle(
                &self,
                bundle: &::vela_engine::service::ServiceUpdateBundle,
            ) -> ::vela_engine::service::ServiceDryRunReport {
                self.domain.dry_run_bundle(&self.domain.pin(), bundle)
            }

            pub fn stage_bundle(
                self,
                bundle: ::vela_engine::service::ServiceUpdateBundle,
            ) -> ::std::result::Result<
                #staged_patch_ident<'app>,
                ::vela_engine::service::ServicePatchError,
            > {
                let runtime = self.runtime.ok_or(
                    ::vela_engine::service::ServicePatchError::
                        MissingRuntimeAuthority {
                            domain: ::std::stringify!(#set_ident),
                        }
                )?;
                let base = self.domain.pin();
                let candidate = self.domain.stage_bundle(
                    &base,
                    bundle,
                    runtime,
                    self.call_options.clone(),
                )?;
                Ok(#staged_patch_ident {
                    domain: self.domain,
                    candidate,
                    patch_state: self.patch_state,
                    revision: ::std::option::Option::None,
                })
            }

            pub fn rollback(
                self,
                rollback: #rollback_ident,
            ) -> ::std::result::Result<
                #root_ident,
                ::vela_engine::service::ServicePatchError,
            > {
                let #rollback_ident { token, patch } = rollback;
                let root = self.domain
                    .controller
                    .rollback_if_current(token)
                    .map(|root| #root_ident {
                        root,
                        _context: ::std::marker::PhantomData,
                    })
                    .map_err(::vela_engine::service::ServicePatchError::from)?;
                if let ::std::option::Option::Some(patch) = patch {
                    self.patch_state.record_rollback(patch)?;
                }
                Ok(root)
            }
        }

        #vis struct #staged_patch_ident<'app> {
            domain: &'app #set_ident,
            candidate: #candidate_ident,
            patch_state: &'app ::vela_engine::service::ServicePatchState,
            revision: ::std::option::Option<
                ::vela_engine::service::PatchRevision
            >,
        }

        impl #staged_patch_ident<'_> {
            #[must_use]
            pub fn generation_id(&self) -> ::vela_common::ServiceGenerationId {
                self.candidate.generation_id()
            }

            pub fn activate(
                self,
            ) -> ::std::result::Result<
                #rollback_ident,
                ::vela_engine::service::ServicePatchError,
            > {
                let expected = self.candidate.base_generation_id();
                let installed = self.candidate.generation_id();
                let token = self.domain
                    .controller
                    .activate_if_current(self.candidate.candidate)
                    .map_err(::vela_engine::service::ServicePatchError::from)?;
                let patch = match self.patch_state.record_activation(
                    expected,
                    installed,
                    self.revision,
                ) {
                    Ok(patch) => patch,
                    Err(error) => {
                        let _ = self.domain.controller.rollback_if_current(token);
                        return Err(
                            ::vela_engine::service::ServicePatchError::from(error)
                        );
                    }
                };
                Ok(#rollback_ident {
                    token,
                    patch: ::std::option::Option::Some(patch),
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
            patch: ::std::option::Option<
                ::vela_engine::service::ServicePatchStateRollback
            >,
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

#[cfg(test)]
mod tests;
