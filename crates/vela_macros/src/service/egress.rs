use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Result, ReturnType, Type};

use crate::export::signature::{
    BorrowOrigin, ClassifiedParameter, ClassifiedSignature, HostAccess, ParameterMode, ReturnMode,
    ScopedReturnContainer, TypeShape,
};
use crate::signature::type_generic_args;

pub(super) fn emit_scoped_adapter_method(
    service_path: &str,
    method: &syn::TraitItemFn,
    signature: &ClassifiedSignature,
    target_ident: &syn::Ident,
    default_call: TokenStream,
) -> Result<TokenStream> {
    let method_signature = super::dispatch_signature(method);
    let method_ident = &method.sig.ident;
    let ReturnMode::ScopedHost {
        origin: BorrowOrigin::Parameter(origin),
        ..
    } = signature.returns.mode
    else {
        return Err(syn::Error::new_spanned(
            &method.sig.output,
            "service scoped adapter requires one parameter origin",
        ));
    };
    let origin_index = usize::from(origin);
    let origin_ident = format_ident!("{}", signature.parameters[origin_index + 1].name);
    let envelope = signature.scoped_return_container().ok_or_else(|| {
        syn::Error::new_spanned(
            &method.sig.output,
            "service scoped adapter requires one admitted return envelope",
        )
    })?;
    let envelope_token = envelope_token(envelope);
    let call_arguments = signature
        .parameters
        .iter()
        .skip(1)
        .enumerate()
        .map(|(index, parameter)| {
            if index == origin_index {
                tracked_argument(parameter)
            } else {
                super::service_call_argument_tokens(parameter)
            }
        })
        .collect::<Result<Vec<_>>>()?;
    let context_candidates = signature
        .parameters
        .iter()
        .skip(1)
        .filter_map(|parameter| match (&parameter.ty, parameter.mode) {
            (TypeShape::Host(ty, HostAccess::Exclusive), ParameterMode::ExclusiveHost) => {
                Some((format_ident!("{}", parameter.name), ty))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let context_branches = context_candidates
        .iter()
        .map(|(context_ident, context_ty)| {
            quote! {
                if self.__vela_runtime.matches::<#context_ty>() {
                    match self.__vela_runtime.lease(
                        #context_ident,
                        __vela_target.artifact(),
                    ) {
                        Ok(mut __vela_runtime_lease) => {
                            let (__vela_runtime, #context_ident) =
                                __vela_runtime_lease.parts();
                            let mut __vela_args =
                                ::vela_engine::runtime::CallArgs::new();
                            #(#call_arguments)*
                            __vela_target.method().call_scoped_with_dispatcher(
                                __vela_runtime,
                                __vela_args,
                            self.__vela_options.clone(),
                            ::std::sync::Arc::clone(&self.__vela_dispatcher),
                            ::vela_engine::runtime::ServiceScopedReturnEgress::new(
                                &__vela_return_origin_identity,
                                #envelope_token,
                            ),
                        ).map_err(
                                ::vela_engine::service::ServiceInvocationError::Vm
                            )
                        }
                        Err(__vela_error) => Err(__vela_error),
                    }
                } else
            }
        });
    let decode = return_decode(envelope, &method.sig.output, &origin_ident)?;

    Ok(quote! {
        #method_signature {
            let Some(__vela_target) = self.#target_ident.as_ref() else {
                return #default_call;
            };
            let __vela_result = #(#context_branches)* {
                Err(::vela_engine::service::ServiceInvocationError::MissingRuntimeContext {
                    service: #service_path.to_owned(),
                    method: ::core::stringify!(#method_ident).to_owned(),
                    expected: self.__vela_runtime.context_name(),
                })
            };
            match __vela_result {
                Ok(__vela_return) => #decode,
                Err(__vela_error) => panic!("{}", __vela_error),
            }
        }
    })
}

fn tracked_argument(parameter: &ClassifiedParameter) -> Result<TokenStream> {
    let ident = format_ident!("{}", parameter.name);
    match (&parameter.ty, parameter.mode) {
        (TypeShape::StorageDirectedShared(_), ParameterMode::StorageDirectedShared) => Ok(quote! {
            let __vela_return_origin_identity =
                __vela_args.push_tracked_positional_host_ref(#ident);
        }),
        (TypeShape::Host(_, HostAccess::Shared), ParameterMode::SharedHost) => Ok(quote! {
            let __vela_return_origin_identity =
                __vela_args.push_tracked_positional_host_ref(#ident);
        }),
        (TypeShape::Host(_, HostAccess::Exclusive), ParameterMode::ExclusiveHost) => Ok(quote! {
            let __vela_return_origin_identity =
                __vela_args.push_tracked_positional_host_mut(#ident);
        }),
        (TypeShape::BorrowedCollection(collection), ParameterMode::SharedHost) => {
            if collection.slice_element.is_some() {
                Ok(quote! {
                    let __vela_return_origin_identity =
                        __vela_args.push_tracked_positional_slice_ref(#ident);
                })
            } else {
                Ok(quote! {
                    let __vela_return_origin_identity =
                        __vela_args.push_tracked_positional_collection_ref(#ident);
                })
            }
        }
        (TypeShape::BorrowedCollection(collection), ParameterMode::ExclusiveHost) => {
            if collection.slice_element.is_some() {
                Ok(quote! {
                    let __vela_return_origin_identity =
                        __vela_args.push_tracked_positional_slice_mut(#ident);
                })
            } else {
                Ok(quote! {
                    let __vela_return_origin_identity =
                        __vela_args.push_tracked_positional_collection_mut(#ident);
                })
            }
        }
        _ => Err(syn::Error::new_spanned(
            parameter.rust_ty.as_ref().unwrap_or(&syn::parse_quote!(())),
            "service borrowed return origin must be one direct host parameter",
        )),
    }
}

fn envelope_token(container: ScopedReturnContainer) -> TokenStream {
    match container {
        ScopedReturnContainer::Direct => {
            quote! { ::vela_engine::runtime::ServiceScopedReturnEnvelope::Direct }
        }
        ScopedReturnContainer::Option => {
            quote! { ::vela_engine::runtime::ServiceScopedReturnEnvelope::Option }
        }
        ScopedReturnContainer::Result => {
            quote! { ::vela_engine::runtime::ServiceScopedReturnEnvelope::Result }
        }
    }
}

fn return_decode(
    container: ScopedReturnContainer,
    output: &ReturnType,
    origin: &syn::Ident,
) -> Result<TokenStream> {
    let invalid = quote! {
        panic!("Vela service returned an invalid scoped return envelope")
    };
    Ok(match container {
        ScopedReturnContainer::Direct => quote! {
            match __vela_return {
                ::vela_engine::runtime::ServiceScopedReturn::Borrowed => #origin,
                ::vela_engine::runtime::ServiceScopedReturn::Empty
                | ::vela_engine::runtime::ServiceScopedReturn::Error(_) => #invalid,
            }
        },
        ScopedReturnContainer::Option => quote! {
            match __vela_return {
                ::vela_engine::runtime::ServiceScopedReturn::Borrowed => Some(#origin),
                ::vela_engine::runtime::ServiceScopedReturn::Empty => None,
                ::vela_engine::runtime::ServiceScopedReturn::Error(_) => #invalid,
            }
        },
        ScopedReturnContainer::Result => {
            let error_ty = result_error_type(output)?;
            quote! {
                match __vela_return {
                    ::vela_engine::runtime::ServiceScopedReturn::Borrowed => Ok(#origin),
                    ::vela_engine::runtime::ServiceScopedReturn::Error(__vela_error) => {
                        Err(<#error_ty as ::vela_engine::args::FromScriptArg>::from_script_arg(
                            &__vela_error,
                        ).unwrap_or_else(|__vela_error| {
                            panic!("Vela service error conversion failed: {}", __vela_error)
                        }))
                    }
                    ::vela_engine::runtime::ServiceScopedReturn::Empty => #invalid,
                }
            }
        }
    })
}

fn result_error_type(output: &ReturnType) -> Result<Type> {
    let ReturnType::Type(_, ty) = output else {
        return Err(syn::Error::new_spanned(
            output,
            "fallible scoped return requires Result<&T, E>",
        ));
    };
    let args = type_generic_args(ty);
    args.get(1).map(|ty| (*ty).clone()).ok_or_else(|| {
        syn::Error::new_spanned(output, "fallible scoped return requires Result<&T, E>")
    })
}
