use proc_macro2::TokenStream;
use quote::quote;

use super::meta::{HintKind, MethodEffect, MethodMeta, ParamMeta};

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
