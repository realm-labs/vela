use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::ItemFn;

use super::attrs::ExportAttrs;
use super::signature::{
    BorrowOrigin, ClassifiedSignature, EffectName, ErrorMode, HostAccess, ParameterMode,
    ReturnMode, ScopedReturnContainer, TypeShape,
};

mod binding;

use binding::{binding_use_tokens, collection_registration_tokens};
pub(crate) use binding::{
    exclusive_host_value_tokens, hint_tokens, host_type_id_tokens, shared_host_value_tokens,
};

pub(crate) fn function_contract(
    item: &ItemFn,
    attrs: &ExportAttrs,
    docs: Option<&str>,
    signature: &ClassifiedSignature,
) -> TokenStream {
    let function_ident = &item.sig.ident;
    let contract_ident = format_ident!("vela_callable_contract_{function_ident}");
    let public_path = &attrs.path;
    let callable_id = u128::from(vela_common::stable_id("rust_export", "", public_path));
    let parameters = signature
        .parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            let name = &parameter.name;
            let identity = vela_common::stable_id("callable_parameter", public_path, name);
            let hint = hint_tokens(&parameter.ty);
            let mode = parameter_mode_tokens(parameter.mode);
            let _ = index;
            let contract = quote! {
                ::vela_engine::interop::CallableParameter::new(#identity, #name, #hint, #mode)
            };
            binding_use_tokens(contract, &parameter.ty)
        });
    let return_hint = hint_tokens(&signature.returns.ty);
    let return_mode = return_mode_tokens(signature.returns.mode, &signature.returns.ty);
    let error_mode = match signature.returns.error_mode {
        ErrorMode::Value => quote! { ::vela_engine::interop::ErrorMode::Value },
        ErrorMode::RuntimeResult => quote! { ::vela_engine::interop::ErrorMode::RuntimeResult },
    };
    let return_contract = binding_use_tokens(
        quote! {
            ::vela_engine::interop::CallableReturn::new(
                #return_hint,
                #return_mode,
                #error_mode,
            )
        },
        &signature.returns.ty,
    );
    let effects = effect_tokens(&signature.effects);
    let asyncness = if signature.is_async {
        quote! { ::vela_common::CallableAsyncness::Async }
    } else {
        quote! { ::vela_common::CallableAsyncness::Sync }
    };
    let docs = docs.map_or_else(|| quote! { None }, |docs| quote! { Some(#docs.to_owned()) });

    quote! {
        #[doc(hidden)]
        #[must_use]
        pub fn #contract_ident() -> ::vela_engine::interop::CallableContract {
            ::vela_engine::interop::CallableContract {
                identity: ::vela_engine::interop::CallableIdentity::new(
                    ::vela_engine::interop::CallableKind::RustFunction,
                    #callable_id,
                ),
                public_path: #public_path.to_owned(),
                parameters: vec![#(#parameters),*],
                returns: #return_contract,
                asyncness: #asyncness,
                effects: #effects,
                access: ::vela_engine::interop::CallableAccess::default(),
                docs: #docs,
                origin: ::vela_engine::interop::CallableOrigin {
                    language: ::vela_engine::interop::CallableLanguage::Rust,
                    source_span: None,
                },
            }
        }
    }
}

pub(crate) fn function_value_adapter(
    item: &ItemFn,
    signature: &ClassifiedSignature,
) -> Option<TokenStream> {
    if signature.supports_sync_scoped_host_adapter() {
        return Some(function_sync_scoped_host_adapter(item, signature));
    }
    if signature.supports_async_host_adapter() {
        return Some(function_async_host_adapter(item, signature));
    }
    if signature.supports_sync_host_adapter() {
        return Some(function_sync_host_adapter(item, signature));
    }
    if !signature.supports_value_adapter() {
        return None;
    }
    let function_ident = &item.sig.ident;
    let contract_ident = format_ident!("vela_callable_contract_{function_ident}");
    let register_ident = format_ident!("vela_register_export_{function_ident}");
    let bundle_ident = format_ident!("vela_export_bundle_{function_ident}");
    let value_types = signature
        .parameters
        .iter()
        .filter(|parameter| parameter.mode == ParameterMode::Value)
        .filter_map(|parameter| parameter.rust_ty.as_ref())
        .collect::<Vec<_>>();
    let args_tuple = match value_types.as_slice() {
        [] => quote! { () },
        [ty] => quote! { (#ty,) },
        types => quote! { (#(#types),*) },
    };
    let expected = value_types.len();
    let mut runtime_index = 0_usize;
    let async_bindings = signature
        .parameters
        .iter()
        .filter_map(|parameter| {
            if parameter.mode == ParameterMode::HiddenContext {
                return None;
            }
            let index = runtime_index;
            runtime_index += 1;
            let name = format_ident!("__vela_arg_{}", parameter.name);
            let ty = parameter
                .rust_ty
                .as_ref()
                .expect("value parameters retain their Rust type");
            Some(quote! {
                let #name = <#ty as ::vela_engine::args::FromScriptArg>::from_script_arg(
                    &args[#index],
                )?;
            })
        })
        .collect::<Vec<_>>();
    let async_call_args = signature
        .parameters
        .iter()
        .map(|parameter| {
            if parameter.mode == ParameterMode::HiddenContext {
                quote! { __vela_context }
            } else {
                let name = format_ident!("__vela_arg_{}", parameter.name);
                quote! { #name }
            }
        })
        .collect::<Vec<_>>();
    let registration = match (signature.is_async, signature.has_hidden_context()) {
        (false, true) => quote! {
            builder.register_typed_context_host_native_fn::<#args_tuple, _>(
                #contract_ident().native_function_desc(), #function_ident,
            )
        },
        (false, false) => quote! {
            builder.register_typed_native_fn::<#args_tuple, _>(
                #contract_ident().native_function_desc(), #function_ident,
            )
        },
        (true, true) => quote! {
            builder.register_async_context_fn(
                #contract_ident().native_function_desc(),
                move |args, __vela_context| {
                    ::std::boxed::Box::pin(async move {
                        if args.len() != #expected {
                            return Err(::vela_vm::error::VmError::new(
                                ::vela_vm::error::VmErrorKind::ArityMismatch {
                                    name: #contract_ident().public_path,
                                    expected: #expected,
                                    actual: args.len(),
                                },
                            ));
                        }
                        #(#async_bindings)*
                        let __vela_result = #function_ident(#(#async_call_args),*).await;
                        ::vela_engine::typed::IntoNativeReturn::into_native_return(__vela_result)
                    })
                },
            )
        },
        (true, false) => quote! {
            builder.register_async_fn(
                #contract_ident().native_function_desc(),
                move |args| {
                    ::std::boxed::Box::pin(async move {
                        if args.len() != #expected {
                            return Err(::vela_vm::error::VmError::new(
                                ::vela_vm::error::VmErrorKind::ArityMismatch {
                                    name: #contract_ident().public_path,
                                    expected: #expected,
                                    actual: args.len(),
                                },
                            ));
                        }
                        #(#async_bindings)*
                        let __vela_result = #function_ident(#(#async_call_args),*).await;
                        ::vela_engine::typed::IntoNativeReturn::into_native_return(__vela_result)
                    })
                },
            )
        },
    };

    Some(quote! {
        #[doc(hidden)]
        #[must_use]
        pub fn #register_ident(
            builder: ::vela_engine::builder::EngineBuilder,
        ) -> ::vela_engine::builder::EngineBuilder {
            #registration
        }

        #[must_use]
        pub fn #bundle_ident() -> ::vela_engine::interop::ExportBundle {
            ::vela_engine::interop::ExportBundle::new(
                vec![#contract_ident()],
                #register_ident,
            )
        }
    })
}

fn function_sync_scoped_host_adapter(
    item: &ItemFn,
    signature: &ClassifiedSignature,
) -> TokenStream {
    let collection_registrations = collection_registration_tokens(signature);
    let function_ident = &item.sig.ident;
    let contract_ident = format_ident!("vela_callable_contract_{function_ident}");
    let register_ident = format_ident!("vela_register_export_{function_ident}");
    let bundle_ident = format_ident!("vela_export_bundle_{function_ident}");
    let expected = signature.parameters.len();
    let ReturnMode::ScopedHost { child, .. } = signature.returns.mode else {
        unreachable!("scoped adapter requires a scoped host return");
    };
    let child_kind = match child {
        HostAccess::Shared => quote! { ::vela_host::lease::HostLeaseKind::Shared },
        HostAccess::Exclusive => quote! { ::vela_host::lease::HostLeaseKind::Exclusive },
    };
    let container = signature
        .scoped_return_container()
        .expect("scoped adapter has a supported return container");
    let child_shape = match &signature.returns.ty {
        shape if shape.host_boundary().is_some() => shape,
        TypeShape::Tuple(_) => &signature.returns.ty,
        TypeShape::Option(inner) => &**inner,
        TypeShape::Result(ok, _) => &**ok,
        _ => unreachable!(),
    };
    let child_shapes = match child_shape {
        shape if shape.host_boundary().is_some() => vec![shape],
        TypeShape::Tuple(elements) => elements.iter().collect::<Vec<_>>(),
        _ => unreachable!("scoped adapter requires host payloads"),
    };
    let request_plans = signature
        .parameters
        .iter()
        .enumerate()
        .filter_map(|(index, parameter)| {
            let (_, access) = parameter.ty.host_boundary()?;
            let type_id = host_type_id_tokens(&parameter.ty)
                .expect("host parameter has a runtime type identity");
            let kind = match access {
                HostAccess::Shared => quote! { ::vela_host::lease::HostLeaseKind::Shared },
                HostAccess::Exclusive => {
                    quote! { ::vela_host::lease::HostLeaseKind::Exclusive }
                }
            };
            Some(quote! {
                ::vela_engine::interop::HostLeaseParameterPlan::argument(
                    #index,
                    #index,
                    #type_id,
                    #kind,
                )
            })
        })
        .collect::<Vec<_>>();
    let argument_bindings = signature
        .parameters
        .iter()
        .enumerate()
        .filter_map(|(index, parameter)| {
            if parameter.ty.host_boundary().is_some() {
                return None;
            }
            let name = format_ident!("__vela_arg_{}", parameter.name);
            let ty = parameter
                .rust_ty
                .as_ref()
                .expect("value parameters retain their Rust type");
            Some(quote! {
                let #name = <#ty as ::vela_engine::args::FromScriptArg>::from_script_arg(
                    &args[#index],
                )?;
            })
        })
        .collect::<Vec<_>>();
    let host_parameter = signature
        .parameters
        .iter()
        .find(|parameter| parameter.ty.host_boundary().is_some())
        .expect("scoped free return has one host origin");
    let host_name = format_ident!("__vela_arg_{}", host_parameter.name);
    let (_, host_access) = host_parameter
        .ty
        .host_boundary()
        .expect("scoped free return origin is host-backed");
    let host_binding = match host_access {
        HostAccess::Shared => {
            let value = shared_host_value_tokens(
                &host_parameter.ty,
                quote! { __vela_parent_lease.object() },
            );
            quote! {
                let #host_name = #value
                    .expect("preflight-validated scoped shared owner");
            }
        }
        HostAccess::Exclusive => {
            let value = exclusive_host_value_tokens(
                &host_parameter.ty,
                quote! { __vela_parent_lease.object_mut().expect("exclusive parent lease") },
            );
            quote! {
                let #host_name = #value
                    .expect("preflight-validated scoped exclusive owner");
            }
        }
    };
    let argument_names = signature
        .parameters
        .iter()
        .map(|parameter| format_ident!("__vela_arg_{}", parameter.name))
        .collect::<Vec<_>>();
    let child_names = child_shapes
        .iter()
        .enumerate()
        .map(|(index, _)| format_ident!("__vela_child_{index}"))
        .collect::<Vec<_>>();
    let child_references = child_shapes
        .iter()
        .map(|shape| {
            let (ty, access) = shape
                .host_boundary()
                .expect("borrowed tuple contains only direct host references");
            match access {
                HostAccess::Shared => quote! { &#ty },
                HostAccess::Exclusive => quote! { &mut #ty },
            }
        })
        .collect::<Vec<_>>();
    let wrapped_children = child_shapes
        .iter()
        .zip(&child_names)
        .map(|(shape, name)| {
            let (_, access) = shape
                .host_boundary()
                .expect("borrowed child is host-backed");
            match access {
                HostAccess::Shared => {
                    let type_id = host_type_id_tokens(shape)
                        .expect("borrowed child has a runtime type identity");
                    quote! {
                        ::vela_host::lease::shared_scoped_host_with_type_id(#name, #type_id)
                    }
                }
                HostAccess::Exclusive => {
                    let type_id = host_type_id_tokens(shape)
                        .expect("borrowed child has a runtime type identity");
                    quote! {
                        ::vela_host::lease::exclusive_scoped_host_with_type_id(#name, #type_id)
                    }
                }
            }
        })
        .collect::<Vec<_>>();
    let is_group = matches!(child_shape, TypeShape::Tuple(_));
    let success_conversion = if is_group {
        quote! {
            let (#(#child_names,)*): (#(#child_references,)*) = __vela_success;
            Ok(vec![#(#wrapped_children),*])
        }
    } else {
        let child_name = &child_names[0];
        let child_reference = &child_references[0];
        let wrapped_child = &wrapped_children[0];
        quote! {
            let #child_name: #child_reference = __vela_success;
            Ok(#wrapped_child)
        }
    };
    let cell_constructor = if is_group {
        quote! { ::vela_host::lease::try_scoped_host_group_cell }
    } else {
        quote! { ::vela_host::lease::try_scoped_host_cell }
    };
    let scoped_return = if is_group {
        let accesses = child_shapes.iter().map(|_| &child_kind);
        quote! {
            ::vela_host::adapter::ScopedHostReturnGroup {
                object: __vela_object,
                accesses: vec![#(#accesses),*],
            }
        }
    } else {
        quote! {
            ::vela_host::adapter::ScopedHostReturn {
                object: __vela_object,
                access: #child_kind,
            }
        }
    };
    let direct_outcome = if is_group {
        quote! { Tuple }
    } else {
        quote! { Direct }
    };
    let option_outcome = if is_group {
        quote! { OptionSomeTuple }
    } else {
        quote! { OptionSome }
    };
    let result_outcome = if is_group {
        quote! { ResultOkTuple }
    } else {
        quote! { ResultOk }
    };
    let outcome = match (container, signature.returns.error_mode) {
        (ScopedReturnContainer::Direct, ErrorMode::Value) => quote! {
            let __vela_object = ::vela_engine::interop::catch_export_panic(
                &__vela_callable,
                || #cell_constructor(
                    __vela_parent,
                    move |__vela_parent_lease| {
                        #host_binding
                        let __vela_success = #function_ident(#(#argument_names),*);
                        #success_conversion
                    },
                ),
            )?;
            Ok(::vela_engine::native::ScopedHostNativeOutcome::#direct_outcome(#scoped_return))
        },
        (ScopedReturnContainer::Direct, ErrorMode::RuntimeResult) => quote! {
            let __vela_object = ::vela_engine::interop::catch_export_panic(
                &__vela_callable,
                || Ok(#cell_constructor(
                    __vela_parent,
                    move |__vela_parent_lease| {
                        #host_binding
                        match #function_ident(#(#argument_names),*) {
                            Ok(__vela_success) => { #success_conversion },
                            Err(__vela_error) => Err(__vela_error),
                        }
                    },
                )),
            )??;
            Ok(::vela_engine::native::ScopedHostNativeOutcome::#direct_outcome(#scoped_return))
        },
        (ScopedReturnContainer::Option, ErrorMode::Value) => quote! {
            let __vela_built = ::vela_engine::interop::catch_export_panic(
                &__vela_callable,
                || Ok(#cell_constructor(
                    __vela_parent,
                    move |__vela_parent_lease| {
                        #host_binding
                        match #function_ident(#(#argument_names),*) {
                            Some(__vela_success) => { #success_conversion },
                            None => Err(()),
                        }
                    },
                )),
            )?;
            match __vela_built {
                Ok(__vela_object) => Ok(
                    ::vela_engine::native::ScopedHostNativeOutcome::#option_outcome(#scoped_return),
                ),
                Err(()) => Ok(::vela_engine::native::ScopedHostNativeOutcome::Value(
                    ::vela_vm::owned_value::OwnedValue::enum_variant(
                        "Option",
                        "None",
                        ::std::iter::empty::<(
                            &'static str,
                            ::vela_vm::owned_value::OwnedValue,
                        )>(),
                    ),
                )),
            }
        },
        (ScopedReturnContainer::Result, ErrorMode::Value) => quote! {
            let __vela_built = ::vela_engine::interop::catch_export_panic(
                &__vela_callable,
                || Ok(#cell_constructor(
                    __vela_parent,
                    move |__vela_parent_lease| {
                        #host_binding
                        match #function_ident(#(#argument_names),*) {
                            Ok(__vela_success) => { #success_conversion },
                            Err(__vela_error) => Err(__vela_error),
                        }
                    },
                )),
            )?;
            match __vela_built {
                Ok(__vela_object) => Ok(
                    ::vela_engine::native::ScopedHostNativeOutcome::#result_outcome(#scoped_return),
                ),
                Err(__vela_error) => Ok(::vela_engine::native::ScopedHostNativeOutcome::Value(
                    ::vela_vm::owned_value::OwnedValue::enum_variant(
                        "Result",
                        "Err",
                        [(
                            "0",
                            ::vela_engine::args::IntoScriptArg::into_script_arg(__vela_error),
                        )],
                    ),
                )),
            }
        },
        (_, ErrorMode::RuntimeResult) => {
            unreachable!("VmResult is unwrapped before return-container classification")
        }
    };

    quote! {
        #[doc(hidden)]
        #[must_use]
        pub fn #register_ident(
            builder: ::vela_engine::builder::EngineBuilder,
        ) -> ::vela_engine::builder::EngineBuilder {
            #(#collection_registrations)*
            let __vela_contract = #contract_ident();
            let __vela_desc = __vela_contract.native_function_desc();
            let __vela_plan = ::vela_engine::interop::PreparedHostLeasePlan::new(
                __vela_contract,
                #expected,
                [#(#request_plans),*],
            );
            let __vela_callable = __vela_plan.callable().to_owned();
            builder.register_scoped_host_fn(
                __vela_desc,
                move |args| __vela_plan.prepare(args),
                move |leases, args| {
                    if args.len() != #expected {
                        return Err(::vela_vm::error::VmError::new(
                            ::vela_vm::error::VmErrorKind::ArityMismatch {
                                name: __vela_callable.clone(),
                                expected: #expected,
                                actual: args.len(),
                            },
                        ));
                    }
                    #(#argument_bindings)*
                    let __vela_parent = leases
                        .first_mut()
                        .expect("preflight emits the borrowed return owner")
                        .take();
                    #outcome
                },
            )
        }

        #[must_use]
        pub fn #bundle_ident() -> ::vela_engine::interop::ExportBundle {
            ::vela_engine::interop::ExportBundle::new(
                vec![#contract_ident()],
                #register_ident,
            )
        }
    }
}

fn function_async_host_adapter(item: &ItemFn, signature: &ClassifiedSignature) -> TokenStream {
    let collection_registrations = collection_registration_tokens(signature);
    let function_ident = &item.sig.ident;
    let contract_ident = format_ident!("vela_callable_contract_{function_ident}");
    let register_ident = format_ident!("vela_register_export_{function_ident}");
    let bundle_ident = format_ident!("vela_export_bundle_{function_ident}");
    let expected = signature.parameters.len();
    let request_plans = signature
        .parameters
        .iter()
        .enumerate()
        .filter_map(|(index, parameter)| {
            let (_, access) = parameter.ty.host_boundary()?;
            let type_id = host_type_id_tokens(&parameter.ty)
                .expect("host parameter has a runtime type identity");
            let kind = match access {
                HostAccess::Shared => quote! { ::vela_host::lease::HostLeaseKind::Shared },
                HostAccess::Exclusive => {
                    quote! { ::vela_host::lease::HostLeaseKind::Exclusive }
                }
            };
            Some(quote! {
                ::vela_engine::interop::HostLeaseParameterPlan::argument(
                    #index,
                    #index,
                    #type_id,
                    #kind,
                )
            })
        })
        .collect::<Vec<_>>();
    let argument_bindings = signature
        .parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            let name = format_ident!("__vela_arg_{}", parameter.name);
            match parameter.ty.host_boundary() {
                Some((_, HostAccess::Shared)) => {
                    let value = shared_host_value_tokens(&parameter.ty, quote! { lease.object() });
                    quote! {
                        let #name = __vela_leases
                            .next()
                            .and_then(|lease| #value)
                            .expect("preflight-validated shared host lease");
                    }
                }
                Some((_, HostAccess::Exclusive)) => {
                    let value = exclusive_host_value_tokens(&parameter.ty, quote! { object });
                    quote! {
                        let #name = __vela_leases
                            .next()
                            .and_then(|lease| lease.object_mut())
                            .and_then(|object| #value)
                            .expect("preflight-validated exclusive host lease");
                    }
                }
                None => {
                    let ty = parameter
                        .rust_ty
                        .as_ref()
                        .expect("value parameters retain their Rust type");
                    quote! {
                        let #name = <#ty as ::vela_engine::args::FromScriptArg>::from_script_arg(
                            &args[#index],
                        )?;
                    }
                }
            }
        })
        .collect::<Vec<_>>();
    let argument_names = signature
        .parameters
        .iter()
        .map(|parameter| format_ident!("__vela_arg_{}", parameter.name))
        .collect::<Vec<_>>();

    quote! {
        #[doc(hidden)]
        #[must_use]
        pub fn #register_ident(
            builder: ::vela_engine::builder::EngineBuilder,
        ) -> ::vela_engine::builder::EngineBuilder {
            #(#collection_registrations)*
            let __vela_contract = #contract_ident();
            let __vela_desc = __vela_contract.native_function_desc();
            let __vela_plan = ::vela_engine::interop::PreparedHostLeasePlan::new(
                __vela_contract,
                #expected,
                [#(#request_plans),*],
            );
            let __vela_callable = __vela_plan.callable().to_owned();
            builder.register_async_direct_host_fn(
                __vela_desc,
                move |args| __vela_plan.prepare(args),
                move |leases, args| {
                    let __vela_callable = __vela_callable.clone();
                    ::std::boxed::Box::pin(async move {
                        if args.len() != #expected {
                            return Err(::vela_vm::error::VmError::new(
                                ::vela_vm::error::VmErrorKind::ArityMismatch {
                                    name: __vela_callable,
                                    expected: #expected,
                                    actual: args.len(),
                                },
                            ));
                        }
                        let mut __vela_leases = leases.iter_mut();
                        #(#argument_bindings)*
                        let __vela_result = #function_ident(#(#argument_names),*).await;
                        ::vela_engine::typed::IntoNativeReturn::into_native_return(__vela_result)
                    })
                },
            )
        }

        #[must_use]
        pub fn #bundle_ident() -> ::vela_engine::interop::ExportBundle {
            ::vela_engine::interop::ExportBundle::new(
                vec![#contract_ident()],
                #register_ident,
            )
        }
    }
}

fn function_sync_host_adapter(item: &ItemFn, signature: &ClassifiedSignature) -> TokenStream {
    let collection_registrations = collection_registration_tokens(signature);
    let function_ident = &item.sig.ident;
    let contract_ident = format_ident!("vela_callable_contract_{function_ident}");
    let register_ident = format_ident!("vela_register_export_{function_ident}");
    let bundle_ident = format_ident!("vela_export_bundle_{function_ident}");
    let expected = signature
        .parameters
        .iter()
        .filter(|parameter| parameter.mode != ParameterMode::HiddenContext)
        .count();
    let mut runtime_argument_index = 0_usize;
    let request_plans = signature
        .parameters
        .iter()
        .enumerate()
        .filter_map(|(contract_index, parameter)| {
            if parameter.mode == ParameterMode::HiddenContext {
                return None;
            }
            let argument_index = runtime_argument_index;
            runtime_argument_index += 1;
            let (_, access) = parameter.ty.host_boundary()?;
            let type_id = host_type_id_tokens(&parameter.ty)
                .expect("host parameter has a runtime type identity");
            let kind = match access {
                HostAccess::Shared => quote! { ::vela_host::lease::HostLeaseKind::Shared },
                HostAccess::Exclusive => {
                    quote! { ::vela_host::lease::HostLeaseKind::Exclusive }
                }
            };
            Some(quote! {
                ::vela_engine::interop::HostLeaseParameterPlan::argument(
                    #contract_index,
                    #argument_index,
                    #type_id,
                    #kind,
                )
            })
        })
        .collect::<Vec<_>>();
    let mut host_lease_index = 0_usize;
    let mut runtime_argument_index = 0_usize;
    let argument_bindings = signature
        .parameters
        .iter()
        .map(|parameter| {
            let name = format_ident!("__vela_arg_{}", parameter.name);
            if parameter.mode == ParameterMode::HiddenContext {
                return quote! { let #name = &mut *__vela_context; };
            }
            let argument_index = runtime_argument_index;
            runtime_argument_index += 1;
            match parameter.ty.host_boundary() {
                Some((_, HostAccess::Shared)) => {
                    let lease_index = host_lease_index;
                    host_lease_index += 1;
                    let value = shared_host_value_tokens(&parameter.ty, quote! { lease.object() });
                    quote! {
                        let #name = __vela_leases
                            .next()
                            .and_then(|lease| #value)
                            .ok_or_else(|| ::vela_host::lease::host_lease_unsupported(
                                __vela_lease_requests[#lease_index].0,
                            ))?;
                    }
                }
                Some((_, HostAccess::Exclusive)) => {
                    let lease_index = host_lease_index;
                    host_lease_index += 1;
                    let value = exclusive_host_value_tokens(&parameter.ty, quote! { object });
                    quote! {
                        let #name = __vela_leases
                            .next()
                            .and_then(|lease| lease.object_mut())
                            .and_then(|object| #value)
                            .ok_or_else(|| ::vela_host::lease::host_lease_unsupported(
                                __vela_lease_requests[#lease_index].0,
                            ))?;
                    }
                }
                None => {
                    let ty = parameter
                        .rust_ty
                        .as_ref()
                        .expect("value parameters retain their Rust type");
                    quote! {
                        let #name = <#ty as ::vela_engine::args::FromScriptArg>::from_script_arg(
                            &args[#argument_index],
                        )?;
                    }
                }
            }
        })
        .collect::<Vec<_>>();
    let argument_names = signature
        .parameters
        .iter()
        .map(|parameter| format_ident!("__vela_arg_{}", parameter.name))
        .collect::<Vec<_>>();
    let invocation = quote! {
        let mut __vela_leases = __vela_erased_leases.iter_mut();
        #(#argument_bindings)*
        ::vela_engine::interop::catch_export_panic(
            &__vela_callable,
            || ::vela_engine::typed::IntoNativeReturn::into_native_return(
                #function_ident(#(#argument_names),*)
            ),
        )
    };
    let registration = if signature.has_hidden_context() {
        quote! {
            builder.register_context_host_native_fn(
                __vela_desc,
                move |args, __vela_outer_context| {
                    let __vela_lease_requests = __vela_plan.prepare(args)?;
                    __vela_outer_context.with_host_leases(
                        &__vela_lease_requests,
                        |__vela_erased_leases, __vela_context| {
                            #invocation
                        },
                    )
                },
            )
        }
    } else {
        quote! {
            builder.register_host_native_fn(__vela_desc, move |args, host| {
                let __vela_lease_requests = __vela_plan.prepare(args)?;
                let mut __vela_result = None;
                host.adapter.with_host_leases(
                    &__vela_lease_requests,
                    &mut |__vela_erased_leases, _leased_adapter| {
                        __vela_result = Some((|| -> ::vela_vm::error::VmResult<
                            ::vela_vm::owned_value::OwnedValue
                        > {
                            #invocation
                        })());
                        Ok(())
                    },
                )?;
                __vela_result.expect("host lease callback must run exactly once")
            })
        }
    };

    quote! {
        #[doc(hidden)]
        #[must_use]
        pub fn #register_ident(
            builder: ::vela_engine::builder::EngineBuilder,
        ) -> ::vela_engine::builder::EngineBuilder {
            #(#collection_registrations)*
            let __vela_contract = #contract_ident();
            let __vela_desc = __vela_contract.native_function_desc();
            let __vela_callable = __vela_contract.public_path.clone();
            let __vela_plan = ::vela_engine::interop::PreparedHostLeasePlan::new(
                __vela_contract,
                #expected,
                [#(#request_plans),*],
            );
            #registration
        }

        #[must_use]
        pub fn #bundle_ident() -> ::vela_engine::interop::ExportBundle {
            ::vela_engine::interop::ExportBundle::new(
                vec![#contract_ident()],
                #register_ident,
            )
        }
    }
}

mod method;

pub(crate) use method::{method_adapter, method_contract};

pub(crate) fn protocol_contract(
    trait_ident: &syn::Ident,
    public_path: &str,
    docs: Option<&str>,
    methods: &[(&syn::TraitItemFn, ClassifiedSignature)],
) -> TokenStream {
    let contract_ident = format_ident!("vela_protocol_contract_{trait_ident}");
    let method_contracts = methods.iter().map(|(method, signature)| {
        let method_ident = &method.sig.ident;
        let method_path = format!("{public_path}::{method_ident}");
        let callable_id = u128::from(vela_common::stable_id(
            "rust_trait_method_export",
            public_path,
            &method_ident.to_string(),
        ));
        let parameters = signature.parameters.iter().map(|parameter| {
            let name = &parameter.name;
            let identity = vela_common::stable_id("callable_parameter", &method_path, name);
            let hint = if matches!(parameter.ty, TypeShape::ReceiverHost) {
                quote! { ::vela_engine::native::TypeHint::Trait(#public_path.to_owned()) }
            } else {
                hint_tokens(&parameter.ty)
            };
            let mode = parameter_mode_tokens(parameter.mode);
            let contract = quote! {
                ::vela_engine::interop::CallableParameter::new(#identity, #name, #hint, #mode)
            };
            binding_use_tokens(contract, &parameter.ty)
        });
        let return_hint = hint_tokens(&signature.returns.ty);
        let return_mode = return_mode_tokens(signature.returns.mode, &signature.returns.ty);
        let error_mode = match signature.returns.error_mode {
            ErrorMode::Value => quote! { ::vela_engine::interop::ErrorMode::Value },
            ErrorMode::RuntimeResult => quote! { ::vela_engine::interop::ErrorMode::RuntimeResult },
        };
        let return_contract = binding_use_tokens(
            quote! {
                ::vela_engine::interop::CallableReturn::new(
                    #return_hint,
                    #return_mode,
                    #error_mode,
                )
            },
            &signature.returns.ty,
        );
        let effects = effect_tokens(&signature.effects);
        let asyncness = if signature.is_async {
            quote! { ::vela_common::CallableAsyncness::Async }
        } else {
            quote! { ::vela_common::CallableAsyncness::Sync }
        };
        let method_docs = crate::signature::docs_from_attrs(&method.attrs)
            .map_or_else(|| quote! { None }, |docs| quote! { Some(#docs.to_owned()) });
        quote! {
            ::vela_engine::interop::CallableContract {
                identity: ::vela_engine::interop::CallableIdentity::new(
                    ::vela_engine::interop::CallableKind::RustTraitMethod,
                    #callable_id,
                ),
                public_path: #method_path.to_owned(),
                parameters: vec![#(#parameters),*],
                returns: #return_contract,
                asyncness: #asyncness,
                effects: #effects,
                access: ::vela_engine::interop::CallableAccess::default(),
                docs: #method_docs,
                origin: ::vela_engine::interop::CallableOrigin {
                    language: ::vela_engine::interop::CallableLanguage::Rust,
                    source_span: None,
                },
            }
        }
    });
    let docs = docs.map_or_else(|| quote! { None }, |docs| quote! { Some(#docs.to_owned()) });

    quote! {
        #[doc(hidden)]
        #[must_use]
        pub fn #contract_ident() -> ::vela_engine::interop::VelaProtocolContract {
            ::vela_engine::interop::VelaProtocolContract {
                identity: ::vela_engine::interop::VelaProtocolIdentity::new(#public_path),
                methods: vec![#(#method_contracts),*],
                docs: #docs,
                origin: ::vela_engine::interop::CallableOrigin {
                    language: ::vela_engine::interop::CallableLanguage::Rust,
                    source_span: None,
                },
            }
        }
    }
}

pub(crate) fn parameter_mode_tokens(mode: ParameterMode) -> TokenStream {
    match mode {
        ParameterMode::Value => quote! { ::vela_engine::interop::BoundaryMode::Value },
        ParameterMode::ReadOnlyValueBorrow => {
            quote! { ::vela_engine::interop::BoundaryMode::ReadOnlyValueBorrow }
        }
        ParameterMode::StorageDirectedShared => {
            quote! { ::vela_engine::interop::BoundaryMode::StorageDirectedShared }
        }
        ParameterMode::SharedHost => quote! { ::vela_engine::interop::BoundaryMode::SharedHost },
        ParameterMode::ExclusiveHost => {
            quote! { ::vela_engine::interop::BoundaryMode::ExclusiveHost }
        }
        ParameterMode::HiddenContext => {
            quote! { ::vela_engine::interop::BoundaryMode::HiddenContext }
        }
    }
}

pub(crate) fn return_mode_tokens(mode: ReturnMode, shape: &TypeShape) -> TokenStream {
    match mode {
        ReturnMode::Owned => quote! { ::vela_engine::interop::ReturnMode::OwnedValue },
        ReturnMode::Structured => quote! { ::vela_engine::interop::ReturnMode::StructuredValue },
        ReturnMode::Boundary => {
            let TypeShape::Value(ty) = shape else {
                unreachable!("boundary return mode requires a value boundary type");
            };
            quote! { <#ty as ::vela_engine::interop::VelaValueBoundary>::vela_return_mode() }
        }
        ReturnMode::ScopedHost {
            origin,
            child,
            parent,
        } => {
            let origin = match origin {
                BorrowOrigin::Receiver => {
                    quote! { ::vela_engine::interop::BorrowedReturnOrigin::Receiver }
                }
                BorrowOrigin::Parameter(index) => {
                    quote! { ::vela_engine::interop::BorrowedReturnOrigin::Parameter(#index) }
                }
            };
            let child_access = host_access_tokens(child);
            let parent_freeze = host_access_tokens(parent);
            quote! {
                ::vela_engine::interop::ReturnMode::ScopedHost {
                    origin: #origin,
                    child_access: #child_access,
                    parent_freeze: #parent_freeze,
                }
            }
        }
    }
}

fn host_access_tokens(access: HostAccess) -> TokenStream {
    match access {
        HostAccess::Shared => quote! { ::vela_engine::interop::ScopedHostAccess::Shared },
        HostAccess::Exclusive => quote! { ::vela_engine::interop::ScopedHostAccess::Exclusive },
    }
}

pub(crate) fn effect_tokens(effects: &std::collections::BTreeSet<EffectName>) -> TokenStream {
    let mut tokens = quote! { ::vela_engine::native::EffectSet::pure() };
    for effect in effects {
        let next = match effect {
            EffectName::Pure => continue,
            EffectName::HostRead => quote! { ::vela_engine::native::EffectSet::host_read() },
            EffectName::HostWrite => quote! { ::vela_engine::native::EffectSet::host_write() },
            EffectName::EventEmit => quote! { ::vela_engine::native::EffectSet::event_emit() },
            EffectName::Time => quote! { ::vela_engine::native::EffectSet::time() },
            EffectName::Random => quote! { ::vela_engine::native::EffectSet::random() },
            EffectName::IoRead => quote! { ::vela_engine::native::EffectSet::io_read() },
            EffectName::IoWrite => quote! { ::vela_engine::native::EffectSet::io_write() },
            EffectName::ReflectionRead => {
                quote! { ::vela_engine::native::EffectSet::reflection_read() }
            }
            EffectName::ReflectionWrite => {
                quote! { ::vela_engine::native::EffectSet::reflection_write() }
            }
            EffectName::ReflectionCall => {
                quote! { ::vela_engine::native::EffectSet::reflection_call() }
            }
        };
        tokens = quote! { #tokens.union(#next) };
    }
    tokens
}
