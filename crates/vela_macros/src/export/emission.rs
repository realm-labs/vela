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

pub(crate) fn method_contract(
    method: &ImplItemFn,
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
        let hint = hint_tokens(&parameter.ty);
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
