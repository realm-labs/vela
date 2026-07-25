use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Result, parse_quote};

use crate::export::emission::{
    exclusive_host_value_tokens, host_type_id_tokens, shared_host_value_tokens,
};
use crate::export::signature::{
    BorrowOrigin, BorrowedCollectionKind, ClassifiedSignature, HostAccess, ParameterMode,
    ReturnMode, ScopedReturnContainer, TypeShape,
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
                let current_lease = lease_index;
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
                                    __vela_lease_requests[#current_lease].0,
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
                                    __vela_lease_requests[#current_lease].0,
                                ))?;
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
    let mut lease_index = 0_usize;
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
                lease_index += 1;
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
                if __vela_leases.len() != #lease_index {
                    return Err(::vela_vm::error::VmError::new(
                        ::vela_vm::error::VmErrorKind::TypeMismatch {
                            operation: "async service host lease count",
                        },
                    ));
                }
                let mut __vela_leases = __vela_leases.iter_mut();
                #(#argument_bindings)*
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
    if signature.scoped_return_container() != Some(ScopedReturnContainer::Direct) {
        return Err(syn::Error::new_spanned(
            &method.sig.output,
            "service borrowed return dispatch currently supports a direct reference only",
        ));
    }
    let ReturnMode::ScopedHost { origin, child, .. } = signature.returns.mode else {
        unreachable!("scoped dispatcher requires a scoped return");
    };
    let BorrowOrigin::Parameter(origin_index) = origin else {
        return Err(syn::Error::new_spanned(
            &method.sig.output,
            "service borrowed returns must originate from a service parameter",
        ));
    };
    let child_type_id = host_type_id_tokens(&signature.returns.ty).ok_or_else(|| {
        syn::Error::new_spanned(
            &method.sig.output,
            "service borrowed return has no registered host identity",
        )
    })?;
    let child_kind = match child {
        HostAccess::Shared => quote! { ::vela_host::lease::HostLeaseKind::Shared },
        HostAccess::Exclusive => quote! { ::vela_host::lease::HostLeaseKind::Exclusive },
    };
    let wrap_child = match child {
        HostAccess::Shared => quote! {
            ::vela_host::lease::shared_scoped_host_with_type_id(
                __vela_child,
                #child_type_id,
            )
        },
        HostAccess::Exclusive => quote! {
            ::vela_host::lease::exclusive_scoped_host_with_type_id(
                __vela_child,
                #child_type_id,
            )
        },
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

    Ok(quote! {
        #method_id => {
            #arity
            #(#value_preparation)*
            let mut __vela_lease_requests =
                ::vela_host::lease::HostLeaseRequestSet::with_capacity(#lease_index);
            #(#lease_requests)*
            let __vela_roots = __vela_context.with_scoped_host_return(
                &__vela_lease_requests,
                |__vela_leases| {
                    let __vela_parent = __vela_leases[#origin_lease_index].take();
                    let __vela_object = ::vela_host::lease::try_scoped_host_cell(
                        __vela_parent,
                        |__vela_parent_lease| {
                            #(#argument_bindings)*
                            let __vela_child =
                                __vela_default.#method_ident(#(#argument_names),*);
                            Ok(#wrap_child)
                        },
                    )?;
                    Ok(Some(::vela_host::adapter::ScopedHostReturns::Single(
                        ::vela_host::adapter::ScopedHostReturn {
                            object: __vela_object,
                            access: #child_kind,
                        },
                    )))
                },
            )?;
            let mut __vela_roots = __vela_roots.ok_or_else(|| {
                ::vela_vm::error::VmError::new(
                    ::vela_vm::error::VmErrorKind::TypeMismatch {
                        operation: "service borrowed return produced no host child",
                    },
                )
            })?;
            if __vela_roots.len() != 1 {
                return Err(::vela_vm::error::VmError::new(
                    ::vela_vm::error::VmErrorKind::TypeMismatch {
                        operation: "service borrowed return produced the wrong child count",
                    },
                ));
            }
            Ok(::vela_vm::owned_value::OwnedValue::HostRef(
                __vela_roots.remove(0),
            ))
        }
    })
}
