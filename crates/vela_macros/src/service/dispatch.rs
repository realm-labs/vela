use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Result, parse_quote};

use crate::export::emission::{exclusive_host_value_tokens, shared_host_value_tokens};
use crate::export::signature::{
    BorrowOrigin, BorrowedCollectionKind, ClassifiedSignature, ParameterMode, ReturnMode,
    ScopedReturnContainer, TypeShape,
};

pub(super) fn emit_rust_dispatch_arm(
    service_path: &str,
    method: &syn::TraitItemFn,
    signature: &ClassifiedSignature,
    method_id: u128,
) -> Result<TokenStream> {
    let method_ident = &method.sig.ident;
    let expected = signature.parameters.len().saturating_sub(1);
    let arity = quote! {
        if __vela_args.len() != #expected {
            return Err(::vela_vm::error::VmError::new(
                ::vela_vm::error::VmErrorKind::ArityMismatch {
                    name: ::std::concat!(
                        #service_path,
                        "::",
                        ::std::stringify!(#method_ident),
                    ).to_owned(),
                    expected: #expected,
                    actual: __vela_args.len(),
                },
            ));
        }
    };
    if matches!(signature.returns.mode, ReturnMode::ScopedHost { .. }) {
        return emit_scoped_rust_dispatch_arm(method, signature, method_id, arity);
    }

    let mut lease_index = 0_usize;
    let mut lease_requests = Vec::new();
    let mut argument_bindings = Vec::new();
    let mut argument_names = Vec::new();
    for (argument_index, parameter) in signature.parameters.iter().skip(1).enumerate() {
        let name = format_ident!("__vela_arg_{}", parameter.name);
        let request_index_name = format_ident!("__vela_request_{}", parameter.name);
        argument_names.push(name.clone());
        if let (TypeShape::BorrowedCollection(collection), ParameterMode::SharedHost) =
            (&parameter.ty, parameter.mode)
            && let (Some(element), BorrowedCollectionKind::Array(element_shape)) =
                (&collection.slice_element, &collection.kind)
            && matches!(element_shape.as_ref(), TypeShape::Value(_))
        {
            let owned_name = format_ident!("__vela_owned_{}", parameter.name);
            argument_bindings.push(quote! {
                let #owned_name = match &__vela_args[#argument_index] {
                    ::vela_vm::owned_value::OwnedValue::Array(__vela_values) => {
                        __vela_values
                            .iter()
                            .map(
                                <#element as
                                    ::vela_engine::args::FromScriptArg>::from_script_arg
                            )
                            .collect::<::vela_vm::error::VmResult<
                                ::std::vec::Vec<#element>
                            >>()?
                    }
                    _ => {
                        return Err(::vela_vm::error::VmError::new(
                            ::vela_vm::error::VmErrorKind::TypeMismatch {
                                operation: "service value slice argument",
                            },
                        ));
                    }
                };
                let #name = #owned_name.as_slice();
            });
            continue;
        }
        match parameter.mode {
            ParameterMode::SharedHost | ParameterMode::ExclusiveHost => {
                lease_index += 1;
                let kind = match parameter.mode {
                    ParameterMode::SharedHost => {
                        quote! { ::vela_host::lease::HostLeaseKind::Shared }
                    }
                    ParameterMode::ExclusiveHost => {
                        quote! { ::vela_host::lease::HostLeaseKind::Exclusive }
                    }
                    _ => unreachable!(),
                };
                lease_requests.push(quote! {
                    let __vela_root = match &__vela_args[#argument_index] {
                        ::vela_vm::owned_value::OwnedValue::HostRef(__vela_root) => *__vela_root,
                        _ => {
                            return Err(::vela_vm::error::VmError::new(
                                ::vela_vm::error::VmErrorKind::TypeMismatch {
                                    operation: "service host argument",
                                },
                            ));
                        }
                    };
                    let #request_index_name = __vela_lease_requests.len();
                    __vela_lease_requests.push((__vela_root, #kind));
                });
                let binding = match parameter.mode {
                    ParameterMode::SharedHost => {
                        let value = shared_host_value_tokens(
                            &parameter.ty,
                            quote! { __vela_lease.object() },
                        );
                        quote! {
                            let #name = __vela_leases
                                .next()
                                .and_then(|__vela_lease| #value)
                                .ok_or_else(|| ::vela_host::lease::host_lease_unsupported(
                                    __vela_lease_requests[#request_index_name].0,
                                ))?;
                        }
                    }
                    ParameterMode::ExclusiveHost => {
                        let value =
                            exclusive_host_value_tokens(&parameter.ty, quote! { __vela_object });
                        quote! {
                            let #name = __vela_leases
                                .next()
                                .and_then(|__vela_lease| __vela_lease.object_mut())
                                .and_then(|__vela_object| #value)
                                .ok_or_else(|| ::vela_host::lease::host_lease_unsupported(
                                    __vela_lease_requests[#request_index_name].0,
                                ))?;
                        }
                    }
                    _ => unreachable!(),
                };
                argument_bindings.push(binding);
            }
            ParameterMode::StorageDirectedShared => {
                lease_index += 1;
                let TypeShape::StorageDirectedShared(ty) = &parameter.ty else {
                    unreachable!("storage-directed mode retains its Rust type")
                };
                let prepared_name = format_ident!("__vela_prepared_{}", parameter.name);
                lease_requests.push(quote! {
                    let mut #prepared_name = None;
                    let #request_index_name = match &__vela_args[#argument_index] {
                        ::vela_vm::owned_value::OwnedValue::HostRef(__vela_root) => {
                            let __vela_index = __vela_lease_requests.len();
                            __vela_lease_requests.push((
                                *__vela_root,
                                ::vela_host::lease::HostLeaseKind::Shared,
                            ));
                            Some(__vela_index)
                        }
                        __vela_value => {
                            #prepared_name = Some(
                                <#ty as ::vela_engine::interop::VelaSharedBoundary>::
                                    decode_shared_temporary(__vela_value)?
                            );
                            None
                        }
                    };
                });
                argument_bindings.push(quote! {
                    let #name: &#ty = if let Some(__vela_index) = #request_index_name {
                        __vela_leases
                            .next()
                            .and_then(|__vela_lease| __vela_lease.object().lease_any())
                            .and_then(|__vela_object| __vela_object.downcast_ref::<#ty>())
                            .ok_or_else(|| ::vela_host::lease::host_lease_unsupported(
                                __vela_lease_requests[__vela_index].0,
                            ))?
                    } else {
                        #prepared_name
                            .as_ref()
                            .ok_or_else(|| ::vela_vm::error::VmError::new(
                                ::vela_vm::error::VmErrorKind::TypeMismatch {
                                    operation: "storage-directed Value preparation",
                                },
                            ))?
                    };
                });
            }
            ParameterMode::Value => {
                let ty = parameter
                    .rust_ty
                    .as_ref()
                    .expect("service value parameter retains its Rust type");
                argument_bindings.push(quote! {
                    let #name =
                        <#ty as ::vela_engine::args::FromScriptArg>::from_script_arg(
                            &__vela_args[#argument_index],
                        )?;
                });
            }
            ParameterMode::ReadOnlyValueBorrow => {
                let owned_name = format_ident!("__vela_owned_{}", parameter.name);
                match parameter.ty {
                    TypeShape::String => argument_bindings.push(quote! {
                        let #owned_name =
                            <::std::string::String as
                                ::vela_engine::args::FromScriptArg>::from_script_arg(
                                    &__vela_args[#argument_index],
                                )?;
                        let #name = #owned_name.as_str();
                    }),
                    _ => {
                        return Err(syn::Error::new_spanned(
                            parameter.rust_ty.as_ref().unwrap_or(&parse_quote!(())),
                            "unsupported read-only service argument in Rust dispatch",
                        ));
                    }
                }
            }
            ParameterMode::HiddenContext => unreachable!("service rejects hidden context"),
        }
    }
    let invocation = quote! {
        let mut __vela_leases = __vela_erased_leases.iter_mut();
        #(#argument_bindings)*
        ::vela_engine::typed::IntoNativeReturn::into_native_return(
            __vela_default.#method_ident(#(#argument_names),*)
        )
    };
    let body = if lease_index == 0 {
        quote! {
            let mut __vela_erased_leases:
                [::vela_host::lease::ErasedHostLease<'_>; 0] = [];
            #invocation
        }
    } else {
        quote! {
            let mut __vela_lease_requests =
                ::vela_host::lease::HostLeaseRequestSet::with_capacity(#lease_index);
            #(#lease_requests)*
            __vela_context.with_host_leases(
                &__vela_lease_requests,
                |__vela_erased_leases, _| {
                    #invocation
                },
            )
        }
    };

    Ok(quote! {
        #method_id => {
            #arity
            #body
        }
    })
}

pub(super) fn emit_async_rust_dispatch_arm(
    service_path: &str,
    method: &syn::TraitItemFn,
    signature: &ClassifiedSignature,
    method_id: u128,
) -> Result<TokenStream> {
    if matches!(signature.returns.mode, ReturnMode::ScopedHost { .. }) {
        return Err(syn::Error::new_spanned(
            &method.sig.output,
            "async service methods cannot return call-scoped host borrows",
        ));
    }
    let method_ident = &method.sig.ident;
    let expected = signature.parameters.len().saturating_sub(1);
    let mut argument_bindings = Vec::new();
    let mut argument_names = Vec::new();
    for (argument_index, parameter) in signature.parameters.iter().skip(1).enumerate() {
        let name = format_ident!("__vela_arg_{}", parameter.name);
        argument_names.push(name.clone());
        if let (TypeShape::BorrowedCollection(collection), ParameterMode::SharedHost) =
            (&parameter.ty, parameter.mode)
            && let (Some(element), BorrowedCollectionKind::Array(element_shape)) =
                (&collection.slice_element, &collection.kind)
            && matches!(element_shape.as_ref(), TypeShape::Value(_))
        {
            let owned_name = format_ident!("__vela_owned_{}", parameter.name);
            argument_bindings.push(quote! {
                let #owned_name = match &__vela_args[#argument_index] {
                    ::vela_vm::owned_value::OwnedValue::Array(__vela_values) => {
                        __vela_values
                            .iter()
                            .map(
                                <#element as
                                    ::vela_engine::args::FromScriptArg>::from_script_arg
                            )
                            .collect::<::vela_vm::error::VmResult<
                                ::std::vec::Vec<#element>
                            >>()?
                    }
                    _ => {
                        return Err(::vela_vm::error::VmError::new(
                            ::vela_vm::error::VmErrorKind::TypeMismatch {
                                operation: "async service value slice argument",
                            },
                        ));
                    }
                };
                let #name = #owned_name.as_slice();
            });
            continue;
        }
        match parameter.mode {
            ParameterMode::SharedHost | ParameterMode::ExclusiveHost => {
                let binding = match parameter.mode {
                    ParameterMode::SharedHost => {
                        let value = shared_host_value_tokens(
                            &parameter.ty,
                            quote! { __vela_lease.object() },
                        );
                        quote! {
                            let #name = __vela_leases
                                .next()
                                .and_then(|__vela_lease| #value)
                                .ok_or_else(|| ::vela_vm::error::VmError::new(
                                    ::vela_vm::error::VmErrorKind::TypeMismatch {
                                        operation: "async service shared host argument",
                                    },
                                ))?;
                        }
                    }
                    ParameterMode::ExclusiveHost => {
                        let value =
                            exclusive_host_value_tokens(&parameter.ty, quote! { __vela_object });
                        quote! {
                            let #name = __vela_leases
                                .next()
                                .and_then(|__vela_lease| __vela_lease.object_mut())
                                .and_then(|__vela_object| #value)
                                .ok_or_else(|| ::vela_vm::error::VmError::new(
                                    ::vela_vm::error::VmErrorKind::TypeMismatch {
                                        operation: "async service exclusive host argument",
                                    },
                                ))?;
                        }
                    }
                    _ => unreachable!(),
                };
                argument_bindings.push(binding);
            }
            ParameterMode::StorageDirectedShared => {
                let TypeShape::StorageDirectedShared(ty) = &parameter.ty else {
                    unreachable!("storage-directed mode retains its Rust type")
                };
                let prepared_name = format_ident!("__vela_prepared_{}", parameter.name);
                argument_bindings.push(quote! {
                    let #prepared_name = match &__vela_args[#argument_index] {
                        ::vela_vm::owned_value::OwnedValue::HostRef(_) => None,
                        __vela_value => Some(
                            <#ty as ::vela_engine::interop::VelaSharedBoundary>::
                                decode_shared_temporary(__vela_value)?
                        ),
                    };
                    let #name: &#ty = if matches!(
                        &__vela_args[#argument_index],
                        ::vela_vm::owned_value::OwnedValue::HostRef(_)
                    ) {
                        __vela_leases
                            .next()
                            .and_then(|__vela_lease| __vela_lease.object().lease_any())
                            .and_then(|__vela_object| __vela_object.downcast_ref::<#ty>())
                            .ok_or_else(|| ::vela_vm::error::VmError::new(
                                ::vela_vm::error::VmErrorKind::TypeMismatch {
                                    operation: "async storage-directed shared Host argument",
                                },
                            ))?
                    } else {
                        #prepared_name
                            .as_ref()
                            .ok_or_else(|| ::vela_vm::error::VmError::new(
                                ::vela_vm::error::VmErrorKind::TypeMismatch {
                                    operation: "async storage-directed Value preparation",
                                },
                            ))?
                    };
                });
            }
            ParameterMode::Value => {
                let ty = parameter
                    .rust_ty
                    .as_ref()
                    .expect("service value parameter retains its Rust type");
                argument_bindings.push(quote! {
                    let #name =
                        <#ty as ::vela_engine::args::FromScriptArg>::from_script_arg(
                            &__vela_args[#argument_index],
                        )?;
                });
            }
            ParameterMode::ReadOnlyValueBorrow => match parameter.ty {
                TypeShape::String => {
                    let owned_name = format_ident!("__vela_owned_{}", parameter.name);
                    argument_bindings.push(quote! {
                        let #owned_name =
                            <::std::string::String as
                                ::vela_engine::args::FromScriptArg>::from_script_arg(
                                    &__vela_args[#argument_index],
                                )?;
                        let #name = #owned_name.as_str();
                    });
                }
                _ => {
                    return Err(syn::Error::new_spanned(
                        parameter.rust_ty.as_ref().unwrap_or(&parse_quote!(())),
                        "unsupported read-only async service argument in Rust dispatch",
                    ));
                }
            },
            ParameterMode::HiddenContext => unreachable!("service rejects hidden context"),
        }
    }

    Ok(quote! {
        #method_id => {
            ::std::boxed::Box::pin(async move {
                if __vela_args.len() != #expected {
                    return Err(::vela_vm::error::VmError::new(
                        ::vela_vm::error::VmErrorKind::ArityMismatch {
                            name: ::std::concat!(
                                #service_path,
                                "::",
                                ::std::stringify!(#method_ident),
                            ).to_owned(),
                            expected: #expected,
                            actual: __vela_args.len(),
                        },
                    ));
                }
                let mut __vela_leases = __vela_leases.iter_mut();
                #(#argument_bindings)*
                if __vela_leases.next().is_some() {
                    return Err(::vela_vm::error::VmError::new(
                        ::vela_vm::error::VmErrorKind::TypeMismatch {
                            operation: "async service host lease count",
                        },
                    ));
                }
                ::vela_engine::typed::IntoNativeReturn::into_native_return(
                    __vela_default.#method_ident(#(#argument_names),*).await
                )
            })
        }
    })
}

fn emit_scoped_rust_dispatch_arm(
    method: &syn::TraitItemFn,
    signature: &ClassifiedSignature,
    method_id: u128,
    arity: TokenStream,
) -> Result<TokenStream> {
    let container = signature.scoped_return_container().ok_or_else(|| {
        syn::Error::new_spanned(
            &method.sig.output,
            "service borrowed return has no executable scoped envelope",
        )
    })?;
    let ReturnMode::ScopedHost { origin, .. } = signature.returns.mode else {
        unreachable!("scoped dispatcher requires a scoped return");
    };
    let BorrowOrigin::Parameter(origin_index) = origin else {
        return Err(syn::Error::new_spanned(
            &method.sig.output,
            "service borrowed returns must originate from a service parameter",
        ));
    };
    let mut lease_index = 0_usize;
    let mut origin_lease_index = None;
    let mut lease_requests = Vec::new();
    let mut value_preparation = Vec::new();
    let mut argument_bindings = Vec::new();
    let mut argument_names = Vec::new();
    for (argument_index, parameter) in signature.parameters.iter().skip(1).enumerate() {
        let name = format_ident!("__vela_arg_{}", parameter.name);
        argument_names.push(name.clone());
        match parameter.mode {
            ParameterMode::StorageDirectedShared => {
                if argument_index != usize::from(origin_index) {
                    return Err(syn::Error::new_spanned(
                        parameter.rust_ty.as_ref().unwrap_or(&parse_quote!(())),
                        "a borrowed-return service currently requires additional shared custom parameters to be passed by value",
                    ));
                }
                let current_lease = lease_index;
                origin_lease_index = Some(current_lease);
                lease_index += 1;
                lease_requests.push(quote! {
                    let __vela_root = match &__vela_args[#argument_index] {
                        ::vela_vm::owned_value::OwnedValue::HostRef(__vela_root) => *__vela_root,
                        _ => {
                            return Err(::vela_vm::error::VmError::new(
                                ::vela_vm::error::VmErrorKind::TypeMismatch {
                                    operation: "service borrowed-return storage-directed origin",
                                },
                            ));
                        }
                    };
                    __vela_lease_requests.push((
                        __vela_root,
                        ::vela_host::lease::HostLeaseKind::Shared,
                    ));
                });
                let value = shared_host_value_tokens(
                    &parameter.ty,
                    quote! { __vela_parent_lease.object() },
                );
                argument_bindings.push(quote! {
                    let #name = #value.ok_or_else(|| {
                        ::vela_host::lease::host_lease_unsupported(
                            __vela_lease_requests[#current_lease].0,
                        )
                    })?;
                });
            }
            ParameterMode::SharedHost | ParameterMode::ExclusiveHost => {
                let current_lease = lease_index;
                if argument_index == usize::from(origin_index) {
                    origin_lease_index = Some(current_lease);
                }
                lease_index += 1;
                let kind = match parameter.mode {
                    ParameterMode::SharedHost => {
                        quote! { ::vela_host::lease::HostLeaseKind::Shared }
                    }
                    ParameterMode::ExclusiveHost => {
                        quote! { ::vela_host::lease::HostLeaseKind::Exclusive }
                    }
                    _ => unreachable!(),
                };
                lease_requests.push(quote! {
                    let __vela_root = match &__vela_args[#argument_index] {
                        ::vela_vm::owned_value::OwnedValue::HostRef(__vela_root) => *__vela_root,
                        _ => {
                            return Err(::vela_vm::error::VmError::new(
                                ::vela_vm::error::VmErrorKind::TypeMismatch {
                                    operation: "service borrowed-return host argument",
                                },
                            ));
                        }
                    };
                    __vela_lease_requests.push((__vela_root, #kind));
                });
                let (shared_object, exclusive_object) =
                    if argument_index == usize::from(origin_index) {
                        (
                            quote! { __vela_parent_lease.object() },
                            quote! {
                                __vela_parent_lease
                                    .object_mut()
                                    .expect("exclusive service parent lease")
                            },
                        )
                    } else {
                        (
                            quote! { __vela_leases[#current_lease].object() },
                            quote! {
                                __vela_leases[#current_lease]
                                    .object_mut()
                                    .expect("exclusive service argument lease")
                            },
                        )
                    };
                let binding = match parameter.mode {
                    ParameterMode::SharedHost => {
                        let value = shared_host_value_tokens(&parameter.ty, shared_object);
                        quote! {
                            let #name = #value.ok_or_else(|| {
                                ::vela_host::lease::host_lease_unsupported(
                                    __vela_lease_requests[#current_lease].0,
                                )
                            })?;
                        }
                    }
                    ParameterMode::ExclusiveHost => {
                        let value = exclusive_host_value_tokens(&parameter.ty, exclusive_object);
                        quote! {
                            let #name = #value.ok_or_else(|| {
                                ::vela_host::lease::host_lease_unsupported(
                                    __vela_lease_requests[#current_lease].0,
                                )
                            })?;
                        }
                    }
                    _ => unreachable!(),
                };
                argument_bindings.push(binding);
            }
            ParameterMode::Value => {
                let ty = parameter
                    .rust_ty
                    .as_ref()
                    .expect("service value parameter retains its Rust type");
                let prepared = format_ident!("__vela_prepared_{}", parameter.name);
                value_preparation.push(quote! {
                    let mut #prepared = Some(
                        <#ty as ::vela_engine::args::FromScriptArg>::from_script_arg(
                            &__vela_args[#argument_index],
                        )?
                    );
                });
                argument_bindings.push(quote! {
                    let #name = #prepared
                        .take()
                        .expect("scoped service callback runs once");
                });
            }
            ParameterMode::ReadOnlyValueBorrow => {
                let prepared = format_ident!("__vela_prepared_{}", parameter.name);
                match parameter.ty {
                    TypeShape::String => {
                        value_preparation.push(quote! {
                            let #prepared =
                                <::std::string::String as
                                    ::vela_engine::args::FromScriptArg>::from_script_arg(
                                        &__vela_args[#argument_index],
                                    )?;
                        });
                        argument_bindings.push(quote! {
                            let #name = #prepared.as_str();
                        });
                    }
                    _ => {
                        return Err(syn::Error::new_spanned(
                            parameter.rust_ty.as_ref().unwrap_or(&parse_quote!(())),
                            "unsupported read-only borrowed-return service argument",
                        ));
                    }
                }
            }
            ParameterMode::HiddenContext => unreachable!("service rejects hidden context"),
        }
    }
    let origin_lease_index = origin_lease_index.ok_or_else(|| {
        syn::Error::new_spanned(
            &method.sig.output,
            "service borrowed return origin must be a host-backed parameter",
        )
    })?;
    let method_ident = &method.sig.ident;
    let origin_argument = &argument_names[usize::from(origin_index)];
    let call_arguments = argument_names
        .iter()
        .enumerate()
        .map(|(index, argument)| {
            if index != usize::from(origin_index) {
                return quote! { #argument };
            }
            match signature.parameters[index + 1].mode {
                ParameterMode::StorageDirectedShared | ParameterMode::SharedHost => {
                    quote! { &*#argument }
                }
                ParameterMode::ExclusiveHost => quote! { &mut *#argument },
                _ => unreachable!("borrowed return origin is host-backed"),
            }
        })
        .collect::<Vec<_>>();
    let direct_root = quote! {
        ::vela_vm::owned_value::OwnedValue::HostRef(
            __vela_lease_requests[#origin_lease_index].0,
        )
    };
    let scoped_invocation = match container {
        ScopedReturnContainer::Direct => quote! {
            #(#argument_bindings)*
            let __vela_origin_pointer = ::std::ptr::from_ref(&*#origin_argument);
            let __vela_child =
                __vela_default.#method_ident(#(#call_arguments),*);
            if !::std::ptr::eq(
                __vela_origin_pointer,
                ::std::ptr::from_ref(&*__vela_child),
            ) {
                return Err(::vela_host::error::HostError {
                    kind: ::vela_host::error::HostErrorKind::InvalidArgument {
                        expected: "service direct borrowed return provenance",
                    },
                    source_span: None,
                });
            }
            __vela_direct_return = Some(#direct_root);
            Ok(None)
        },
        ScopedReturnContainer::Option => quote! {
            #(#argument_bindings)*
            let __vela_origin_pointer = ::std::ptr::from_ref(&*#origin_argument);
            __vela_direct_return = Some(match __vela_default
                .#method_ident(#(#call_arguments),*)
            {
                Some(__vela_child) => {
                    if !::std::ptr::eq(
                        __vela_origin_pointer,
                        ::std::ptr::from_ref(&*__vela_child),
                    ) {
                        return Err(::vela_host::error::HostError {
                            kind: ::vela_host::error::HostErrorKind::InvalidArgument {
                                expected: "service optional borrowed return provenance",
                            },
                            source_span: None,
                        });
                    }
                    ::vela_vm::owned_value::OwnedValue::enum_variant(
                        "Option",
                        "Some",
                        [("0", #direct_root)],
                    )
                }
                None => ::vela_vm::owned_value::OwnedValue::enum_variant(
                    "Option",
                    "None",
                    ::std::iter::empty::<(
                        &'static str,
                        ::vela_vm::owned_value::OwnedValue,
                    )>(),
                ),
            });
            Ok(None)
        },
        ScopedReturnContainer::Result => quote! {
            #(#argument_bindings)*
            let __vela_origin_pointer = ::std::ptr::from_ref(&*#origin_argument);
            __vela_direct_return = Some(match __vela_default
                .#method_ident(#(#call_arguments),*)
            {
                Ok(__vela_child) => {
                    if !::std::ptr::eq(
                        __vela_origin_pointer,
                        ::std::ptr::from_ref(&*__vela_child),
                    ) {
                        return Err(::vela_host::error::HostError {
                            kind: ::vela_host::error::HostErrorKind::InvalidArgument {
                                expected: "service fallible borrowed return provenance",
                            },
                            source_span: None,
                        });
                    }
                    ::vela_vm::owned_value::OwnedValue::enum_variant(
                        "Result",
                        "Ok",
                        [("0", #direct_root)],
                    )
                }
                Err(__vela_error) => {
                    ::vela_vm::owned_value::OwnedValue::enum_variant(
                        "Result",
                        "Err",
                        [(
                            "0",
                            ::vela_engine::args::IntoScriptArg::into_script_arg(
                                __vela_error,
                            ),
                        )],
                    )
                }
            });
            Ok(None)
        },
    };

    Ok(quote! {
        #method_id => {
            #arity
            #(#value_preparation)*
            let mut __vela_lease_requests =
                ::vela_host::lease::HostLeaseRequestSet::with_capacity(#lease_index);
            #(#lease_requests)*
            let mut __vela_direct_return = None;
            __vela_context.with_scoped_host_return(
                &__vela_lease_requests,
                |__vela_leases| {
                    let __vela_parent_lease =
                        &mut __vela_leases[#origin_lease_index];
                    #scoped_invocation
                },
            )?;
            __vela_direct_return.ok_or_else(|| {
                ::vela_vm::error::VmError::new(
                    ::vela_vm::error::VmErrorKind::TypeMismatch {
                        operation: "service borrowed return produced no direct value",
                    },
                )
            })
        }
    })
}
