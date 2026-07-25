use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::ImplItemFn;

use super::{
    binding_use_tokens, collection_registration_tokens, effect_tokens, exclusive_host_value_tokens,
    hint_tokens, host_type_id_tokens, parameter_mode_tokens, return_mode_tokens,
    shared_host_value_tokens,
};
use crate::export::signature::{
    ClassifiedSignature, ErrorMode, HostAccess, ParameterMode, ReturnMode, ScopedReturnContainer,
    TypeShape,
};

pub(crate) fn method_contract(
    method: &ImplItemFn,
    self_ty: &syn::Type,
    owner_path: &str,
    public_name: &str,
    docs: Option<&str>,
    reflect_callable: bool,
    signature: &ClassifiedSignature,
) -> TokenStream {
    let method_ident = &method.sig.ident;
    let contract_ident = format_ident!("vela_callable_contract_{method_ident}");
    let public_path = format!("{owner_path}::{public_name}");
    let callable_id = u128::from(vela_common::stable_id(
        "rust_method_export",
        owner_path,
        public_name,
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
                    ::vela_engine::interop::CallableKind::RustMethod,
                    #callable_id,
                ),
                public_path: #public_path.to_owned(),
                parameters: vec![#(#parameters),*],
                returns: #return_contract,
                asyncness: #asyncness,
                effects: #effects,
                access: ::vela_engine::interop::CallableAccess {
                    reflect_callable: #reflect_callable,
                    ..::vela_engine::interop::CallableAccess::default()
                },
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
    let collection_registrations = collection_registration_tokens(signature);
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
    let additional_plans = signature
        .parameters
        .iter()
        .enumerate()
        .skip(1)
        .filter_map(|(contract_index, parameter)| {
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
                    if parameter.mode == ParameterMode::ReadOnlyValueBorrow {
                        return quote! {
                            let #name =
                                <::std::string::String as ::vela_engine::args::FromScriptArg>::
                                    from_script_arg(&args[#argument_index])?;
                        };
                    }
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
        .map(|parameter| {
            let name = format_ident!("__vela_arg_{}", parameter.name);
            if parameter.mode == ParameterMode::ReadOnlyValueBorrow {
                quote! { #name.as_str() }
            } else {
                quote! { #name }
            }
        })
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
            #(#collection_registrations)*
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
            let __vela_callable = __vela_contract.public_path.clone();
            let __vela_plan = ::vela_engine::interop::PreparedHostLeasePlan::new(
                __vela_contract,
                #expected,
                [
                    ::vela_engine::interop::HostLeaseParameterPlan::receiver(
                        0,
                        <#self_ty as ::vela_engine::interop::VelaHostBoundary>::vela_host_type_id(),
                        #receiver_kind,
                    ),
                    #(#additional_plans),*
                ],
            );
            builder.register_native_method_fn(__vela_desc, move |receiver, args, host| {
                if !receiver.segments.is_empty() {
                    return Err(::vela_host::lease::host_lease_unsupported(receiver.root).into());
                }
                let __vela_lease_requests = __vela_plan.prepare_method(receiver.root, args)?;
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
                                &__vela_callable,
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
    let collection_registrations = collection_registration_tokens(signature);
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
                .expect("preflight-validated scoped shared receiver");
        },
        ParameterMode::ExclusiveHost => quote! {
            let __vela_receiver = __vela_parent_lease
                .object_mut()
                .and_then(|object| object.lease_any_mut())
                .and_then(|object| object.downcast_mut::<#self_ty>())
                .expect("preflight-validated scoped exclusive receiver");
        },
        _ => unreachable!(),
    };
    let ReturnMode::ScopedHost { child, .. } = signature.returns.mode else {
        unreachable!();
    };
    let container = signature
        .scoped_return_container()
        .expect("scoped method has a supported return container");
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
        _ => unreachable!(),
    };
    let child_kind = match child {
        HostAccess::Shared => quote! { ::vela_host::lease::HostLeaseKind::Shared },
        HostAccess::Exclusive => quote! { ::vela_host::lease::HostLeaseKind::Exclusive },
    };
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
                .expect("borrowed method return is host-backed");
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
                .expect("borrowed method return is host-backed");
            let type_id = host_type_id_tokens(shape)
                .expect("borrowed method return has a runtime type identity");
            match access {
                HostAccess::Shared => {
                    quote! {
                        ::vela_host::lease::shared_scoped_host_with_type_id(#name, #type_id)
                    }
                }
                HostAccess::Exclusive => {
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
    let expected = signature.parameters.len().saturating_sub(1);
    let value_bindings = signature
        .parameters
        .iter()
        .skip(1)
        .enumerate()
        .map(|(index, parameter)| {
            let name = format_ident!("__vela_arg_{}", parameter.name);
            if parameter.mode == ParameterMode::ReadOnlyValueBorrow {
                return quote! {
                    let #name =
                        <::std::string::String as ::vela_engine::args::FromScriptArg>::
                            from_script_arg(&args[#index])?;
                };
            }
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
        .map(|parameter| {
            let name = format_ident!("__vela_arg_{}", parameter.name);
            if parameter.mode == ParameterMode::ReadOnlyValueBorrow {
                quote! { #name.as_str() }
            } else {
                quote! { #name }
            }
        })
        .collect::<Vec<_>>();
    let call_target = trait_path.map_or_else(
        || quote! { <#self_ty>::#method_ident },
        |path| quote! { <#self_ty as #path>::#method_ident },
    );
    let scoped_return = if is_group {
        let accesses = child_shapes.iter().map(|_| &child_kind);
        quote! {
            ::vela_host::adapter::ScopedHostReturns::Group(
                ::vela_host::adapter::ScopedHostReturnGroup {
                    object: __vela_object,
                    accesses: vec![#(#accesses),*],
                },
            )
        }
    } else {
        quote! {
            ::vela_host::adapter::ScopedHostReturns::Single(
                ::vela_host::adapter::ScopedHostReturn {
                    object: __vela_object,
                    access: #child_kind,
                },
            )
        }
    };
    let invocation = match (container, signature.returns.error_mode) {
        (ScopedReturnContainer::Direct, ErrorMode::Value) => quote! {
            match ::vela_engine::interop::catch_export_panic(
                &__vela_contract.public_path,
                || #cell_constructor(
                    __vela_parent,
                    |__vela_parent_lease| {
                        #receiver_binding
                        let __vela_success = #call_target(
                            __vela_receiver,
                            #(#argument_names),*
                        );
                        #success_conversion
                    },
                ),
            ) {
                Ok(__vela_object) => Ok(Some(#scoped_return)),
                Err(__vela_error) => {
                    __vela_invocation_result = Some(Err(__vela_error));
                    Ok(None)
                }
            }
        },
        (ScopedReturnContainer::Direct, ErrorMode::RuntimeResult) => quote! {
            match ::vela_engine::interop::catch_export_panic(
                &__vela_contract.public_path,
                || Ok(#cell_constructor(
                    __vela_parent,
                    |__vela_parent_lease| {
                        #receiver_binding
                        match #call_target(__vela_receiver, #(#argument_names),*) {
                            Ok(__vela_success) => { #success_conversion },
                            Err(__vela_error) => Err(__vela_error),
                        }
                    },
                )),
            ) {
                Ok(Ok(__vela_object)) => Ok(Some(#scoped_return)),
                Ok(Err(__vela_error)) | Err(__vela_error) => {
                    __vela_invocation_result = Some(Err(__vela_error));
                    Ok(None)
                }
            }
        },
        (ScopedReturnContainer::Option, ErrorMode::Value) => quote! {
            match ::vela_engine::interop::catch_export_panic(
                &__vela_contract.public_path,
                || Ok(#cell_constructor(
                    __vela_parent,
                    |__vela_parent_lease| {
                        #receiver_binding
                        match #call_target(__vela_receiver, #(#argument_names),*) {
                            Some(__vela_success) => { #success_conversion },
                            None => Err(()),
                        }
                    },
                )),
            ) {
                Ok(Ok(__vela_object)) => Ok(Some(#scoped_return)),
                Ok(Err(())) => {
                    __vela_invocation_result = Some(Ok(
                        ::vela_vm::owned_value::OwnedValue::enum_variant(
                            "Option",
                            "None",
                            ::std::iter::empty::<(
                                &'static str,
                                ::vela_vm::owned_value::OwnedValue,
                            )>(),
                        ),
                    ));
                    Ok(None)
                }
                Err(__vela_error) => {
                    __vela_invocation_result = Some(Err(__vela_error));
                    Ok(None)
                }
            }
        },
        (ScopedReturnContainer::Result, ErrorMode::Value) => quote! {
            match ::vela_engine::interop::catch_export_panic(
                &__vela_contract.public_path,
                || Ok(#cell_constructor(
                    __vela_parent,
                    |__vela_parent_lease| {
                        #receiver_binding
                        match #call_target(__vela_receiver, #(#argument_names),*) {
                            Ok(__vela_success) => { #success_conversion },
                            Err(__vela_error) => Err(__vela_error),
                        }
                    },
                )),
            ) {
                Ok(Ok(__vela_object)) => Ok(Some(#scoped_return)),
                Ok(Err(__vela_error)) => {
                    __vela_invocation_result = Some(Ok(
                        ::vela_vm::owned_value::OwnedValue::enum_variant(
                            "Result",
                            "Err",
                            [(
                                "0",
                                ::vela_engine::args::IntoScriptArg::into_script_arg(
                                    __vela_error,
                                ),
                            )],
                        ),
                    ));
                    Ok(None)
                }
                Err(__vela_error) => {
                    __vela_invocation_result = Some(Err(__vela_error));
                    Ok(None)
                }
            }
        },
        (_, ErrorMode::RuntimeResult) => {
            unreachable!("VmResult is unwrapped before return-container classification")
        }
    };
    let retained_payload = if is_group {
        quote! {
            ::vela_vm::owned_value::OwnedValue::tuple(
                __vela_roots
                    .into_iter()
                    .map(::vela_vm::owned_value::OwnedValue::HostRef),
            )
        }
    } else {
        quote! {
            {
                let [root] = __vela_roots.as_slice() else {
                    panic!("single scoped method return must retain one root");
                };
                ::vela_vm::owned_value::OwnedValue::HostRef(*root)
            }
        }
    };
    let retained_result = match container {
        ScopedReturnContainer::Direct => quote! { #retained_payload },
        ScopedReturnContainer::Option => quote! {
            ::vela_vm::owned_value::OwnedValue::enum_variant(
                "Option",
                "Some",
                [("0", #retained_payload)],
            )
        },
        ScopedReturnContainer::Result => quote! {
            ::vela_vm::owned_value::OwnedValue::enum_variant(
                "Result",
                "Ok",
                [("0", #retained_payload)],
            )
        },
    };

    quote! {
        #[doc(hidden)]
        #[must_use]
        pub fn #register_ident(
            builder: ::vela_engine::builder::EngineBuilder,
        ) -> ::vela_engine::builder::EngineBuilder {
            #(#collection_registrations)*
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
                let mut __vela_invocation_result = None;
                let __vela_retained = host.adapter.with_scoped_host_return(
                    &__vela_requests,
                    &mut |leases| {
                        let __vela_parent = leases
                            .first_mut()
                            .expect("scoped method retains its receiver")
                            .take();
                        #invocation
                    },
                )?;
                match __vela_retained {
                    Some(__vela_roots) => Ok(#retained_result),
                    None => __vela_invocation_result
                        .expect("missing scoped method invocation result"),
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
    let collection_registrations = collection_registration_tokens(signature);
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
            match parameter.ty.host_boundary() {
                Some((_, HostAccess::Shared)) => {
                    let value = shared_host_value_tokens(&parameter.ty, quote! { lease.object() });
                    quote! {
                        let #name = __vela_leases
                            .next()
                            .and_then(|lease| #value)
                            .ok_or_else(|| ::vela_host::lease::host_lease_unsupported(root))?;
                    }
                }
                Some((_, HostAccess::Exclusive)) => {
                    let value = exclusive_host_value_tokens(&parameter.ty, quote! { object });
                    quote! {
                        let #name = __vela_leases
                            .next()
                            .and_then(|lease| lease.object_mut())
                            .and_then(|object| #value)
                            .ok_or_else(|| ::vela_host::lease::host_lease_unsupported(root))?;
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
            #(#collection_registrations)*
            #descriptor
            #registration
        }
    }
}
