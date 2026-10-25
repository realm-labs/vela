use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::ItemFn;

use super::attrs::ExportAttrs;
use super::signature::{
    ClassifiedSignature, EffectName, ErrorMode, ParameterMode, ReturnMode, TypeShape,
};

mod binding;

use binding::hint_tokens;

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
    let parameters = signature.parameters.iter().map(|parameter| {
        let name = &parameter.name;
        let identity = vela_common::stable_id("callable_parameter", public_path, name);
        let hint = hint_tokens(&parameter.ty);
        let mode = match parameter.mode {
            ParameterMode::Value => quote! { ::vela_engine::interop::BoundaryMode::Value },
            ParameterMode::ReadOnlyValueBorrow => {
                quote! { ::vela_engine::interop::BoundaryMode::ReadOnlyValueBorrow }
            }
            ParameterMode::HiddenContext => {
                quote! { ::vela_engine::interop::BoundaryMode::HiddenContext }
            }
        };
        quote! { ::vela_engine::interop::CallableParameter::new(#identity, #name, #hint, #mode) }
    });
    let return_hint = hint_tokens(&signature.returns.ty);
    let return_mode = match signature.returns.mode {
        ReturnMode::Owned => quote! { ::vela_engine::interop::ReturnMode::OwnedValue },
        ReturnMode::Structured => quote! { ::vela_engine::interop::ReturnMode::StructuredValue },
        ReturnMode::Boundary => {
            let TypeShape::Value(ty) = &signature.returns.ty else {
                unreachable!("boundary return mode requires a value boundary type");
            };
            quote! { <#ty as ::vela_engine::interop::VelaValueBoundary>::vela_return_mode() }
        }
    };
    let error_mode = match signature.returns.error_mode {
        ErrorMode::Value => quote! { ::vela_engine::interop::ErrorMode::Value },
        ErrorMode::RuntimeResult => quote! { ::vela_engine::interop::ErrorMode::RuntimeResult },
    };
    let effects = effect_tokens(&signature.effects);
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
                    #return_hint, #return_mode, #error_mode,
                ),
                effects: #effects,
                access: ::vela_engine::interop::CallableAccess::default(),
                docs: #docs,
                attrs: ::std::collections::BTreeMap::new(),
                origin: ::vela_engine::interop::CallableOrigin {
                    language: ::vela_engine::interop::CallableLanguage::Rust,
                    source_span: None,
                },
            }
        }
    }
}

pub(crate) fn function_value_adapter(
    item: &ItemFn,
    signature: &ClassifiedSignature,
) -> TokenStream {
    let function_ident = &item.sig.ident;
    let contract_ident = format_ident!("vela_callable_contract_{function_ident}");
    let register_ident = format_ident!("vela_register_export_{function_ident}");
    let bundle_ident = format_ident!("vela_function_{function_ident}");
    let mut argument_index = 0_usize;
    let mut bindings = Vec::new();
    let mut arguments = Vec::new();
    for (index, parameter) in signature.parameters.iter().enumerate() {
        if parameter.mode == ParameterMode::HiddenContext {
            arguments.push(quote! { &mut *__vela_context });
            continue;
        }
        let name = format_ident!("__vela_arg_{index}");
        let ty = &parameter.rust_ty;
        let (decoded_ty, argument) = if parameter.mode == ParameterMode::ReadOnlyValueBorrow {
            let syn::Type::Reference(reference) = ty else {
                unreachable!("read-only parameters are references");
            };
            if crate::signature::type_ident(&reference.elem).as_deref() == Some("str") {
                (quote! { ::std::string::String }, quote! { #name.as_str() })
            } else {
                let inner = &reference.elem;
                (quote! { #inner }, quote! { &#name })
            }
        } else {
            (quote! { #ty }, quote! { #name })
        };
        bindings.push(quote! {
            let #name = <#decoded_ty as ::vela_engine::args::FromScriptArg>::from_script_arg(
                &args[#argument_index],
            )?;
        });
        arguments.push(argument);
        argument_index += 1;
    }
    let invocation = quote! {
        if args.len() != #argument_index {
            return Err(::vela_vm::error::VmError::new(
                ::vela_vm::error::VmErrorKind::ArityMismatch {
                    name: __vela_callable.clone(),
                    expected: #argument_index,
                    actual: args.len(),
                },
            ));
        }
        ::vela_engine::interop::catch_export_panic(&__vela_callable, || {
            #(#bindings)*
            ::vela_engine::typed::IntoNativeReturn::into_native_return(
                #function_ident(#(#arguments),*)
            )
        })
    };
    let invocation = if signature
        .parameters
        .iter()
        .any(|parameter| parameter.mode == ParameterMode::ReadOnlyValueBorrow)
    {
        invocation
    } else {
        let types = signature
            .parameters
            .iter()
            .filter(|parameter| parameter.mode == ParameterMode::Value)
            .map(|parameter| &parameter.rust_ty);
        let args_tuple = quote! { (#(#types,)*) };
        let call = if signature.has_hidden_context() {
            quote! {
                ::vela_engine::typed::TypedContextHostNativeFunction::<#args_tuple>::call_context(
                    &#function_ident, args, __vela_context,
                )
            }
        } else {
            quote! {
                ::vela_engine::typed::TypedNativeFunction::<#args_tuple>::call(
                    &#function_ident, args,
                )
            }
        };
        quote! {
            ::vela_engine::interop::catch_export_panic(&__vela_callable, || #call)
        }
    };
    let registration = if signature.has_hidden_context() {
        quote! {
            builder.register_context_host_native_fn(__vela_desc, move |args, __vela_context| {
                #invocation
            })
        }
    } else {
        quote! {
            builder.register_native_fn(__vela_desc, move |args| {
                #invocation
            })
        }
    };

    quote! {
        #[doc(hidden)]
        #[must_use]
        pub fn #register_ident(
            builder: ::vela_engine::builder::EngineBuilder,
        ) -> ::vela_engine::builder::EngineBuilder {
            let __vela_contract = #contract_ident();
            let __vela_desc = __vela_contract.native_function_desc();
            let __vela_callable = __vela_contract.public_path.clone();
            #registration
        }

        #[must_use]
        pub fn #bundle_ident() -> ::vela_engine::registration::ModuleRegistration {
            ::vela_engine::registration::ModuleRegistration::new(
                vec![#contract_ident()],
                #register_ident,
            )
        }
    }
}

fn effect_tokens(effects: &std::collections::BTreeSet<EffectName>) -> TokenStream {
    let mut tokens = quote! { ::vela_engine::native::EffectSet::pure() };
    for effect in effects {
        let next = match effect {
            EffectName::Pure => continue,
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
