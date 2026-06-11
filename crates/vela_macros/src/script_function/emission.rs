use proc_macro2::TokenStream;
use quote::quote;
use vela_common::PrimitiveTag;

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
            let mut desc = ::vela_engine::native::NativeFunctionDesc::new(
                #name,
                ::vela_engine::native::NativeFunctionId::new(#id),
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
        FunctionEffect::Pure => quote! { ::vela_engine::native::EffectSet::pure() },
        FunctionEffect::HostRead => quote! { ::vela_engine::native::EffectSet::host_read() },
        FunctionEffect::HostWrite => quote! { ::vela_engine::native::EffectSet::host_write() },
        FunctionEffect::EventEmit => quote! { ::vela_engine::native::EffectSet::event_emit() },
    }
}

fn hint_tokens(hint: HintKind) -> TokenStream {
    match hint {
        HintKind::Any => quote! { ::vela_engine::native::TypeHint::Any },
        HintKind::Primitive(tag) => primitive_hint_tokens(tag),
        HintKind::Array => quote! { ::vela_engine::native::TypeHint::Array },
        HintKind::Map => quote! { ::vela_engine::native::TypeHint::Map },
        HintKind::Set => quote! { ::vela_engine::native::TypeHint::Set },
        HintKind::PathProxy => quote! { ::vela_engine::native::TypeHint::PathProxy },
        HintKind::Function => quote! { ::vela_engine::native::TypeHint::Function },
    }
}

fn primitive_hint_tokens(tag: PrimitiveTag) -> TokenStream {
    match tag {
        PrimitiveTag::Null => quote! { ::vela_engine::native::TypeHint::null() },
        PrimitiveTag::Bool => quote! { ::vela_engine::native::TypeHint::boolean() },
        PrimitiveTag::I8 => quote! { ::vela_engine::native::TypeHint::i8() },
        PrimitiveTag::I16 => quote! { ::vela_engine::native::TypeHint::i16() },
        PrimitiveTag::I32 => quote! { ::vela_engine::native::TypeHint::i32() },
        PrimitiveTag::I64 => quote! { ::vela_engine::native::TypeHint::i64() },
        PrimitiveTag::U8 => quote! { ::vela_engine::native::TypeHint::u8() },
        PrimitiveTag::U16 => quote! { ::vela_engine::native::TypeHint::u16() },
        PrimitiveTag::U32 => quote! { ::vela_engine::native::TypeHint::u32() },
        PrimitiveTag::U64 => quote! { ::vela_engine::native::TypeHint::u64() },
        PrimitiveTag::F32 => quote! { ::vela_engine::native::TypeHint::f32() },
        PrimitiveTag::F64 => quote! { ::vela_engine::native::TypeHint::f64() },
        PrimitiveTag::String => quote! { ::vela_engine::native::TypeHint::string() },
        PrimitiveTag::Bytes => quote! { ::vela_engine::native::TypeHint::bytes() },
    }
}

fn access_tokens(function: &FunctionMeta) -> TokenStream {
    let base = if function.public {
        quote! { ::vela_engine::native::FunctionAccess::public() }
    } else {
        quote! { ::vela_engine::native::FunctionAccess::private() }
    };
    let reflect_visible = function.reflect_visible;
    let reflect_callable = function.reflect_callable;

    quote! {
        {
            #base
                .reflect_visible(#reflect_visible)
                .reflect_callable(#reflect_callable)
        }
    }
}
