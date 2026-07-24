use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Result, parse_quote};

use crate::export::emission::{exclusive_host_value_tokens, shared_host_value_tokens};
use crate::export::signature::{ClassifiedSignature, ParameterMode, ReturnMode, TypeShape};

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
        return Ok(quote! {
            #method_id => {
                #arity
                Err(::vela_vm::error::VmError::new(
                    ::vela_vm::error::VmErrorKind::TypeMismatch {
                        operation: "borrowed service return dispatch is not executable yet",
                    },
                ))
            }
        });
    }

    let mut lease_index = 0_usize;
    let mut lease_requests = Vec::new();
    let mut argument_bindings = Vec::new();
    let mut argument_names = Vec::new();
    for (argument_index, parameter) in signature.parameters.iter().skip(1).enumerate() {
        let name = format_ident!("__vela_arg_{}", parameter.name);
        argument_names.push(name.clone());
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
