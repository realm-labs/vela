use proc_macro2::TokenStream;
use quote::quote;

use super::meta::{HintKind, MethodEffect, MethodMeta, MethodReceiver, ParamMeta};

pub(super) fn method_tokens(method: &MethodMeta) -> TokenStream {
    let desc = method_desc_expr(method);

    quote! {
        methods.push(#desc);
    }
}

fn method_desc_expr(method: &MethodMeta) -> TokenStream {
    let name = &method.name;
    let stable_name = &method.stable_name;
    let effect = effect_tokens(method.effect);
    let returns = hint_tokens(method.returns);
    let params = method.params.iter().map(param_tokens);
    let access = access_tokens(method);
    let docs = method
        .docs
        .as_ref()
        .map(|docs| quote! { desc = desc.docs(#docs); });
    let attrs = method.attrs.iter().map(|(name, value)| {
        quote! {
            desc = desc.attr(#name, #value);
        }
    });

    quote! {{
        let method_id = ::vela_common::HostMethodId::new(::vela_common::stable_id(
            "host_method",
            &owner_stable_path,
            #stable_name,
        ));
        let mut desc = ::vela_engine::method::NativeMethodDesc::new(
            owner_key.clone(),
            method_id,
            #name,
        )
        .effects(#effect)
        .returns(#returns)
        .access(#access);
        #(
            desc = desc.param(#params);
        )*
        #(#attrs)*
        #docs
        desc
    }}
}

pub(super) fn native_method_registration_tokens(methods: &[MethodMeta]) -> TokenStream {
    let mut builder = quote! { builder };
    for method in methods.iter().filter(|method| method.callable_native) {
        let desc = method_desc_expr(method);
        let args_tuple = args_tuple_tokens(&method.params);
        let ident = &method.ident;
        builder = quote! {
            #builder.register_typed_native_method_fn::<#args_tuple, _>(
                #desc,
                Self::#ident,
            )
        };
    }

    quote! {
        #builder
    }
}

pub(super) fn script_host_method_registration_tokens(methods: &[MethodMeta]) -> TokenStream {
    let mut builder = quote! { builder };
    for method in methods {
        let desc = method_desc_expr(method);
        if method.callable_native {
            let args_tuple = args_tuple_tokens(&method.params);
            let ident = &method.ident;
            builder = quote! {
                #builder.register_typed_native_method_fn::<#args_tuple, _>(
                    #desc,
                    Self::#ident,
                )
            };
        } else {
            builder = quote! {
                #builder.register_host_method_desc(#desc)
            };
        }
    }

    quote! {
        #builder
    }
}

pub(super) fn script_host_object_impl_tokens(
    self_ty: &syn::Type,
    methods: &[MethodMeta],
) -> TokenStream {
    let arms = methods
        .iter()
        .filter(|method| method.receiver != MethodReceiver::HostBoundary)
        .map(host_method_arm_tokens);

    quote! {
        impl ::vela_host::object::ScriptHostObject for #self_ty {
            fn host_type_id(&self) -> ::vela_common::HostTypeId {
                ::vela_host::object::ScriptHostFieldAccess::script_host_type_id(self)
            }

            fn read_host_path(
                &self,
                path: &::vela_host::path::HostPath,
            ) -> ::vela_host::error::HostResult<::vela_host::value::HostValue> {
                ::vela_host::object::ScriptHostFieldAccess::read_host_path_from(self, path, 0)
            }

            fn write_host_path(
                &mut self,
                path: &::vela_host::path::HostPath,
                value: ::vela_host::value::HostValue,
            ) -> ::vela_host::error::HostResult<()> {
                ::vela_host::object::ScriptHostFieldAccess::write_host_path_from(self, path, 0, value)
            }

            fn call_host_method(
                &mut self,
                path: &::vela_host::path::HostPath,
                method: ::vela_common::HostMethodId,
                args: &[::vela_host::value::HostValue],
            ) -> ::vela_host::error::HostResult<::vela_host::value::HostValue> {
                if !path.segments.is_empty() {
                    return ::vela_host::object::ScriptHostFieldAccess::call_host_method_from(
                        self,
                        path,
                        0,
                        method,
                        args,
                    );
                }
                let owner_stable_path = Self::vela_stable_type_path();
                match method {
                    #(#arms)*
                    _ => Err(::vela_host::error::HostError {
                        kind: ::vela_host::error::HostErrorKind::UnsupportedMethod { method },
                        source_span: None,
                    }),
                }
            }
        }
    }
}

fn host_method_arm_tokens(method: &MethodMeta) -> TokenStream {
    let stable_name = &method.stable_name;
    let ident = &method.ident;
    let arg_bindings = method
        .params
        .iter()
        .enumerate()
        .map(host_method_arg_binding_tokens);
    let arg_names = method
        .params
        .iter()
        .map(|param| quote::format_ident!("__vela_arg_{}", param.name));
    let receiver = match method.receiver {
        MethodReceiver::SharedSelf | MethodReceiver::MutSelf => quote! { self },
        MethodReceiver::HostBoundary => {
            unreachable!("host-boundary methods are not direct object methods")
        }
    };

    quote! {
        method if method == ::vela_common::HostMethodId::new(::vela_common::stable_id(
            "host_method",
            owner_stable_path,
            #stable_name,
        )) => {
            #(#arg_bindings)*
            let __vela_result = #receiver.#ident(#(#arg_names),*);
            ::vela_host::object::HostValueInto::into_host_value(__vela_result)
        }
    }
}

fn host_method_arg_binding_tokens((index, param): (usize, &ParamMeta)) -> TokenStream {
    let name = quote::format_ident!("__vela_arg_{}", param.name);
    let ty = &param.ty;
    let expected = format!("argument `{}`", param.name);
    quote! {
        let #name = {
            let Some(__vela_value) = args.get(#index) else {
                return Err(::vela_host::error::HostError {
                    kind: ::vela_host::error::HostErrorKind::InvalidArgument {
                        expected: #expected,
                    },
                    source_span: None,
                });
            };
            <#ty as ::vela_host::object::HostValueFrom>::from_host_value(__vela_value)?
        };
    }
}

fn args_tuple_tokens(params: &[ParamMeta]) -> TokenStream {
    match params {
        [] => quote! { () },
        [param] => {
            let ty = &param.ty;
            quote! { (#ty,) }
        }
        params => {
            let types = params.iter().map(|param| &param.ty);
            quote! { (#(#types),*) }
        }
    }
}

fn param_tokens(param: &ParamMeta) -> TokenStream {
    let name = &param.name;
    let hint = hint_tokens(param.hint);
    quote! { #name, #hint }
}

fn effect_tokens(effect: MethodEffect) -> TokenStream {
    match effect {
        MethodEffect::Pure => quote! { ::vela_engine::native::EffectSet::pure() },
        MethodEffect::HostRead => quote! { ::vela_engine::native::EffectSet::host_read() },
        MethodEffect::HostWrite => quote! { ::vela_engine::native::EffectSet::host_write() },
        MethodEffect::EventEmit => quote! { ::vela_engine::native::EffectSet::event_emit() },
    }
}

fn hint_tokens(hint: HintKind) -> TokenStream {
    match hint {
        HintKind::Any => quote! { ::vela_engine::native::TypeHint::Any },
        HintKind::Null => quote! { ::vela_engine::native::TypeHint::Null },
        HintKind::Bool => quote! { ::vela_engine::native::TypeHint::Bool },
        HintKind::Int => quote! { ::vela_engine::native::TypeHint::Int },
        HintKind::Float => quote! { ::vela_engine::native::TypeHint::Float },
        HintKind::String => quote! { ::vela_engine::native::TypeHint::String },
        HintKind::Array => quote! { ::vela_engine::native::TypeHint::Array },
        HintKind::Map => quote! { ::vela_engine::native::TypeHint::Map },
        HintKind::Set => quote! { ::vela_engine::native::TypeHint::Set },
        HintKind::PathProxy => quote! { ::vela_engine::native::TypeHint::PathProxy },
        HintKind::HostOwner => quote! { ::vela_engine::native::TypeHint::Host(owner_key.clone()) },
        HintKind::Function => quote! { ::vela_engine::native::TypeHint::Function },
    }
}

fn access_tokens(method: &MethodMeta) -> TokenStream {
    let reflect_callable = method.reflect_callable;

    quote! {
        {
            ::vela_engine::native::FunctionAccess::public().reflect_callable(#reflect_callable)
        }
    }
}
