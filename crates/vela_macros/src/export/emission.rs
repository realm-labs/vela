use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{ImplItemFn, ItemFn};

use super::attrs::ExportAttrs;
use super::signature::{
    BorrowOrigin, ClassifiedSignature, EffectName, ErrorMode, HostAccess, ParameterMode,
    ReturnMode, TypeShape,
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
            quote! {
                ::vela_engine::interop::CallableParameter::new(#identity, #name, #hint, #mode)
            }
        });
    let return_hint = hint_tokens(&signature.returns.ty);
    let return_mode = return_mode_tokens(signature.returns.mode);
    let error_mode = match signature.returns.error_mode {
        ErrorMode::Value => quote! { ::vela_engine::interop::ErrorMode::Value },
        ErrorMode::RuntimeResult => quote! { ::vela_engine::interop::ErrorMode::RuntimeResult },
    };
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
                returns: ::vela_engine::interop::CallableReturn::new(
                    #return_hint,
                    #return_mode,
                    #error_mode,
                ),
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
    let TypeShape::Host(child_ty, _) = &signature.returns.ty else {
        unreachable!("direct scoped adapter requires a direct host return");
    };
    let request_bindings = signature
        .parameters
        .iter()
        .enumerate()
        .filter_map(|(index, parameter)| {
            let TypeShape::Host(ty, access) = &parameter.ty else {
                return None;
            };
            let kind = match access {
                HostAccess::Shared => quote! { ::vela_host::lease::HostLeaseKind::Shared },
                HostAccess::Exclusive => {
                    quote! { ::vela_host::lease::HostLeaseKind::Exclusive }
                }
            };
            Some(quote! {
                ::vela_engine::interop::HostParamLeaseRequest::from_argument(
                    &__vela_contract,
                    #index,
                    #index,
                    <#ty as ::vela_engine::interop::VelaHostBoundary>::vela_host_type_id(),
                    #kind,
                    &args[#index],
                )?
            })
        })
        .collect::<Vec<_>>();
    let argument_bindings = signature
        .parameters
        .iter()
        .enumerate()
        .filter_map(|(index, parameter)| {
            if matches!(parameter.ty, TypeShape::Host(_, _)) {
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
        .find(|parameter| matches!(parameter.ty, TypeShape::Host(_, _)))
        .expect("scoped free return has one host origin");
    let host_name = format_ident!("__vela_arg_{}", host_parameter.name);
    let TypeShape::Host(host_ty, host_access) = &host_parameter.ty else {
        unreachable!();
    };
    let host_binding = match host_access {
        HostAccess::Shared => quote! {
            let #host_name = __vela_parent_lease
                .object()
                .lease_any()
                .and_then(|object| object.downcast_ref::<#host_ty>())
                .ok_or_else(|| ::vela_host::lease::host_lease_unsupported(__vela_owner))?;
        },
        HostAccess::Exclusive => quote! {
            let #host_name = __vela_parent_lease
                .object_mut()
                .and_then(|object| object.lease_any_mut())
                .and_then(|object| object.downcast_mut::<#host_ty>())
                .ok_or_else(|| ::vela_host::lease::host_lease_unsupported(__vela_owner))?;
        },
    };
    let argument_names = signature
        .parameters
        .iter()
        .map(|parameter| format_ident!("__vela_arg_{}", parameter.name))
        .collect::<Vec<_>>();
    let wrap_child = match child {
        HostAccess::Shared => quote! { ::vela_host::lease::shared_scoped_host(__vela_child) },
        HostAccess::Exclusive => {
            quote! { ::vela_host::lease::exclusive_scoped_host(__vela_child) }
        }
    };
    let child_reference = match child {
        HostAccess::Shared => quote! { &#child_ty },
        HostAccess::Exclusive => quote! { &mut #child_ty },
    };

    quote! {
        #[doc(hidden)]
        #[must_use]
        pub fn #register_ident(
            builder: ::vela_engine::builder::EngineBuilder,
        ) -> ::vela_engine::builder::EngineBuilder {
            builder.register_scoped_host_fn(
                #contract_ident().native_function_desc(),
                move |args| {
                    let __vela_contract = #contract_ident();
                    if args.len() != #expected {
                        return Err(::vela_vm::error::VmError::new(
                            ::vela_vm::error::VmErrorKind::ArityMismatch {
                                name: __vela_contract.public_path,
                                expected: #expected,
                                actual: args.len(),
                            },
                        ));
                    }
                    let __vela_requests = vec![#(#request_bindings),*];
                    ::vela_engine::interop::preflight_host_parameter_leases(&__vela_requests)
                },
                move |leases, args| {
                    let __vela_callable = #contract_ident().public_path;
                    if args.len() != #expected {
                        return Err(::vela_vm::error::VmError::new(
                            ::vela_vm::error::VmErrorKind::ArityMismatch {
                                name: __vela_callable,
                                expected: #expected,
                                actual: args.len(),
                            },
                        ));
                    }
                    #(#argument_bindings)*
                    let __vela_owner = match args.iter().find_map(|argument| match argument {
                        ::vela_vm::owned_value::OwnedValue::HostRef(root) => Some(*root),
                        _ => None,
                    }) {
                        Some(root) => root,
                        None => return Err(::vela_vm::error::VmError::new(
                            ::vela_vm::error::VmErrorKind::TypeMismatch {
                                operation: "scoped host owner",
                            },
                        )),
                    };
                    let __vela_parent = leases
                        .first_mut()
                        .expect("preflight emits the borrowed return owner")
                        .take();
                    let __vela_object = ::vela_engine::interop::catch_export_panic(
                        &__vela_callable,
                        || ::vela_host::lease::try_scoped_host_cell(
                            __vela_parent,
                            move |__vela_parent_lease| {
                                #host_binding
                                let __vela_child: #child_reference = #function_ident(#(#argument_names),*);
                                Ok(#wrap_child)
                            },
                        ),
                    )?;
                    Ok(::vela_host::adapter::ScopedHostReturn {
                        object: __vela_object,
                        access: #child_kind,
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

fn function_async_host_adapter(item: &ItemFn, signature: &ClassifiedSignature) -> TokenStream {
    let function_ident = &item.sig.ident;
    let contract_ident = format_ident!("vela_callable_contract_{function_ident}");
    let register_ident = format_ident!("vela_register_export_{function_ident}");
    let bundle_ident = format_ident!("vela_export_bundle_{function_ident}");
    let expected = signature.parameters.len();
    let request_bindings = signature
        .parameters
        .iter()
        .enumerate()
        .filter_map(|(index, parameter)| {
            let TypeShape::Host(ty, access) = &parameter.ty else {
                return None;
            };
            let kind = match access {
                HostAccess::Shared => quote! { ::vela_host::lease::HostLeaseKind::Shared },
                HostAccess::Exclusive => {
                    quote! { ::vela_host::lease::HostLeaseKind::Exclusive }
                }
            };
            Some(quote! {
                ::vela_engine::interop::HostParamLeaseRequest::from_argument(
                    &__vela_contract,
                    #index,
                    #index,
                    <#ty as ::vela_engine::interop::VelaHostBoundary>::vela_host_type_id(),
                    #kind,
                    &args[#index],
                )?
            })
        })
        .collect::<Vec<_>>();
    let argument_bindings = signature
        .parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            let name = format_ident!("__vela_arg_{}", parameter.name);
            match &parameter.ty {
                TypeShape::Host(ty, HostAccess::Shared) => {
                    quote! {
                        let #name = __vela_leases
                            .next()
                            .and_then(|lease| lease.object().lease_any())
                            .and_then(|object| object.downcast_ref::<#ty>())
                            .expect("preflight-validated shared host lease");
                    }
                }
                TypeShape::Host(ty, HostAccess::Exclusive) => {
                    quote! {
                        let #name = __vela_leases
                            .next()
                            .and_then(|lease| lease.object_mut())
                            .and_then(|object| object.lease_any_mut())
                            .and_then(|object| object.downcast_mut::<#ty>())
                            .expect("preflight-validated exclusive host lease");
                    }
                }
                _ => {
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
            builder.register_async_direct_host_fn(
                #contract_ident().native_function_desc(),
                move |args| {
                    let __vela_contract = #contract_ident();
                    if args.len() != #expected {
                        return Err(::vela_vm::error::VmError::new(
                            ::vela_vm::error::VmErrorKind::ArityMismatch {
                                name: __vela_contract.public_path,
                                expected: #expected,
                                actual: args.len(),
                            },
                        ));
                    }
                    let __vela_requests = vec![#(#request_bindings),*];
                    ::vela_engine::interop::preflight_host_parameter_leases(&__vela_requests)
                },
                move |leases, args| {
                    let __vela_callable = #contract_ident().public_path;
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
    let request_bindings = signature
        .parameters
        .iter()
        .enumerate()
        .filter_map(|(contract_index, parameter)| {
            if parameter.mode == ParameterMode::HiddenContext {
                return None;
            }
            let argument_index = runtime_argument_index;
            runtime_argument_index += 1;
            let TypeShape::Host(ty, access) = &parameter.ty else {
                return None;
            };
            let kind = match access {
                HostAccess::Shared => quote! { ::vela_host::lease::HostLeaseKind::Shared },
                HostAccess::Exclusive => {
                    quote! { ::vela_host::lease::HostLeaseKind::Exclusive }
                }
            };
            Some(quote! {
                ::vela_engine::interop::HostParamLeaseRequest::from_argument(
                    &__vela_contract,
                    #contract_index,
                    #argument_index,
                    <#ty as ::vela_engine::interop::VelaHostBoundary>::vela_host_type_id(),
                    #kind,
                    &args[#argument_index],
                )?
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
            match &parameter.ty {
                TypeShape::Host(ty, HostAccess::Shared) => {
                    let lease_index = host_lease_index;
                    host_lease_index += 1;
                    quote! {
                        let #name = __vela_leases
                            .next()
                            .and_then(|lease| lease.object().lease_any())
                            .and_then(|object| object.downcast_ref::<#ty>())
                            .ok_or_else(|| ::vela_host::lease::host_lease_unsupported(
                                __vela_requests[#lease_index].canonical_host_identity,
                            ))?;
                    }
                }
                TypeShape::Host(ty, HostAccess::Exclusive) => {
                    let lease_index = host_lease_index;
                    host_lease_index += 1;
                    quote! {
                        let #name = __vela_leases
                            .next()
                            .and_then(|lease| lease.object_mut())
                            .and_then(|object| object.lease_any_mut())
                            .and_then(|object| object.downcast_mut::<#ty>())
                            .ok_or_else(|| ::vela_host::lease::host_lease_unsupported(
                                __vela_requests[#lease_index].canonical_host_identity,
                            ))?;
                    }
                }
                _ => {
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
            &__vela_contract.public_path,
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
                    if args.len() != #expected {
                        return Err(::vela_vm::error::VmError::new(
                            ::vela_vm::error::VmErrorKind::ArityMismatch {
                                name: __vela_contract.public_path.clone(),
                                expected: #expected,
                                actual: args.len(),
                            },
                        ));
                    }
                    let __vela_requests = vec![#(#request_bindings),*];
                    let __vela_lease_requests =
                        ::vela_engine::interop::preflight_host_parameter_leases(
                            &__vela_requests,
                        )?;
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
                if args.len() != #expected {
                    return Err(::vela_vm::error::VmError::new(
                        ::vela_vm::error::VmErrorKind::ArityMismatch {
                            name: __vela_contract.public_path.clone(),
                            expected: #expected,
                            actual: args.len(),
                        },
                    ));
                }
                let __vela_requests = vec![#(#request_bindings),*];
                let __vela_lease_requests =
                    ::vela_engine::interop::preflight_host_parameter_leases(&__vela_requests)?;
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
            let __vela_contract = #contract_ident();
            let __vela_desc = __vela_contract.native_function_desc();
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

pub(crate) fn method_contract(
    method: &ImplItemFn,
    self_ty: &syn::Type,
    owner_path: &str,
    docs: Option<&str>,
    signature: &ClassifiedSignature,
) -> TokenStream {
    let method_ident = &method.sig.ident;
    let contract_ident = format_ident!("vela_callable_contract_{method_ident}");
    let public_path = format!("{owner_path}::{method_ident}");
    let callable_id = u128::from(vela_common::stable_id(
        "rust_method_export",
        owner_path,
        &method_ident.to_string(),
    ));
    let parameters = signature.parameters.iter().map(|parameter| {
        let name = &parameter.name;
        let identity = vela_common::stable_id("callable_parameter", &public_path, name);
        let hint = if matches!(parameter.ty, TypeShape::ReceiverHost) {
            quote! { <#self_ty as ::vela_engine::interop::VelaHostBoundary>::vela_host_type_hint() }
        } else {
            hint_tokens(&parameter.ty)
        };
        let mode = parameter_mode_tokens(parameter.mode);
        quote! {
            ::vela_engine::interop::CallableParameter::new(#identity, #name, #hint, #mode)
        }
    });
    let return_hint = hint_tokens(&signature.returns.ty);
    let return_mode = return_mode_tokens(signature.returns.mode);
    let error_mode = match signature.returns.error_mode {
        ErrorMode::Value => quote! { ::vela_engine::interop::ErrorMode::Value },
        ErrorMode::RuntimeResult => quote! { ::vela_engine::interop::ErrorMode::RuntimeResult },
    };
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
                    ::vela_engine::interop::CallableKind::RustMethod,
                    #callable_id,
                ),
                public_path: #public_path.to_owned(),
                parameters: vec![#(#parameters),*],
                returns: ::vela_engine::interop::CallableReturn::new(
                    #return_hint,
                    #return_mode,
                    #error_mode,
                ),
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

pub(crate) fn method_adapter(
    method: &ImplItemFn,
    self_ty: &syn::Type,
    trait_path: Option<&syn::Path>,
    signature: &ClassifiedSignature,
) -> Option<TokenStream> {
    if signature.supports_sync_scoped_method_adapter() {
        return Some(method_sync_scoped_host_adapter(
            method, self_ty, trait_path, signature,
        ));
    }
    if signature.supports_async_method_adapter() {
        return Some(method_async_adapter(method, self_ty, trait_path, signature));
    }
    if !signature.supports_sync_method_adapter() {
        return None;
    }
    let method_ident = &method.sig.ident;
    let contract_ident = format_ident!("vela_callable_contract_{method_ident}");
    let register_ident = format_ident!("vela_register_export_{method_ident}");
    let receiver = signature
        .parameters
        .first()
        .expect("method classifier always emits a receiver");
    let receiver_kind = match receiver.mode {
        ParameterMode::SharedHost => quote! { ::vela_host::lease::HostLeaseKind::Shared },
        ParameterMode::ExclusiveHost => quote! { ::vela_host::lease::HostLeaseKind::Exclusive },
        _ => unreachable!("method receiver must be a host borrow"),
    };
    let receiver_binding = match receiver.mode {
        ParameterMode::SharedHost => quote! {
            let __vela_receiver = __vela_leases
                .next()
                .and_then(|lease| lease.object().lease_any())
                .and_then(|object| object.downcast_ref::<#self_ty>())
                .ok_or_else(|| ::vela_host::lease::host_lease_unsupported(receiver.root))?;
        },
        ParameterMode::ExclusiveHost => quote! {
            let __vela_receiver = __vela_leases
                .next()
                .and_then(|lease| lease.object_mut())
                .and_then(|object| object.lease_any_mut())
                .and_then(|object| object.downcast_mut::<#self_ty>())
                .ok_or_else(|| ::vela_host::lease::host_lease_unsupported(receiver.root))?;
        },
        _ => unreachable!("method receiver must be a host borrow"),
    };
    let expected = signature.parameters.len().saturating_sub(1);
    let mut runtime_argument_index = 0_usize;
    let additional_requests = signature
        .parameters
        .iter()
        .enumerate()
        .skip(1)
        .filter_map(|(contract_index, parameter)| {
            let argument_index = runtime_argument_index;
            runtime_argument_index += 1;
            let TypeShape::Host(ty, access) = &parameter.ty else {
                return None;
            };
            let kind = match access {
                HostAccess::Shared => quote! { ::vela_host::lease::HostLeaseKind::Shared },
                HostAccess::Exclusive => {
                    quote! { ::vela_host::lease::HostLeaseKind::Exclusive }
                }
            };
            Some(quote! {
                ::vela_engine::interop::HostParamLeaseRequest::from_argument(
                    &__vela_contract,
                    #contract_index,
                    #argument_index,
                    <#ty as ::vela_engine::interop::VelaHostBoundary>::vela_host_type_id(),
                    #kind,
                    &args[#argument_index],
                )?
            })
        })
        .collect::<Vec<_>>();
    let mut runtime_argument_index = 0_usize;
    let mut host_lease_index = 1_usize;
    let argument_bindings = signature
        .parameters
        .iter()
        .skip(1)
        .map(|parameter| {
            let argument_index = runtime_argument_index;
            runtime_argument_index += 1;
            let name = format_ident!("__vela_arg_{}", parameter.name);
            match &parameter.ty {
                TypeShape::Host(ty, HostAccess::Shared) => {
                    let lease_index = host_lease_index;
                    host_lease_index += 1;
                    quote! {
                        let #name = __vela_leases
                            .next()
                            .and_then(|lease| lease.object().lease_any())
                            .and_then(|object| object.downcast_ref::<#ty>())
                            .ok_or_else(|| ::vela_host::lease::host_lease_unsupported(
                                __vela_requests[#lease_index].canonical_host_identity,
                            ))?;
                    }
                }
                TypeShape::Host(ty, HostAccess::Exclusive) => {
                    let lease_index = host_lease_index;
                    host_lease_index += 1;
                    quote! {
                        let #name = __vela_leases
                            .next()
                            .and_then(|lease| lease.object_mut())
                            .and_then(|object| object.lease_any_mut())
                            .and_then(|object| object.downcast_mut::<#ty>())
                            .ok_or_else(|| ::vela_host::lease::host_lease_unsupported(
                                __vela_requests[#lease_index].canonical_host_identity,
                            ))?;
                    }
                }
                _ => {
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
        .skip(1)
        .map(|parameter| format_ident!("__vela_arg_{}", parameter.name))
        .collect::<Vec<_>>();
    let call_target = trait_path.map_or_else(
        || quote! { <#self_ty>::#method_ident },
        |trait_path| quote! { <#self_ty as #trait_path>::#method_ident },
    );

    Some(quote! {
        #[doc(hidden)]
        #[must_use]
        pub fn #register_ident(
            builder: ::vela_engine::builder::EngineBuilder,
        ) -> ::vela_engine::builder::EngineBuilder {
            let __vela_contract = Self::#contract_ident();
            let mut __vela_desc = __vela_contract.native_method_desc(
                <#self_ty as ::vela_engine::schema::ScriptHostSchema>::script_host_type_desc().key,
            );
            __vela_desc.id = ::vela_common::HostMethodId::new(
                ::core::primitive::u128::from(::vela_common::stable_id(
                    "host_method",
                    <#self_ty>::vela_stable_type_path(),
                    ::core::stringify!(#method_ident),
                )),
            );
            builder.register_native_method_fn(__vela_desc, move |receiver, args, host| {
                if !receiver.segments.is_empty() {
                    return Err(::vela_host::lease::host_lease_unsupported(receiver.root).into());
                }
                if args.len() != #expected {
                    return Err(::vela_vm::error::VmError::new(
                        ::vela_vm::error::VmErrorKind::ArityMismatch {
                            name: __vela_contract.public_path.clone(),
                            expected: #expected,
                            actual: args.len(),
                        },
                    ));
                }
                let mut __vela_requests = vec![
                    ::vela_engine::interop::HostParamLeaseRequest::from_argument(
                        &__vela_contract,
                        0,
                        0,
                        <#self_ty as ::vela_engine::interop::VelaHostBoundary>::vela_host_type_id(),
                        #receiver_kind,
                        &::vela_vm::owned_value::OwnedValue::HostRef(receiver.root),
                    )?,
                ];
                __vela_requests.extend([#(#additional_requests),*]);
                let __vela_lease_requests =
                    ::vela_engine::interop::preflight_host_parameter_leases(&__vela_requests)?;
                let mut __vela_result = None;
                host.adapter.with_host_leases(
                    &__vela_lease_requests,
                    &mut |__vela_erased_leases, _leased_adapter| {
                        __vela_result = Some((|| -> ::vela_vm::error::VmResult<
                            ::vela_vm::owned_value::OwnedValue
                        > {
                            let mut __vela_leases = __vela_erased_leases.iter_mut();
                            #receiver_binding
                            #(#argument_bindings)*
                            ::vela_engine::interop::catch_export_panic(
                                &__vela_contract.public_path,
                                || ::vela_engine::typed::IntoNativeReturn::into_native_return(
                                    #call_target(
                                        __vela_receiver,
                                        #(#argument_names),*
                                    )
                                ),
                            )
                        })());
                        Ok(())
                    },
                )?;
                __vela_result.expect("host lease callback must run exactly once")
            })
        }
    })
}

fn method_sync_scoped_host_adapter(
    method: &ImplItemFn,
    self_ty: &syn::Type,
    trait_path: Option<&syn::Path>,
    signature: &ClassifiedSignature,
) -> TokenStream {
    let method_ident = &method.sig.ident;
    let contract_ident = format_ident!("vela_callable_contract_{method_ident}");
    let register_ident = format_ident!("vela_register_export_{method_ident}");
    let receiver = signature
        .parameters
        .first()
        .expect("scoped method has a receiver");
    let receiver_kind = match receiver.mode {
        ParameterMode::SharedHost => quote! { ::vela_host::lease::HostLeaseKind::Shared },
        ParameterMode::ExclusiveHost => {
            quote! { ::vela_host::lease::HostLeaseKind::Exclusive }
        }
        _ => unreachable!("scoped method receiver is borrowed"),
    };
    let receiver_binding = match receiver.mode {
        ParameterMode::SharedHost => quote! {
            let __vela_receiver = __vela_parent_lease
                .object()
                .lease_any()
                .and_then(|object| object.downcast_ref::<#self_ty>())
                .ok_or_else(|| ::vela_host::lease::host_lease_unsupported(receiver.root))?;
        },
        ParameterMode::ExclusiveHost => quote! {
            let __vela_receiver = __vela_parent_lease
                .object_mut()
                .and_then(|object| object.lease_any_mut())
                .and_then(|object| object.downcast_mut::<#self_ty>())
                .ok_or_else(|| ::vela_host::lease::host_lease_unsupported(receiver.root))?;
        },
        _ => unreachable!(),
    };
    let ReturnMode::ScopedHost { child, .. } = signature.returns.mode else {
        unreachable!();
    };
    let TypeShape::Host(child_ty, _) = &signature.returns.ty else {
        unreachable!();
    };
    let child_kind = match child {
        HostAccess::Shared => quote! { ::vela_host::lease::HostLeaseKind::Shared },
        HostAccess::Exclusive => quote! { ::vela_host::lease::HostLeaseKind::Exclusive },
    };
    let child_reference = match child {
        HostAccess::Shared => quote! { &#child_ty },
        HostAccess::Exclusive => quote! { &mut #child_ty },
    };
    let wrap_child = match child {
        HostAccess::Shared => quote! { ::vela_host::lease::shared_scoped_host(__vela_child) },
        HostAccess::Exclusive => {
            quote! { ::vela_host::lease::exclusive_scoped_host(__vela_child) }
        }
    };
    let expected = signature.parameters.len().saturating_sub(1);
    let value_bindings = signature
        .parameters
        .iter()
        .skip(1)
        .enumerate()
        .map(|(index, parameter)| {
            let name = format_ident!("__vela_arg_{}", parameter.name);
            let ty = parameter.rust_ty.as_ref().expect("value parameter type");
            quote! {
                let #name = <#ty as ::vela_engine::args::FromScriptArg>::from_script_arg(
                    &args[#index],
                )?;
            }
        })
        .collect::<Vec<_>>();
    let argument_names = signature
        .parameters
        .iter()
        .skip(1)
        .map(|parameter| format_ident!("__vela_arg_{}", parameter.name))
        .collect::<Vec<_>>();
    let call_target = trait_path.map_or_else(
        || quote! { <#self_ty>::#method_ident },
        |path| quote! { <#self_ty as #path>::#method_ident },
    );

    quote! {
        #[doc(hidden)]
        #[must_use]
        pub fn #register_ident(
            builder: ::vela_engine::builder::EngineBuilder,
        ) -> ::vela_engine::builder::EngineBuilder {
            let __vela_contract = Self::#contract_ident();
            let mut __vela_desc = __vela_contract.native_method_desc(
                <#self_ty as ::vela_engine::schema::ScriptHostSchema>::script_host_type_desc().key,
            );
            __vela_desc.id = ::vela_common::HostMethodId::new(
                ::core::primitive::u128::from(::vela_common::stable_id(
                    "host_method",
                    <#self_ty>::vela_stable_type_path(),
                    ::core::stringify!(#method_ident),
                )),
            );
            builder.register_native_method_fn(__vela_desc, move |receiver, args, host| {
                if !receiver.segments.is_empty() {
                    return Err(::vela_host::lease::host_lease_unsupported(receiver.root).into());
                }
                if args.len() != #expected {
                    return Err(::vela_vm::error::VmError::new(
                        ::vela_vm::error::VmErrorKind::ArityMismatch {
                            name: __vela_contract.public_path.clone(),
                            expected: #expected,
                            actual: args.len(),
                        },
                    ));
                }
                #(#value_bindings)*
                let __vela_requests = [(receiver.root, #receiver_kind)];
                let mut __vela_invocation_error = None;
                let __vela_retained = host.adapter.with_scoped_host_return(
                    &__vela_requests,
                    &mut |leases| {
                        let __vela_parent = leases
                            .first_mut()
                            .expect("scoped method retains its receiver")
                            .take();
                        match ::vela_engine::interop::catch_export_panic(
                            &__vela_contract.public_path,
                            || ::vela_host::lease::try_scoped_host_cell(
                                __vela_parent,
                                move |__vela_parent_lease| {
                                    #receiver_binding
                                    let __vela_child: #child_reference = #call_target(
                                        __vela_receiver,
                                        #(#argument_names),*
                                    );
                                    Ok(#wrap_child)
                                },
                            ),
                        ) {
                            Ok(object) => Ok(Some(::vela_host::adapter::ScopedHostReturn {
                                object,
                                access: #child_kind,
                            })),
                            Err(error) => {
                                __vela_invocation_error = Some(error);
                                Ok(None)
                            }
                        }
                    },
                )?;
                match __vela_retained {
                    Some(root) => Ok(::vela_vm::owned_value::OwnedValue::HostRef(root)),
                    None => Err(__vela_invocation_error
                        .expect("missing scoped method invocation result")),
                }
            })
        }
    }
}

fn method_async_adapter(
    method: &ImplItemFn,
    self_ty: &syn::Type,
    trait_path: Option<&syn::Path>,
    signature: &ClassifiedSignature,
) -> TokenStream {
    let method_ident = &method.sig.ident;
    let contract_ident = format_ident!("vela_callable_contract_{method_ident}");
    let register_ident = format_ident!("vela_register_export_{method_ident}");
    let receiver = signature
        .parameters
        .first()
        .expect("method classifier emits a receiver");
    let (receiver_kind, owned_receiver, borrowed_receiver, direct_receiver) = match receiver.mode {
        ParameterMode::SharedHost => (
            quote! { ::vela_host::lease::HostLeaseKind::Shared },
            quote! {
                let __vela_receiver = ::vela_engine::host_lease::HostLeaseRef::<#self_ty>::from_erased(
                    lease, root,
                )?;
            },
            quote! {
                let __vela_receiver = __vela_leases
                    .next()
                    .and_then(|lease| lease.object().lease_any())
                    .and_then(|object| object.downcast_ref::<#self_ty>())
                    .ok_or_else(|| ::vela_host::lease::host_lease_unsupported(root))?;
            },
            quote! { &*__vela_receiver },
        ),
        ParameterMode::ExclusiveHost => (
            quote! { ::vela_host::lease::HostLeaseKind::Exclusive },
            quote! {
                let mut __vela_receiver = ::vela_engine::host_lease::HostLeaseMut::<#self_ty>::from_erased(
                    lease, root,
                )?;
            },
            quote! {
                let __vela_receiver = __vela_leases
                    .next()
                    .and_then(|lease| lease.object_mut())
                    .and_then(|object| object.lease_any_mut())
                    .and_then(|object| object.downcast_mut::<#self_ty>())
                    .ok_or_else(|| ::vela_host::lease::host_lease_unsupported(root))?;
            },
            quote! { &mut *__vela_receiver },
        ),
        _ => unreachable!("method receiver must be a host borrow"),
    };
    let call_target = trait_path.map_or_else(
        || quote! { <#self_ty>::#method_ident },
        |path| quote! { <#self_ty as #path>::#method_ident },
    );
    let expected = signature
        .parameters
        .iter()
        .skip(1)
        .filter(|parameter| parameter.mode != ParameterMode::HiddenContext)
        .count();
    let mut runtime_index = 0_usize;
    let mut has_param_leases = false;
    let param_leases = signature
        .parameters
        .iter()
        .skip(1)
        .filter_map(|parameter| {
            if parameter.mode == ParameterMode::HiddenContext {
                return None;
            }
            let index = runtime_index;
            runtime_index += 1;
            let kind = match parameter.mode {
                ParameterMode::SharedHost => {
                    has_param_leases = true;
                    quote! { ::vela_host::lease::HostLeaseKind::Shared }
                }
                ParameterMode::ExclusiveHost => {
                    has_param_leases = true;
                    quote! { ::vela_host::lease::HostLeaseKind::Exclusive }
                }
                _ => return None,
            };
            Some(quote! { (#index, #kind) })
        })
        .collect::<Vec<_>>();
    let mut runtime_index = 0_usize;
    let argument_bindings = signature
        .parameters
        .iter()
        .skip(1)
        .map(|parameter| {
            let name = format_ident!("__vela_arg_{}", parameter.name);
            if parameter.mode == ParameterMode::HiddenContext {
                return quote! { let #name = &mut *__vela_context; };
            }
            let index = runtime_index;
            runtime_index += 1;
            match &parameter.ty {
                TypeShape::Host(ty, HostAccess::Shared) => quote! {
                    let #name = __vela_leases
                        .next()
                        .and_then(|lease| lease.object().lease_any())
                        .and_then(|object| object.downcast_ref::<#ty>())
                        .ok_or_else(|| ::vela_host::lease::host_lease_unsupported(root))?;
                },
                TypeShape::Host(ty, HostAccess::Exclusive) => quote! {
                    let #name = __vela_leases
                        .next()
                        .and_then(|lease| lease.object_mut())
                        .and_then(|object| object.lease_any_mut())
                        .and_then(|object| object.downcast_mut::<#ty>())
                        .ok_or_else(|| ::vela_host::lease::host_lease_unsupported(root))?;
                },
                _ => {
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
        .skip(1)
        .map(|parameter| format_ident!("__vela_arg_{}", parameter.name))
        .collect::<Vec<_>>();
    let descriptor = quote! {
        let __vela_contract = Self::#contract_ident();
        let mut __vela_desc = __vela_contract.native_method_desc(
            <#self_ty as ::vela_engine::schema::ScriptHostSchema>::script_host_type_desc().key,
        );
        __vela_desc.id = ::vela_common::HostMethodId::new(
            ::core::primitive::u128::from(::vela_common::stable_id(
                "host_method",
                <#self_ty>::vela_stable_type_path(),
                ::core::stringify!(#method_ident),
            )),
        );
    };
    let arity = quote! {
        if args.len() != #expected {
            return Err(::vela_vm::error::VmError::new(
                ::vela_vm::error::VmErrorKind::ArityMismatch {
                    name: __vela_callable.clone(),
                    expected: #expected,
                    actual: args.len(),
                },
            ));
        }
    };
    let registration = if signature.has_hidden_context() || has_param_leases {
        quote! {
            builder.register_async_context_direct_method_fn(
                __vela_desc,
                #receiver_kind,
                vec![#(#param_leases),*],
                move |root, leases, args, __vela_context| {
                    let __vela_callable = __vela_contract.public_path.clone();
                    ::std::boxed::Box::pin(async move {
                        #arity
                        let mut __vela_leases = leases.iter_mut();
                        #borrowed_receiver
                        #(#argument_bindings)*
                        let __vela_result = #call_target(
                            __vela_receiver,
                            #(#argument_names),*
                        ).await;
                        ::vela_engine::typed::IntoNativeReturn::into_native_return(__vela_result)
                    })
                },
            )
        }
    } else {
        quote! {
            builder.register_async_direct_method_fn(
                __vela_desc,
                #receiver_kind,
                move |root, lease, args| {
                    let __vela_callable = __vela_contract.public_path.clone();
                    ::std::boxed::Box::pin(async move {
                        #arity
                        #owned_receiver
                        #(#argument_bindings)*
                        let __vela_result = #call_target(
                            #direct_receiver,
                            #(#argument_names),*
                        ).await;
                        ::vela_engine::typed::IntoNativeReturn::into_native_return(__vela_result)
                    })
                },
            )
        }
    };

    quote! {
        #[doc(hidden)]
        #[must_use]
        pub fn #register_ident(
            builder: ::vela_engine::builder::EngineBuilder,
        ) -> ::vela_engine::builder::EngineBuilder {
            #descriptor
            #registration
        }
    }
}

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
            quote! {
                ::vela_engine::interop::CallableParameter::new(#identity, #name, #hint, #mode)
            }
        });
        let return_hint = hint_tokens(&signature.returns.ty);
        let return_mode = return_mode_tokens(signature.returns.mode);
        let error_mode = match signature.returns.error_mode {
            ErrorMode::Value => quote! { ::vela_engine::interop::ErrorMode::Value },
            ErrorMode::RuntimeResult => quote! { ::vela_engine::interop::ErrorMode::RuntimeResult },
        };
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
                returns: ::vela_engine::interop::CallableReturn::new(
                    #return_hint,
                    #return_mode,
                    #error_mode,
                ),
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

fn parameter_mode_tokens(mode: ParameterMode) -> TokenStream {
    match mode {
        ParameterMode::Value => quote! { ::vela_engine::interop::BoundaryMode::Value },
        ParameterMode::ReadOnlyValueBorrow => {
            quote! { ::vela_engine::interop::BoundaryMode::ReadOnlyValueBorrow }
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

fn return_mode_tokens(mode: ReturnMode) -> TokenStream {
    match mode {
        ReturnMode::Owned => quote! { ::vela_engine::interop::ReturnMode::OwnedValue },
        ReturnMode::Structured => quote! { ::vela_engine::interop::ReturnMode::StructuredValue },
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

fn effect_tokens(effects: &std::collections::BTreeSet<EffectName>) -> TokenStream {
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

fn hint_tokens(shape: &TypeShape) -> TokenStream {
    match shape {
        TypeShape::Unit => quote! { ::vela_engine::native::TypeHint::unit() },
        TypeShape::Bool => quote! { ::vela_engine::native::TypeHint::boolean() },
        TypeShape::Char => quote! { ::vela_engine::native::TypeHint::char() },
        TypeShape::I8 => quote! { ::vela_engine::native::TypeHint::i8() },
        TypeShape::I16 => quote! { ::vela_engine::native::TypeHint::i16() },
        TypeShape::I32 => quote! { ::vela_engine::native::TypeHint::i32() },
        TypeShape::I64 => quote! { ::vela_engine::native::TypeHint::i64() },
        TypeShape::U8 => quote! { ::vela_engine::native::TypeHint::u8() },
        TypeShape::U16 => quote! { ::vela_engine::native::TypeHint::u16() },
        TypeShape::U32 => quote! { ::vela_engine::native::TypeHint::u32() },
        TypeShape::U64 => quote! { ::vela_engine::native::TypeHint::u64() },
        TypeShape::F32 => quote! { ::vela_engine::native::TypeHint::f32() },
        TypeShape::F64 => quote! { ::vela_engine::native::TypeHint::f64() },
        TypeShape::String => quote! { ::vela_engine::native::TypeHint::string() },
        TypeShape::Bytes => quote! { ::vela_engine::native::TypeHint::bytes() },
        TypeShape::Array(element) => {
            let element = hint_tokens(element);
            quote! { ::vela_engine::native::TypeHint::array_of(#element) }
        }
        TypeShape::Map(key, value) => {
            let key = hint_tokens(key);
            let value = hint_tokens(value);
            quote! { ::vela_engine::native::TypeHint::map_of(#key, #value) }
        }
        TypeShape::Set(element) => {
            let element = hint_tokens(element);
            quote! { ::vela_engine::native::TypeHint::set_of(#element) }
        }
        TypeShape::Option(payload) => {
            let payload = hint_tokens(payload);
            quote! { ::vela_engine::native::TypeHint::option_of(#payload) }
        }
        TypeShape::Result(ok, err) => {
            let ok = hint_tokens(ok);
            let err = hint_tokens(err);
            quote! { ::vela_engine::native::TypeHint::result_of(#ok, #err) }
        }
        TypeShape::Value(ty) => {
            quote! { <#ty as ::vela_engine::interop::VelaValueBoundary>::vela_type_hint() }
        }
        TypeShape::Host(ty, _) => {
            quote! { <#ty as ::vela_engine::interop::VelaHostBoundary>::vela_host_type_hint() }
        }
        TypeShape::ReceiverHost => {
            quote! { <Self as ::vela_engine::interop::VelaHostBoundary>::vela_host_type_hint() }
        }
    }
}
