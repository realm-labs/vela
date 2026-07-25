use std::collections::{BTreeSet, HashSet};

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::visit::{self, Visit};
use syn::{
    FnArg, ItemTrait, LitStr, Result, ReturnType, Signature, TraitItem, Type, TypeParamBound,
    Visibility, parse::Parser, parse_quote, parse2,
};

use crate::attrs::parse_qualified_name;
use crate::export::emission::{effect_tokens, hint_tokens, parameter_mode_tokens};
use crate::export::signature::{
    BorrowedCollectionKind, ClassifiedParameter, ClassifiedSignature, EffectName, ErrorMode,
    HostAccess, ParameterMode, ReturnMode, TypeShape, classify_service_method,
};
use crate::signature::{
    docs_from_attrs, reject_extern_signature, reject_generic_signature, reject_unsafe_signature,
};

mod boundary;
mod dispatch;
mod egress;
mod requirements;

use requirements::{
    RegistrationSpec, add_parameter_requirements, add_return_requirements, requirement_ident,
    service_return_mode_tokens,
};

pub(crate) fn expand(attr: TokenStream, input: TokenStream) -> TokenStream {
    match expand_result(attr, input) {
        Ok(tokens) => tokens,
        Err(error) => error.to_compile_error(),
    }
}

fn expand_result(attr: TokenStream, input: TokenStream) -> Result<TokenStream> {
    let mut item = parse2::<ItemTrait>(input)?;
    validate_trait(&item)?;
    let service_path = parse_path(attr)?;
    let service_id = u128::from(vela_common::stable_id("vela_service", "", &service_path));
    let mut methods = Vec::new();
    let mut registrations = Vec::new();
    let mut registration_keys = HashSet::new();

    for trait_item in &item.items {
        let TraitItem::Fn(method) = trait_item else {
            return Err(syn::Error::new_spanned(
                trait_item,
                "#[vela::service] traits support methods only",
            ));
        };
        validate_method(method)?;
        boundary::validate_return(&method.sig.output)?;
        let mut signature = classify_service_method(&method.sig, &BTreeSet::new())?;
        normalize_service_effects(&mut signature);
        boundary::validate_outer_scoped_return(&method.sig.output, &signature)?;
        if signature
            .parameters
            .iter()
            .any(|parameter| parameter.mode == ParameterMode::HiddenContext)
        {
            return Err(syn::Error::new_spanned(
                &method.sig,
                "service methods receive runtime authority through the service set context, not NativeCallContext",
            ));
        }
        if signature.returns.error_mode == ErrorMode::RuntimeResult {
            return Err(syn::Error::new_spanned(
                &method.sig.output,
                "service methods return business values or Result<T, E>, not VmResult<T>",
            ));
        }
        let emitted = emit_method(
            &service_path,
            &item.ident,
            method,
            &signature,
            &mut registrations,
            &mut registration_keys,
        )?;
        methods.push(emitted);
    }
    if methods.is_empty() {
        return Err(syn::Error::new_spanned(
            &item,
            "#[vela::service] requires at least one method",
        ));
    }
    rewrite_async_trait_methods(&mut item);

    let trait_ident = &item.ident;
    let dispatch_module_ident = dispatch_module_ident(trait_ident);
    let schema_ident = schema_function_ident(trait_ident);
    let service_id_ident = service_id_function_ident(trait_ident);
    let register_ident = registration_function_ident(trait_ident);
    let compose_ident = composition_function_ident(trait_ident);
    let dispatch_ident = rust_dispatch_function_ident(trait_ident);
    let async_dispatch_ident = rust_async_dispatch_function_ident(trait_ident);
    let adapter_ident = format_ident!("__VelaServiceAdapter{trait_ident}");
    let registration_tokens = registrations.iter().map(RegistrationSpec::tokens);
    let method_tokens = methods.iter().map(|method| &method.tokens);
    let adapter_fields = methods.iter().map(|method| &method.adapter_field);
    let adapter_initializers = methods.iter().map(|method| &method.adapter_initializer);
    let adapter_methods = methods.iter().map(|method| &method.adapter_method);
    let dispatch_trait_methods = methods.iter().map(|method| &method.dispatch_trait_method);
    let default_dispatch_methods = methods.iter().map(|method| &method.default_dispatch_method);
    let rust_dispatch_arms = methods
        .iter()
        .filter_map(|method| method.rust_dispatch_arm.as_ref());
    let async_rust_dispatch_arms = methods
        .iter()
        .filter_map(|method| method.async_rust_dispatch_arm.as_ref());
    let docs = docs_from_attrs(&item.attrs)
        .map_or_else(|| quote! { None }, |docs| quote! { Some(#docs.to_owned()) });

    Ok(quote! {
        #item

        #[doc(hidden)]
        pub mod #dispatch_module_ident {
            use super::*;

            pub trait Dispatch: ::std::marker::Send + ::std::marker::Sync {
                #(#dispatch_trait_methods)*
            }

            impl<__VelaServiceImpl> Dispatch for __VelaServiceImpl
            where
                __VelaServiceImpl:
                    #trait_ident + ::std::marker::Send + ::std::marker::Sync,
            {
                #(#default_dispatch_methods)*
            }
        }

        #[doc(hidden)]
        #[allow(non_snake_case)]
        #[must_use]
        pub fn #register_ident(
            builder: ::vela_engine::builder::EngineBuilder,
        ) -> ::vela_engine::builder::EngineBuilder {
            let builder = builder;
            #(#registration_tokens)*
            builder
        }

        #[doc(hidden)]
        #[allow(non_snake_case)]
        pub const fn #service_id_ident() -> ::vela_common::ServiceId {
            ::vela_common::ServiceId::new(#service_id)
        }

        #[doc(hidden)]
        #[allow(non_snake_case)]
        pub fn #schema_ident(
            registry: &::vela_engine::type_binding::TypeBindingRegistry,
        ) -> ::std::result::Result<
            ::vela_engine::service::ServiceSchema,
            ::vela_engine::service::ServiceSchemaError,
        > {
            let _service_docs: ::std::option::Option<::std::string::String> = #docs;
            ::vela_engine::service::ServiceSchema::new(
                ::vela_common::ServiceId::new(#service_id),
                #service_path,
                vec![#(#method_tokens),*],
                registry,
            )
        }

        #[doc(hidden)]
        pub fn #compose_ident(
            __vela_default: ::std::sync::Arc<dyn #dispatch_module_ident::Dispatch>,
            __vela_runtime: ::vela_engine::service::ServiceRuntimeBinding,
            __vela_options: ::vela_engine::runtime::CallOptions,
            __vela_dispatcher: ::std::sync::Arc<
                dyn ::vela_engine::service::ServiceCallDispatcher
            >,
            __vela_selections: &::vela_engine::service::ServiceSelectionTable<
                ::vela_engine::service::LinkedVelaServiceMethod
            >,
        ) -> ::std::sync::Arc<dyn #dispatch_module_ident::Dispatch> {
            ::std::sync::Arc::new(#adapter_ident {
                __vela_default,
                __vela_runtime,
                __vela_options,
                __vela_dispatcher,
                #(#adapter_initializers,)*
            })
        }

        #[doc(hidden)]
        pub fn #dispatch_ident(
            __vela_default: &(dyn #dispatch_module_ident::Dispatch + 'static),
            __vela_method: ::vela_common::ServiceMethodId,
            __vela_args: &[::vela_vm::owned_value::OwnedValue],
            __vela_context: &mut ::vela_engine::context::NativeCallContext<'_, '_>,
        ) -> ::vela_vm::error::VmResult<::vela_vm::owned_value::OwnedValue> {
            match __vela_method.get() {
                #(#rust_dispatch_arms,)*
                _ => Err(::vela_vm::error::VmError::new(
                    ::vela_vm::error::VmErrorKind::UnknownMethod {
                        method: ::std::format!(
                            "{}::{}",
                            #service_path,
                            __vela_method.get(),
                        ),
                    },
                )),
            }
        }

        #[doc(hidden)]
        pub fn #async_dispatch_ident<'__vela_call, '__vela_lease>(
            __vela_default:
                &'__vela_call (dyn #dispatch_module_ident::Dispatch + 'static),
            __vela_method: ::vela_common::ServiceMethodId,
            __vela_args:
                &'__vela_call [::vela_vm::owned_value::OwnedValue],
            __vela_leases: &'__vela_call mut [
                ::vela_host::lease::ErasedHostLease<'__vela_lease>
            ],
        ) -> ::vela_engine::service::ServiceFuture<
            '__vela_call,
            ::vela_vm::error::VmResult<::vela_vm::owned_value::OwnedValue>,
        >
        where
            '__vela_lease: '__vela_call,
        {
            match __vela_method.get() {
                #(#async_rust_dispatch_arms,)*
                _ => ::std::boxed::Box::pin(async move {
                    Err(::vela_vm::error::VmError::new(
                        ::vela_vm::error::VmErrorKind::UnknownMethod {
                            method: ::std::format!(
                                "{}::{}",
                                #service_path,
                                __vela_method.get(),
                            ),
                        },
                    ))
                }),
            }
        }

        #[doc(hidden)]
        struct #adapter_ident {
            __vela_default: ::std::sync::Arc<dyn #dispatch_module_ident::Dispatch>,
            __vela_runtime: ::vela_engine::service::ServiceRuntimeBinding,
            __vela_options: ::vela_engine::runtime::CallOptions,
            __vela_dispatcher: ::std::sync::Arc<
                dyn ::vela_engine::service::ServiceCallDispatcher
            >,
            #(#adapter_fields,)*
        }

        impl #dispatch_module_ident::Dispatch for #adapter_ident {
            #(#adapter_methods)*
        }
    })
}

pub(crate) fn schema_function_ident(trait_ident: &syn::Ident) -> syn::Ident {
    format_ident!("__vela_service_schema_{trait_ident}")
}

pub(crate) fn service_id_function_ident(trait_ident: &syn::Ident) -> syn::Ident {
    format_ident!("__vela_service_id_{trait_ident}")
}

pub(crate) fn registration_function_ident(trait_ident: &syn::Ident) -> syn::Ident {
    format_ident!("__vela_register_service_{trait_ident}")
}

pub(crate) fn composition_function_ident(trait_ident: &syn::Ident) -> syn::Ident {
    format_ident!("__vela_compose_service_{trait_ident}")
}

pub(crate) fn rust_dispatch_function_ident(trait_ident: &syn::Ident) -> syn::Ident {
    format_ident!("__vela_dispatch_rust_service_{trait_ident}")
}

pub(crate) fn rust_async_dispatch_function_ident(trait_ident: &syn::Ident) -> syn::Ident {
    format_ident!("__vela_dispatch_async_rust_service_{trait_ident}")
}

pub(crate) fn dispatch_module_ident(trait_ident: &syn::Ident) -> syn::Ident {
    format_ident!("__vela_service_dispatch_{trait_ident}")
}

fn validate_trait(item: &ItemTrait) -> Result<()> {
    if !matches!(item.vis, Visibility::Public(_)) {
        return Err(syn::Error::new_spanned(
            &item.vis,
            "#[vela::service] requires a public Rust trait",
        ));
    }
    reject_generic_signature(&item.generics, "#[vela::service]")?;
    if item.unsafety.is_some() || item.auto_token.is_some() {
        return Err(syn::Error::new_spanned(
            item,
            "#[vela::service] does not support unsafe or auto traits",
        ));
    }
    let mut required = BTreeSet::new();
    for bound in &item.supertraits {
        let TypeParamBound::Trait(bound) = bound else {
            return Err(syn::Error::new_spanned(
                bound,
                "#[vela::service] supports only Send + Sync supertraits",
            ));
        };
        let Some(ident) = bound.path.get_ident() else {
            return Err(syn::Error::new_spanned(
                bound,
                "#[vela::service] supports only Send + Sync supertraits",
            ));
        };
        match ident.to_string().as_str() {
            "Send" | "Sync" => {
                required.insert(ident.to_string());
            }
            _ => {
                return Err(syn::Error::new_spanned(
                    bound,
                    "#[vela::service] supports only Send + Sync supertraits",
                ));
            }
        }
    }
    if required != BTreeSet::from(["Send".to_owned(), "Sync".to_owned()]) {
        return Err(syn::Error::new_spanned(
            &item.supertraits,
            "#[vela::service] traits must require Send + Sync",
        ));
    }
    Ok(())
}

fn validate_method(method: &syn::TraitItemFn) -> Result<()> {
    if method.sig.generics.where_clause.is_some()
        || method
            .sig
            .generics
            .params
            .iter()
            .any(|parameter| !matches!(parameter, syn::GenericParam::Lifetime(_)))
    {
        return Err(syn::Error::new_spanned(
            &method.sig.generics,
            "#[vela::service] does not support generic parameters or where clauses",
        ));
    }
    reject_unsafe_signature(&method.sig, "#[vela::service]")?;
    reject_extern_signature(&method.sig, "#[vela::service]")?;
    if method.sig.constness.is_some() || method.sig.variadic.is_some() {
        return Err(syn::Error::new_spanned(
            &method.sig,
            "#[vela::service] does not support const or variadic methods",
        ));
    }
    if method.sig.asyncness.is_some() && !method.sig.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &method.sig.generics,
            "async service methods do not support explicit lifetime parameters",
        ));
    }
    let Some(FnArg::Receiver(receiver)) = method.sig.inputs.first() else {
        return Err(syn::Error::new_spanned(
            &method.sig,
            "service methods require an &self receiver",
        ));
    };
    if receiver.reference.is_none() || receiver.mutability.is_some() {
        return Err(syn::Error::new_spanned(
            receiver,
            "service methods require an &self receiver",
        ));
    }
    for input in method.sig.inputs.iter().skip(1) {
        if let FnArg::Typed(parameter) = input {
            reject_self_type(&parameter.ty)?;
        }
    }
    if let ReturnType::Type(_, ty) = &method.sig.output {
        reject_self_type(ty)?;
    }
    Ok(())
}

fn reject_self_type(ty: &Type) -> Result<()> {
    #[derive(Default)]
    struct SelfTypeVisitor {
        found: bool,
    }

    impl<'ast> Visit<'ast> for SelfTypeVisitor {
        fn visit_type_path(&mut self, path: &'ast syn::TypePath) {
            if path
                .path
                .segments
                .first()
                .is_some_and(|segment| segment.ident == "Self")
            {
                self.found = true;
            }
            visit::visit_type_path(self, path);
        }
    }

    let mut visitor = SelfTypeVisitor::default();
    visitor.visit_type(ty);
    if visitor.found {
        Err(syn::Error::new_spanned(
            ty,
            "service boundary types cannot mention Self or associated Self types",
        ))
    } else {
        Ok(())
    }
}

fn normalize_service_effects(signature: &mut ClassifiedSignature) {
    let has_explicit_host_parameter = signature.parameters.iter().skip(1).any(|parameter| {
        matches!(
            parameter.mode,
            ParameterMode::StorageDirectedShared
                | ParameterMode::SharedHost
                | ParameterMode::ExclusiveHost
        )
    });
    if !has_explicit_host_parameter && signature.effects == BTreeSet::from([EffectName::HostRead]) {
        signature.effects = BTreeSet::from([EffectName::Pure]);
    }
}

fn parse_path(attr: TokenStream) -> Result<String> {
    let mut path = None;
    let parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("path") {
            if path.is_some() {
                return Err(meta.error("service path is duplicated"));
            }
            path = Some(parse_qualified_name(
                meta.value()?.parse::<LitStr>()?,
                "service path",
            )?);
            return Ok(());
        }
        Err(meta.error("unsupported service attribute"))
    });
    parser.parse2(attr)?;
    path.ok_or_else(|| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            "#[vela::service] requires path = \"module::service\"",
        )
    })
}

struct EmittedMethod {
    tokens: TokenStream,
    adapter_field: TokenStream,
    adapter_initializer: TokenStream,
    dispatch_trait_method: TokenStream,
    default_dispatch_method: TokenStream,
    adapter_method: TokenStream,
    rust_dispatch_arm: Option<TokenStream>,
    async_rust_dispatch_arm: Option<TokenStream>,
}

fn emit_method(
    service_path: &str,
    trait_ident: &syn::Ident,
    method: &syn::TraitItemFn,
    signature: &ClassifiedSignature,
    registrations: &mut Vec<RegistrationSpec>,
    registration_keys: &mut HashSet<String>,
) -> Result<EmittedMethod> {
    let method_ident = &method.sig.ident;
    let method_path = format!("{service_path}::{method_ident}");
    let method_id = u128::from(vela_common::stable_id(
        "vela_service_method",
        service_path,
        &method_ident.to_string(),
    ));
    let service_id = u128::from(vela_common::stable_id("vela_service", "", service_path));
    let mut requirements = Vec::new();
    let mut requirement_keys = HashSet::new();
    let mut parameter_bindings = Vec::new();
    for parameter in signature.parameters.iter().skip(1) {
        let top = add_parameter_requirements(
            parameter,
            &mut requirements,
            &mut requirement_keys,
            registrations,
            registration_keys,
        )?;
        parameter_bindings.push(top);
    }
    let return_binding = add_return_requirements(
        &method.sig.output,
        &signature.returns.ty,
        signature.returns.mode,
        &mut requirements,
        &mut requirement_keys,
        registrations,
        registration_keys,
    )?;
    let requirement_statements = requirements.iter().enumerate().map(|(index, requirement)| {
        let ident = requirement_ident(index);
        let ty = &requirement.ty;
        let representation = &requirement.representation;
        let location = &requirement.location;
        quote! {
            let #ident =
                ::vela_engine::service::ServiceTypeRequirement::for_rust_type::<#ty>(
                    registry,
                    #location,
                    #representation,
                )?;
        }
    });
    let parameters = signature
        .parameters
        .iter()
        .skip(1)
        .zip(parameter_bindings)
        .map(|(parameter, binding_index)| {
            let name = &parameter.name;
            let identity = vela_common::stable_id("callable_parameter", &method_path, name);
            let hint = hint_tokens(&parameter.ty);
            let mode = parameter_mode_tokens(parameter.mode);
            let binding = requirement_ident(binding_index);
            quote! {
                ::vela_engine::interop::CallableParameter::new(
                    #identity,
                    #name,
                    #hint,
                    #mode,
                ).with_binding(#binding.contract())
            }
        });
    let return_hint = hint_tokens(&signature.returns.ty);
    let return_mode = service_return_mode_tokens(signature.returns.mode, &signature.returns.ty)?;
    let return_binding = requirement_ident(return_binding);
    let effects = effect_tokens(&signature.effects);
    let asyncness = if signature.is_async {
        quote! { ::vela_common::CallableAsyncness::Async }
    } else {
        quote! { ::vela_common::CallableAsyncness::Sync }
    };
    let method_docs = docs_from_attrs(&method.attrs)
        .map_or_else(|| quote! { None }, |docs| quote! { Some(#docs.to_owned()) });
    let requirement_values = (0..requirements.len()).map(requirement_ident);
    let tokens = quote! {{
        #(#requirement_statements)*
        ::vela_engine::service::ServiceMethodDescriptor::new(
            ::vela_common::ServiceMethodId::new(#method_id),
            #method_path,
            ::vela_engine::interop::CallableContract {
                identity: ::vela_engine::interop::CallableIdentity::new(
                    ::vela_engine::interop::CallableKind::RustTraitMethod,
                    #method_id,
                ),
                public_path: #method_path.to_owned(),
                parameters: vec![#(#parameters),*],
                returns: ::vela_engine::interop::CallableReturn::new(
                    #return_hint,
                    #return_mode,
                    ::vela_engine::interop::ErrorMode::Value,
                ).with_binding(#return_binding.contract()),
                asyncness: #asyncness,
                effects: #effects,
                access: ::vela_engine::interop::CallableAccess::default(),
                docs: #method_docs,
                attrs: ::std::collections::BTreeMap::new(),
                origin: ::vela_engine::interop::CallableOrigin {
                    language: ::vela_engine::interop::CallableLanguage::Rust,
                    source_span: None,
                },
            },
            vec![#(#requirement_values),*],
        )
    }};
    let target_ident = format_ident!("__vela_target_{method_ident}");
    let adapter_field = quote! {
        #target_ident: ::std::option::Option<
            ::vela_engine::service::LinkedVelaServiceMethod
        >
    };
    let adapter_initializer = quote! {
        #target_ident: match __vela_selections
            .get(
                ::vela_common::ServiceId::new(#service_id),
                ::vela_common::ServiceMethodId::new(#method_id),
            )
            .expect("complete service selection table must contain every method")
        {
            ::vela_engine::service::ServiceMethodSelection::RustDefault => None,
            ::vela_engine::service::ServiceMethodSelection::Vela(__vela_target) => {
                Some(__vela_target.clone())
            }
        }
    };
    let dispatch_signature = dispatch_signature(method);
    let dispatch_trait_method = quote! {
        #dispatch_signature;
    };
    let default_dispatch_method =
        emit_default_dispatch_method(trait_ident, method, &dispatch_signature);
    let adapter_method = emit_adapter_method(service_path, method, signature, &target_ident)?;
    let rust_dispatch_arm = if signature.is_async {
        None
    } else {
        Some(dispatch::emit_rust_dispatch_arm(
            service_path,
            method,
            signature,
            method_id,
        )?)
    };
    let async_rust_dispatch_arm = if signature.is_async {
        Some(dispatch::emit_async_rust_dispatch_arm(
            service_path,
            method,
            signature,
            method_id,
        )?)
    } else {
        None
    };
    Ok(EmittedMethod {
        tokens,
        adapter_field,
        adapter_initializer,
        dispatch_trait_method,
        default_dispatch_method,
        adapter_method,
        rust_dispatch_arm,
        async_rust_dispatch_arm,
    })
}

fn emit_adapter_method(
    service_path: &str,
    method: &syn::TraitItemFn,
    signature: &ClassifiedSignature,
    target_ident: &syn::Ident,
) -> Result<TokenStream> {
    let method_signature = dispatch_signature(method);
    let method_ident = &method.sig.ident;
    let argument_idents = signature
        .parameters
        .iter()
        .skip(1)
        .map(|parameter| format_ident!("{}", parameter.name))
        .collect::<Vec<_>>();
    let default_call = quote! {
        self.__vela_default.#method_ident(#(#argument_idents),*)
    };
    if signature.is_async {
        return emit_async_adapter_method(
            service_path,
            method,
            signature,
            target_ident,
            method_signature,
            default_call,
        );
    }
    if matches!(signature.returns.mode, ReturnMode::ScopedHost { .. }) {
        return egress::emit_scoped_adapter_method(
            service_path,
            method,
            signature,
            target_ident,
            default_call,
        );
    }

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
    let call_arguments = signature
        .parameters
        .iter()
        .skip(1)
        .map(service_call_argument_tokens)
        .collect::<Result<Vec<_>>>()?;
    let context_branches = context_candidates
        .iter()
        .map(|(context_ident, context_ty)| {
            quote! {
                if self.__vela_runtime.matches::<#context_ty>() {
                    self.__vela_runtime.invoke(
                        #context_ident,
                        __vela_target.artifact(),
                        |__vela_runtime, #context_ident| {
                            let mut __vela_args = ::vela_engine::runtime::CallArgs::new();
                            #(#call_arguments)*
                            let __vela_value = __vela_target.method().call_with_dispatcher(
                                __vela_runtime,
                                __vela_args,
                                self.__vela_options.clone(),
                                ::std::sync::Arc::clone(&self.__vela_dispatcher),
                            )?;
                            __vela_runtime.value_to_owned(&__vela_value)
                        },
                    )
                } else
            }
        });
    let return_ty: Type = match &method.sig.output {
        ReturnType::Default => parse_quote!(()),
        ReturnType::Type(_, ty) => ty.as_ref().clone(),
    };

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
                Ok(__vela_value) => {
                    <#return_ty as ::vela_engine::args::FromScriptArg>::from_script_arg(
                        &__vela_value,
                    )
                    .unwrap_or_else(|__vela_error| {
                        panic!(
                            "Vela service return conversion failed for `{}`: {}",
                            ::core::concat!(
                                #service_path,
                                "::",
                                ::core::stringify!(#method_ident),
                            ),
                            __vela_error,
                        )
                    })
                }
                Err(__vela_error) => panic!("{}", __vela_error),
            }
        }
    })
}

fn emit_async_adapter_method(
    service_path: &str,
    method: &syn::TraitItemFn,
    signature: &ClassifiedSignature,
    target_ident: &syn::Ident,
    method_signature: Signature,
    default_call: TokenStream,
) -> Result<TokenStream> {
    let method_ident = &method.sig.ident;
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
    let call_arguments = signature
        .parameters
        .iter()
        .skip(1)
        .map(service_call_argument_tokens)
        .collect::<Result<Vec<_>>>()?;
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
                            match __vela_target.method().call_async_with_dispatcher(
                                __vela_runtime,
                                __vela_args,
                                self.__vela_options.clone(),
                                ::std::sync::Arc::clone(&self.__vela_dispatcher),
                            ).await {
                                Ok(__vela_value) => __vela_runtime
                                    .value_to_owned(&__vela_value)
                                    .map_err(
                                        ::vela_engine::service::ServiceInvocationError::Vm
                                    ),
                                Err(__vela_error) => Err(
                                    ::vela_engine::service::ServiceInvocationError::Vm(
                                        __vela_error
                                    )
                                ),
                            }
                        }
                        Err(__vela_error) => Err(__vela_error),
                    }
                } else
            }
        });
    let return_ty: Type = match &method.sig.output {
        ReturnType::Default => parse_quote!(()),
        ReturnType::Type(_, ty) => ty.as_ref().clone(),
    };

    Ok(quote! {
        #method_signature {
            let Some(__vela_target) = self.#target_ident.as_ref() else {
                return #default_call;
            };
            ::std::boxed::Box::pin(async move {
                let __vela_result = #(#context_branches)* {
                    Err(
                        ::vela_engine::service::ServiceInvocationError::
                            MissingRuntimeContext {
                                service: #service_path.to_owned(),
                                method: ::core::stringify!(#method_ident).to_owned(),
                                expected: self.__vela_runtime.context_name(),
                            }
                    )
                };
                match __vela_result {
                    Ok(__vela_value) => {
                        <#return_ty as
                            ::vela_engine::args::FromScriptArg>::from_script_arg(
                                &__vela_value,
                            )
                        .unwrap_or_else(|__vela_error| {
                            panic!(
                                "Vela async service return conversion failed for `{}`: {}",
                                ::core::concat!(
                                    #service_path,
                                    "::",
                                    ::core::stringify!(#method_ident),
                                ),
                                __vela_error,
                            )
                        })
                    }
                    Err(__vela_error) => panic!("{}", __vela_error),
                }
            })
        }
    })
}

fn rewrite_async_trait_methods(item: &mut ItemTrait) {
    for trait_item in &mut item.items {
        let TraitItem::Fn(method) = trait_item else {
            continue;
        };
        if method.sig.asyncness.take().is_none() {
            continue;
        }
        let output: Type = match &method.sig.output {
            ReturnType::Default => parse_quote!(()),
            ReturnType::Type(_, ty) => ty.as_ref().clone(),
        };
        method.sig.output = parse_quote!(
            -> impl ::std::future::Future<Output = #output> + ::std::marker::Send
        );
    }
}

fn dispatch_signature(method: &syn::TraitItemFn) -> Signature {
    let mut signature = method.sig.clone();
    if signature.asyncness.take().is_none() {
        return signature;
    }
    let output: Type = match &signature.output {
        ReturnType::Default => parse_quote!(()),
        ReturnType::Type(_, ty) => ty.as_ref().clone(),
    };
    signature.generics.params.push(parse_quote!('__vela_call));
    for input in &mut signature.inputs {
        match input {
            FnArg::Receiver(receiver) => {
                if let Some((_, lifetime)) = &mut receiver.reference {
                    *lifetime = Some(parse_quote!('__vela_call));
                }
            }
            FnArg::Typed(parameter) => {
                if let Type::Reference(reference) = parameter.ty.as_mut() {
                    reference.lifetime = Some(parse_quote!('__vela_call));
                }
            }
        }
    }
    signature.output = parse_quote!(
        -> ::vela_engine::service::ServiceFuture<'__vela_call, #output>
    );
    signature
}

fn emit_default_dispatch_method(
    trait_ident: &syn::Ident,
    method: &syn::TraitItemFn,
    dispatch_signature: &Signature,
) -> TokenStream {
    let method_ident = &method.sig.ident;
    let arguments = method.sig.inputs.iter().skip(1).filter_map(|input| {
        let FnArg::Typed(parameter) = input else {
            return None;
        };
        let syn::Pat::Ident(ident) = parameter.pat.as_ref() else {
            return None;
        };
        Some(&ident.ident)
    });
    if method.sig.asyncness.is_some() {
        quote! {
            #dispatch_signature {
                ::std::boxed::Box::pin(async move {
                    <__VelaServiceImpl as #trait_ident>::#method_ident(
                        self,
                        #(#arguments),*
                    ).await
                })
            }
        }
    } else {
        quote! {
            #dispatch_signature {
                <__VelaServiceImpl as #trait_ident>::#method_ident(
                    self,
                    #(#arguments),*
                )
            }
        }
    }
}

fn service_call_argument_tokens(parameter: &ClassifiedParameter) -> Result<TokenStream> {
    let ident = format_ident!("{}", parameter.name);
    match (&parameter.ty, parameter.mode) {
        (TypeShape::Host(_, HostAccess::Shared), ParameterMode::SharedHost) => Ok(quote! {
            __vela_args.push_positional_host_ref(#ident);
        }),
        (TypeShape::Host(_, HostAccess::Exclusive), ParameterMode::ExclusiveHost) => Ok(quote! {
            __vela_args.push_positional_host_mut(#ident);
        }),
        (TypeShape::StorageDirectedShared(ty), ParameterMode::StorageDirectedShared) => {
            Ok(quote! {
                <#ty as ::vela_engine::interop::VelaSharedBoundary>::
                    push_shared_service_arg(#ident, &mut __vela_args);
            })
        }
        (TypeShape::BorrowedCollection(collection), ParameterMode::SharedHost) => {
            if collection.slice_element.is_some() {
                if matches!(
                    &collection.kind,
                    BorrowedCollectionKind::Array(element)
                        if matches!(element.as_ref(), TypeShape::Value(_))
                ) {
                    Ok(quote! {
                        __vela_args.push(::vela_vm::owned_value::OwnedValue::Array(
                            #ident
                                .iter()
                                .cloned()
                                .map(::vela_engine::args::IntoScriptArg::into_script_arg)
                                .collect()
                        ));
                    })
                } else {
                    Ok(quote! {
                        __vela_args.push_positional_slice_ref(#ident);
                    })
                }
            } else {
                Ok(quote! {
                    __vela_args.push_positional_collection_ref(#ident);
                })
            }
        }
        (TypeShape::BorrowedCollection(collection), ParameterMode::ExclusiveHost) => {
            if collection.slice_element.is_some() {
                Ok(quote! {
                    __vela_args.push_positional_slice_mut(#ident);
                })
            } else {
                Ok(quote! {
                    __vela_args.push_positional_collection_mut(#ident);
                })
            }
        }
        (_, ParameterMode::Value | ParameterMode::ReadOnlyValueBorrow) => Ok(quote! {
            __vela_args.push(
                ::vela_engine::args::IntoScriptArg::into_script_arg(#ident)
            );
        }),
        (_, mode) => Err(syn::Error::new_spanned(
            parameter.rust_ty.as_ref().unwrap_or(&parse_quote!(())),
            format!("unsupported Vela service call parameter mode {mode:?}"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use quote::quote;

    use super::expand_result;

    #[test]
    fn service_generates_stable_schema_and_registration_bundle() {
        let output = expand_result(
            quote! { path = "game::reward" },
            quote! {
                pub trait RewardService: Send + Sync {
                    fn apply(&self, amount: i64) -> Result<Vec<String>, String>;
                }
            },
        )
        .expect("service trait should expand")
        .to_string();

        assert!(output.contains("__vela_service_schema_RewardService"));
        assert!(output.contains("__vela_register_service_RewardService"));
        assert!(output.contains("RustTraitMethod"));
        assert!(output.contains("ServiceTypeRequirement"));
        assert!(output.contains("__vela_compose_service_RewardService"));
        assert!(output.contains("ServiceRuntimeBinding"));
        assert!(!output.contains("HostRef"));
        assert!(!output.contains("__vela_runtime : :: vela_engine :: runtime :: Runtime"));
    }

    #[test]
    fn service_requires_shared_object_safe_receiver() {
        let error = expand_result(
            quote! { path = "game::reward" },
            quote! {
                pub trait RewardService: Send + Sync {
                    fn apply(&mut self, amount: i64);
                }
            },
        )
        .expect_err("mutable service receiver must fail");

        assert!(error.to_string().contains("&self receiver"));
    }

    #[test]
    fn service_borrowed_return_uses_parameter_origin_and_scoped_dispatch() {
        let output = expand_result(
            quote! { path = "game::inventory" },
            quote! {
                pub trait InventoryService: Send + Sync {
                    fn values<'borrow>(
                        &self,
                        context: &'borrow mut RequestContext,
                    ) -> &'borrow mut RequestContext;
                }
            },
        )
        .expect("lifetime-only borrowed service return should expand")
        .to_string();

        assert!(output.contains("with_scoped_host_return"));
        assert!(output.contains("call_scoped_with_dispatcher"));
        assert!(output.contains("HostLeaseRequestSet"));
        assert!(output.contains("BorrowedReturnOrigin :: Parameter (0"));
        assert!(!output.contains("borrowed service return dispatch is not executable"));
    }

    #[test]
    fn service_rejects_runtime_result_boundary() {
        let error = expand_result(
            quote! { path = "game::reward" },
            quote! {
                pub trait RewardService: Send + Sync {
                    fn apply(&self, amount: i64) -> VmResult<i64>;
                }
            },
        )
        .expect_err("VmResult must stay outside business service ABI");

        assert!(error.to_string().contains("business values"));
    }
}
