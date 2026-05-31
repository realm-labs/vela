use proc_macro2::TokenStream;
use quote::quote;

use super::meta::{FunctionEffect, FunctionMeta, FunctionMode, HintKind, ParamMeta};

pub(super) fn desc_tokens(function: &FunctionMeta) -> TokenStream {
    let id = function.id;
    let name = &function.name;
    let effect = effect_tokens(function.effect);
    let returns = hint_tokens(function.returns);
    let params = function.params.iter().map(param_tokens);
    let access = access_tokens(function);
    let docs = function
        .docs
        .as_ref()
        .map(|docs| quote! { desc = desc.docs(#docs); });
    let attrs = function.attrs.iter().map(|(name, value)| {
        quote! {
            desc = desc.attr(#name, #value);
        }
    });

    quote! {
        {
            let mut desc = ::vela_engine::NativeFunctionDesc::new(
                #name,
                ::vela_engine::NativeFunctionId::new(#id),
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
        }
    }
}

fn param_tokens(param: &ParamMeta) -> TokenStream {
    let name = &param.name;
    let hint = hint_tokens(param.hint);
    quote! { #name, #hint }
}

pub(super) fn args_tuple_tokens(params: &[ParamMeta]) -> TokenStream {
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

pub(super) fn register_tokens(
    mode: FunctionMode,
    args_tuple: &TokenStream,
    desc_name: &syn::Ident,
    fn_ident: &syn::Ident,
) -> TokenStream {
    match mode {
        FunctionMode::Pure => {
            quote! { builder.register_typed_native_fn::<#args_tuple, _>(#desc_name(), #fn_ident) }
        }
        FunctionMode::Context => {
            quote! {
                builder.register_typed_context_host_native_fn::<#args_tuple, _>(
                    #desc_name(),
                    #fn_ident,
                )
            }
        }
        FunctionMode::Host => {
            quote! {
                builder.register_typed_host_native_fn::<#args_tuple, _>(
                    #desc_name(),
                    #fn_ident,
                )
            }
        }
    }
}

fn effect_tokens(effect: FunctionEffect) -> TokenStream {
    match effect {
        FunctionEffect::Pure => quote! { ::vela_engine::EffectSet::pure() },
        FunctionEffect::HostRead => quote! { ::vela_engine::EffectSet::host_read() },
        FunctionEffect::HostWrite => quote! { ::vela_engine::EffectSet::host_write() },
        FunctionEffect::EventEmit => quote! { ::vela_engine::EffectSet::event_emit() },
    }
}

fn hint_tokens(hint: HintKind) -> TokenStream {
    match hint {
        HintKind::Any => quote! { ::vela_engine::TypeHint::Any },
        HintKind::Null => quote! { ::vela_engine::TypeHint::Null },
        HintKind::Bool => quote! { ::vela_engine::TypeHint::Bool },
        HintKind::Int => quote! { ::vela_engine::TypeHint::Int },
        HintKind::Float => quote! { ::vela_engine::TypeHint::Float },
        HintKind::String => quote! { ::vela_engine::TypeHint::String },
        HintKind::Array => quote! { ::vela_engine::TypeHint::Array },
        HintKind::Map => quote! { ::vela_engine::TypeHint::Map },
        HintKind::Set => quote! { ::vela_engine::TypeHint::Set },
        HintKind::Function => quote! { ::vela_engine::TypeHint::Function },
    }
}

fn access_tokens(function: &FunctionMeta) -> TokenStream {
    let reflect_callable = function.reflect_callable;
    let permissions = function.permissions.iter().map(|permission| {
        quote! {
            access = access.require_permission(#permission);
        }
    });

    quote! {
        {
            let mut access =
                ::vela_engine::FunctionAccess::public().reflect_callable(#reflect_callable);
            #(#permissions)*
            access
        }
    }
}
