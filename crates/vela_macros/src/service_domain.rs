use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Fields, ItemStruct, Result, parse2};

use crate::service_domain_emission::DomainEmission;
use crate::service_domain_input::{
    parse_context, parse_service_field, validate_services_not_empty, validate_struct,
};

pub(crate) fn expand(attr: TokenStream, input: TokenStream) -> TokenStream {
    expand_result(attr, input).unwrap_or_else(|error| error.to_compile_error())
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
    validate_services_not_empty(&item, &services)?;

    let set_ident = &item.ident;
    let generation_ident = format_ident!("__{set_ident}Generation");
    let defaults_ident = format_ident!("__{set_ident}Defaults");
    let builder_ident = format_ident!("{set_ident}Builder");
    let builder_state_ident = format_ident!("__{set_ident}BuilderState");
    let app_ident = format_ident!("{set_ident}App");
    let patches_ident = format_ident!("{set_ident}Patches");
    let staged_patch_ident = format_ident!("{set_ident}StagedPatch");
    let call_ident = format_ident!("{set_ident}Call");
    let root_ident = format_ident!("{set_ident}Root");
    let candidate_ident = format_ident!("{set_ident}Candidate");
    let rollback_ident = format_ident!("{set_ident}Rollback");
    let dispatcher_ident = format_ident!("__VelaServiceDispatcher{set_ident}");
    let DomainEmission {
        marker_uses,
        registration_entries,
        schema_entries,
        generation_fields,
        defaults_fields,
        builder_fields,
        empty_builder_fields,
        builder_setters,
        required_defaults,
        defaults_initializers,
        initial_generation_fields,
        rust_snapshot_fields,
        dispatcher_fields,
        dispatcher_initializers,
        dispatcher_rust_branches,
        dispatcher_async_rust_branches,
        composed_services,
        generation_initializers,
        generation_accessors,
        root_accessors,
    } = DomainEmission::new(&services, set_ident);
    let vis = &item.vis;
    let schema_factory_ident = format_ident!("__vela_service_domain_schema_{set_ident}");
    let schema_factory = crate::service_domain_emission::schema_factory(
        set_ident,
        &schema_factory_ident,
        &schema_entries,
    );

    Ok(quote! {
        #(#marker_uses)*
        #schema_factory

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
            execution: ::std::option::Option<
                ::vela_engine::service::PinnedServiceExecution
            >,
        }

        impl #generation_ident {
            fn from_defaults(defaults: &#defaults_ident) -> Self {
                Self {
                    #(#initial_generation_fields,)*
                    selections: None,
                    artifact: None,
                    execution: None,
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
                identity: ::vela_engine::service::ServiceExecutionIdentity,
            ) -> Self {
                if artifact.is_none() {
                    return Self {
                        #(#rust_snapshot_fields,)*
                        selections: Some(selections),
                        artifact: None,
                        execution: None,
                    };
                }
                let __vela_dispatcher: ::std::sync::Arc<
                    dyn ::vela_engine::service::ServiceCallDispatcher
                > = ::std::sync::Arc::new(#dispatcher_ident {
                    #(#dispatcher_initializers,)*
                    selections: selections.clone(),
                });
                let __vela_execution =
                    ::vela_engine::service::PinnedServiceExecution::new(
                        identity,
                        ::std::sync::Arc::clone(&__vela_dispatcher),
                        ::std::sync::Arc::clone(
                            artifact.as_ref().expect(
                                "a composed Vela Service generation owns its linked artifact"
                            ),
                        ),
                        runtime,
                        options,
                    );
                #(#composed_services)*
                Self {
                    #(#generation_initializers,)*
                    selections: Some(selections),
                    artifact,
                    execution: Some(__vela_execution),
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

            #[must_use]
            pub fn execution(
                &self,
            ) -> ::std::option::Option<
                &::vela_engine::service::PinnedServiceExecution
            > {
                self.execution.as_ref()
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
                            ) => {
                                __vela_context.release_service_leases_for_vela_reentry(
                                    __vela_leases,
                                );
                                __vela_method.call_in_context_async(
                                    __vela_context,
                                    __vela_args,
                                )
                            }
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
                type __VelaServiceRegistration = fn(
                    ::vela_engine::builder::EngineBuilder
                ) -> ::vela_engine::builder::EngineBuilder;
                const __VELA_SERVICE_REGISTRATIONS: &[
                    __VelaServiceRegistration
                ] = &[#(#registration_entries),*];

                #[inline(never)]
                fn __vela_update_engine_builder(
                    builder: &mut ::std::option::Option<
                        ::vela_engine::builder::EngineBuilder
                    >,
                    update: fn(
                        ::vela_engine::builder::EngineBuilder
                    ) -> ::vela_engine::builder::EngineBuilder,
                ) {
                    let current = builder.take().expect(
                        "generated Service registration owns its Engine builder"
                    );
                    *builder = Some(update(current));
                }

                let mut builder = Some(builder);
                for &registration in __VELA_SERVICE_REGISTRATIONS {
                    __vela_update_engine_builder(&mut builder, registration);
                }
                let builder = builder.take().expect(
                    "generated Service registration retains its Engine builder"
                );
                #builder_ident {
                    state: ::std::boxed::Box::new(#builder_state_ident {
                        engine: Some(
                            builder.register_service_set_schema(#schema_factory_ident)
                        ),
                        call_options: ::vela_engine::runtime::CallOptions::new(
                            1_000_000,
                            16 * 1024 * 1024,
                            256,
                        ),
                        task_scope: None,
                        emergency_patch_effect_ceiling: None,
                        #(#empty_builder_fields,)*
                    }),
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
                let artifact = update.artifact().cloned();
                let selections = update.into_snapshot(&self.schema)?;
                self.controller
                    .stage(&base.root, |generation| #generation_ident::__vela_composed(
                        &self.defaults,
                        runtime,
                        options,
                        selections,
                        artifact,
                        ::vela_engine::service::ServiceExecutionIdentity::new(
                            self.schema.id(),
                            generation,
                            self.schema.patch_effect_ceiling().required_capability_set(),
                        ),
                    ))
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
                self.controller
                    .stage(&base.root, |generation| #generation_ident::__vela_composed(
                        &self.defaults,
                        runtime,
                        options,
                        selections,
                        artifact,
                        ::vela_engine::service::ServiceExecutionIdentity::new(
                            self.schema.id(),
                            generation,
                            self.schema.patch_effect_ceiling().required_capability_set(),
                        ),
                    ))
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
                self.controller
                    .stage(&base.root, |generation| #generation_ident::__vela_composed(
                        &self.defaults,
                        runtime,
                        options,
                        selections,
                        Some(artifact),
                        ::vela_engine::service::ServiceExecutionIdentity::new(
                            self.schema.id(),
                            generation,
                            self.schema.patch_effect_ceiling().required_capability_set(),
                        ),
                    ))
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

        struct #builder_state_ident {
            engine: ::std::option::Option<::vela_engine::builder::EngineBuilder>,
            call_options: ::vela_engine::runtime::CallOptions,
            task_scope: ::std::option::Option<::vela_engine::task::TaskScope>,
            emergency_patch_effect_ceiling:
                ::std::option::Option<::vela_engine::native::EffectSet>,
            #(#builder_fields,)*
        }

        #vis struct #builder_ident {
            state: ::std::boxed::Box<#builder_state_ident>,
        }

        impl #builder_ident {
            #[must_use]
            pub fn call_options(
                mut self,
                options: ::vela_engine::runtime::CallOptions,
            ) -> Self {
                self.state.call_options = options;
                self
            }

            #[must_use]
            pub fn task_scope(mut self, scope: ::vela_engine::task::TaskScope) -> Self {
                self.state.task_scope = Some(scope);
                self
            }

            #[must_use]
            pub fn emergency_patch_effect_ceiling(
                mut self,
                ceiling: ::vela_engine::native::EffectSet,
            ) -> Self {
                self.state.emergency_patch_effect_ceiling = Some(ceiling);
                self
            }

            #(#builder_setters)*

            pub fn build(
                mut self,
            ) -> ::std::result::Result<
                #app_ident,
                ::vela_engine::service::ServiceDomainBuildError,
            > {
                #(#required_defaults)*
                let task_scope = self.state.task_scope.take().ok_or(
                    ::vela_engine::service::ServiceDomainBuildError::MissingTaskScope {
                        domain: ::std::stringify!(#set_ident),
                    },
                )?;
                let emergency_patch_effect_ceiling =
                    self.state.emergency_patch_effect_ceiling.take().ok_or(
                        ::vela_engine::service::ServiceDomainBuildError::MissingPatchEffectCeiling {
                            domain: ::std::stringify!(#set_ident),
                        },
                    )?;
                if !emergency_patch_effect_ceiling
                    .contains_all(::vela_engine::native::EffectSet::task_spawn())
                {
                    return Err(
                        ::vela_engine::service::ServiceDomainBuildError::PatchEffectCeilingMissingTaskSpawn {
                            domain: ::std::stringify!(#set_ident),
                        },
                    );
                }
                let engine = self.state.engine.take().expect(
                        "generated Service builder owns its Engine builder"
                    )
                    .service_patch_effect_ceiling(emergency_patch_effect_ceiling)
                    .build()?;
                let runtime =
                    ::vela_engine::service::ServiceRuntimeBinding::for_engine(
                        engine.clone()
                    );
                let schema = #schema_factory_ident(
                    engine.type_bindings().as_ref(),
                    emergency_patch_effect_ceiling,
                )?;
                let id = schema.id();
                let defaults = #defaults_ident {
                    #(#defaults_initializers,)*
                };
                let domain = #set_ident {
                    controller: ::vela_engine::service::ServiceController::new(
                        id,
                        |_| #generation_ident::from_defaults(&defaults),
                    ),
                    schema,
                    defaults,
                    _context: ::std::marker::PhantomData,
                };
                Ok(#app_ident {
                    engine,
                    domain,
                    call_options: self.state.call_options.clone().with_task_scope(task_scope),
                    runtime,
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
            runtime: ::vela_engine::service::ServiceRuntimeBinding,
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
                    runtime: self.runtime.clone(),
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
            runtime: ::vela_engine::service::ServiceRuntimeBinding,
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
                let runtime = self.runtime;
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
                let runtime = self.runtime;
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

            #[doc(hidden)]
            #[must_use]
            pub fn pinned_execution(
                &self,
            ) -> ::std::option::Option<
                &::vela_engine::service::PinnedServiceExecution
            > {
                self.root.services().execution()
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
