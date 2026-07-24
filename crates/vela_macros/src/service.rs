use std::collections::{BTreeSet, HashSet};

use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};
use syn::visit::{self, Visit};
use syn::{
    FnArg, ItemTrait, LitStr, Result, ReturnType, TraitItem, Type, TypeParamBound, Visibility,
    parse::Parser, parse_quote, parse2,
};

use crate::attrs::parse_qualified_name;
use crate::export::emission::{
    effect_tokens, hint_tokens, parameter_mode_tokens, return_mode_tokens,
};
use crate::export::signature::{
    BorrowedCollectionKind, BorrowedCollectionShape, ClassifiedParameter, ClassifiedSignature,
    EffectName, ErrorMode, HostAccess, ParameterMode, ReturnMode, TypeShape, classify_method,
};
use crate::signature::{
    docs_from_attrs, reject_extern_signature, reject_generic_signature, reject_unsafe_signature,
    type_generic_args,
};

pub(crate) fn expand(attr: TokenStream, input: TokenStream) -> TokenStream {
    match expand_result(attr, input) {
        Ok(tokens) => tokens,
        Err(error) => error.to_compile_error(),
    }
}

fn expand_result(attr: TokenStream, input: TokenStream) -> Result<TokenStream> {
    let item = parse2::<ItemTrait>(input)?;
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
        let mut signature = classify_method(&method.sig, &BTreeSet::new())?;
        normalize_service_effects(&mut signature);
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

    let trait_ident = &item.ident;
    let schema_ident = schema_function_ident(trait_ident);
    let register_ident = registration_function_ident(trait_ident);
    let registration_tokens = registrations.iter().map(RegistrationSpec::tokens);
    let method_tokens = methods.iter().map(|method| &method.tokens);
    let docs = docs_from_attrs(&item.attrs)
        .map_or_else(|| quote! { None }, |docs| quote! { Some(#docs.to_owned()) });

    Ok(quote! {
        #item

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
    })
}

pub(crate) fn schema_function_ident(trait_ident: &syn::Ident) -> syn::Ident {
    format_ident!("__vela_service_schema_{trait_ident}")
}

pub(crate) fn registration_function_ident(trait_ident: &syn::Ident) -> syn::Ident {
    format_ident!("__vela_register_service_{trait_ident}")
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
    reject_generic_signature(&method.sig.generics, "#[vela::service]")?;
    reject_unsafe_signature(&method.sig, "#[vela::service]")?;
    reject_extern_signature(&method.sig, "#[vela::service]")?;
    if method.sig.constness.is_some() || method.sig.variadic.is_some() {
        return Err(syn::Error::new_spanned(
            &method.sig,
            "#[vela::service] does not support const or variadic methods",
        ));
    }
    if method.sig.asyncness.is_some() {
        return Err(syn::Error::new_spanned(
            method.sig.asyncness,
            "authored async service methods require the S6 object-safe adapter; S4 service methods are synchronous",
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
            ParameterMode::SharedHost | ParameterMode::ExclusiveHost
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
}

fn emit_method(
    service_path: &str,
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
                origin: ::vela_engine::interop::CallableOrigin {
                    language: ::vela_engine::interop::CallableLanguage::Rust,
                    source_span: None,
                },
            },
            vec![#(#requirement_values),*],
        )
    }};
    Ok(EmittedMethod { tokens })
}

#[allow(clippy::too_many_arguments)]
fn add_parameter_requirements(
    parameter: &ClassifiedParameter,
    requirements: &mut Vec<RequirementSpec>,
    requirement_keys: &mut HashSet<String>,
    registrations: &mut Vec<RegistrationSpec>,
    registration_keys: &mut HashSet<String>,
) -> Result<usize> {
    let location = format!("parameter {}", parameter.name);
    match (&parameter.ty, parameter.mode) {
        (TypeShape::String, ParameterMode::ReadOnlyValueBorrow) => {
            let ty: Type = parse_quote!(::std::string::String);
            let top = push_requirement(
                requirements,
                requirement_keys,
                RequirementSpec::owned(ty.clone(), location),
            );
            push_value_registration(registrations, registration_keys, ty);
            Ok(top)
        }
        (TypeShape::Host(ty, access), _) => Ok(push_requirement(
            requirements,
            requirement_keys,
            RequirementSpec::new(ty.clone(), host_representation(*access), location),
        )),
        (TypeShape::BorrowedCollection(collection), _) => {
            let top = push_borrowed_collection_requirement(
                collection,
                location.clone(),
                requirements,
                requirement_keys,
                registrations,
                registration_keys,
            );
            collect_collection_children(
                collection,
                &location,
                requirements,
                requirement_keys,
                registrations,
                registration_keys,
            );
            Ok(top)
        }
        (_, ParameterMode::Value) => {
            let ty = parameter
                .rust_ty
                .clone()
                .expect("classified value parameter retains its Rust type");
            Ok(collect_owned_type(
                &ty,
                &location,
                requirements,
                requirement_keys,
                registrations,
                registration_keys,
            ))
        }
        (_, mode) => Err(syn::Error::new_spanned(
            parameter.rust_ty.as_ref().unwrap_or(&parse_quote!(())),
            format!("unsupported service parameter mode {mode:?}"),
        )),
    }
}

#[allow(clippy::too_many_arguments)]
fn add_return_requirements(
    output: &ReturnType,
    shape: &TypeShape,
    mode: ReturnMode,
    requirements: &mut Vec<RequirementSpec>,
    requirement_keys: &mut HashSet<String>,
    registrations: &mut Vec<RegistrationSpec>,
    registration_keys: &mut HashSet<String>,
) -> Result<usize> {
    let location = "return";
    match mode {
        ReturnMode::Owned | ReturnMode::Structured | ReturnMode::Boundary => {
            let ty = match output {
                ReturnType::Default => parse_quote!(()),
                ReturnType::Type(_, ty) => ty.as_ref().clone(),
            };
            Ok(collect_owned_type(
                &ty,
                location,
                requirements,
                requirement_keys,
                registrations,
                registration_keys,
            ))
        }
        ReturnMode::ScopedHost { .. } => match shape {
            TypeShape::Host(ty, access) => Ok(push_requirement(
                requirements,
                requirement_keys,
                RequirementSpec::new(ty.clone(), host_representation(*access), location),
            )),
            TypeShape::BorrowedCollection(collection) => Ok(push_borrowed_collection_requirement(
                collection,
                location.to_owned(),
                requirements,
                requirement_keys,
                registrations,
                registration_keys,
            )),
            _ => Err(syn::Error::new_spanned(
                output,
                "service borrowed returns currently require one direct reference or collection view",
            )),
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_owned_type(
    ty: &Type,
    location: &str,
    requirements: &mut Vec<RequirementSpec>,
    requirement_keys: &mut HashSet<String>,
    registrations: &mut Vec<RegistrationSpec>,
    registration_keys: &mut HashSet<String>,
) -> usize {
    let top = push_requirement(
        requirements,
        requirement_keys,
        RequirementSpec::owned(ty.clone(), location),
    );
    push_value_registration(registrations, registration_keys, ty.clone());
    match ty {
        Type::Array(array) => {
            collect_owned_type(
                &array.elem,
                location,
                requirements,
                requirement_keys,
                registrations,
                registration_keys,
            );
        }
        Type::Tuple(tuple) => {
            for element in &tuple.elems {
                collect_owned_type(
                    element,
                    location,
                    requirements,
                    requirement_keys,
                    registrations,
                    registration_keys,
                );
            }
        }
        Type::Path(_) => {
            for argument in type_generic_args(ty) {
                collect_owned_type(
                    argument,
                    location,
                    requirements,
                    requirement_keys,
                    registrations,
                    registration_keys,
                );
            }
        }
        _ => {}
    }
    top
}

#[allow(clippy::too_many_arguments)]
fn push_borrowed_collection_requirement(
    collection: &BorrowedCollectionShape,
    location: String,
    requirements: &mut Vec<RequirementSpec>,
    requirement_keys: &mut HashSet<String>,
    registrations: &mut Vec<RegistrationSpec>,
    registration_keys: &mut HashSet<String>,
) -> usize {
    let representation = collection_representation(collection);
    if let Some(element) = &collection.slice_element {
        let ty: Type = parse_quote!(::vela_engine::standard::SliceBinding<#element>);
        let top = push_requirement(
            requirements,
            requirement_keys,
            RequirementSpec::new(ty, representation, location),
        );
        push_registration(
            registrations,
            registration_keys,
            RegistrationSpec::Slice(element.as_ref().clone()),
        );
        top
    } else {
        let ty = collection.rust_ty.clone();
        let top = push_requirement(
            requirements,
            requirement_keys,
            RequirementSpec::new(ty.clone(), representation, location),
        );
        push_value_registration(registrations, registration_keys, ty);
        top
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_collection_children(
    collection: &BorrowedCollectionShape,
    location: &str,
    requirements: &mut Vec<RequirementSpec>,
    requirement_keys: &mut HashSet<String>,
    registrations: &mut Vec<RegistrationSpec>,
    registration_keys: &mut HashSet<String>,
) {
    if let Some(element) = &collection.slice_element {
        collect_owned_type(
            element,
            location,
            requirements,
            requirement_keys,
            registrations,
            registration_keys,
        );
        return;
    }
    match &collection.rust_ty {
        Type::Array(array) => {
            collect_owned_type(
                &array.elem,
                location,
                requirements,
                requirement_keys,
                registrations,
                registration_keys,
            );
        }
        ty => {
            for argument in type_generic_args(ty) {
                collect_owned_type(
                    argument,
                    location,
                    requirements,
                    requirement_keys,
                    registrations,
                    registration_keys,
                );
            }
        }
    }
}

fn push_requirement(
    requirements: &mut Vec<RequirementSpec>,
    keys: &mut HashSet<String>,
    requirement: RequirementSpec,
) -> usize {
    let key = requirement.key();
    if let Some(index) = requirements
        .iter()
        .position(|existing| existing.key() == key)
    {
        return index;
    }
    keys.insert(key);
    let index = requirements.len();
    requirements.push(requirement);
    index
}

fn push_value_registration(
    registrations: &mut Vec<RegistrationSpec>,
    keys: &mut HashSet<String>,
    ty: Type,
) {
    push_registration(registrations, keys, RegistrationSpec::Value(ty));
}

fn push_registration(
    registrations: &mut Vec<RegistrationSpec>,
    keys: &mut HashSet<String>,
    registration: RegistrationSpec,
) {
    if keys.insert(registration.key()) {
        registrations.push(registration);
    }
}

struct RequirementSpec {
    ty: Type,
    representation: TokenStream,
    location: String,
}

impl RequirementSpec {
    fn new(ty: Type, representation: TokenStream, location: impl Into<String>) -> Self {
        Self {
            ty,
            representation,
            location: location.into(),
        }
    }

    fn owned(ty: Type, location: impl Into<String>) -> Self {
        Self::new(
            ty,
            quote! { ::vela_common::InteropRepresentation::Owned },
            location,
        )
    }

    fn key(&self) -> String {
        format!("{}:{}", self.ty.to_token_stream(), self.representation)
    }
}

enum RegistrationSpec {
    Value(Type),
    Slice(Type),
}

impl RegistrationSpec {
    fn key(&self) -> String {
        match self {
            Self::Value(ty) => format!("value:{}", ty.to_token_stream()),
            Self::Slice(ty) => format!("slice:{}", ty.to_token_stream()),
        }
    }

    fn tokens(&self) -> TokenStream {
        match self {
            Self::Value(ty) => quote! {
                let builder = builder.register_rust_value_closure::<#ty>();
            },
            Self::Slice(element) => quote! {
                let builder = builder.register_rust_slice::<#element>();
            },
        }
    }
}

fn requirement_ident(index: usize) -> syn::Ident {
    format_ident!("__vela_requirement_{index}")
}

fn host_representation(access: HostAccess) -> TokenStream {
    match access {
        HostAccess::Shared => quote! { ::vela_common::InteropRepresentation::SharedHost },
        HostAccess::Exclusive => quote! { ::vela_common::InteropRepresentation::ExclusiveHost },
    }
}

fn collection_representation(collection: &BorrowedCollectionShape) -> TokenStream {
    let kind = match &collection.kind {
        BorrowedCollectionKind::Array(_) => quote! { ::vela_common::CollectionViewKind::Array },
        BorrowedCollectionKind::Map(_, _) => quote! { ::vela_common::CollectionViewKind::Map },
        BorrowedCollectionKind::Set(_) => quote! { ::vela_common::CollectionViewKind::Set },
    };
    match collection.access {
        HostAccess::Shared => {
            quote! { ::vela_common::InteropRepresentation::CollectionView(#kind) }
        }
        HostAccess::Exclusive => {
            let mutation = match collection.mutation {
                vela_common::CollectionViewMutation::Fixed => {
                    quote! { ::vela_common::CollectionViewMutation::Fixed }
                }
                vela_common::CollectionViewMutation::Growable => {
                    quote! { ::vela_common::CollectionViewMutation::Growable }
                }
            };
            quote! {
                ::vela_common::InteropRepresentation::CollectionMut {
                    kind: #kind,
                    mutation: #mutation,
                }
            }
        }
    }
}

fn service_return_mode_tokens(mode: ReturnMode, shape: &TypeShape) -> Result<TokenStream> {
    let tokens = return_mode_tokens(mode, shape);
    let ReturnMode::ScopedHost {
        origin: crate::export::signature::BorrowOrigin::Parameter(index),
        child,
        parent,
    } = mode
    else {
        return Ok(tokens);
    };
    let adjusted = index.checked_sub(1).ok_or_else(|| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            "service borrowed return cannot use its service receiver as a parameter origin",
        )
    })?;
    let child = match child {
        HostAccess::Shared => quote! { ::vela_engine::interop::ScopedHostAccess::Shared },
        HostAccess::Exclusive => quote! { ::vela_engine::interop::ScopedHostAccess::Exclusive },
    };
    let parent = match parent {
        HostAccess::Shared => quote! { ::vela_engine::interop::ScopedHostAccess::Shared },
        HostAccess::Exclusive => quote! { ::vela_engine::interop::ScopedHostAccess::Exclusive },
    };
    Ok(quote! {
        ::vela_engine::interop::ReturnMode::ScopedHost {
            origin: ::vela_engine::interop::BorrowedReturnOrigin::Parameter(#adjusted),
            child_access: #child,
            parent_freeze: #parent,
        }
    })
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
        assert!(!output.contains("HostRef"));
        assert!(!output.contains("Runtime"));
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
