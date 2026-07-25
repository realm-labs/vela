use std::collections::HashSet;

use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};
use syn::{Result, ReturnType, Type, parse_quote};

use crate::export::emission::return_mode_tokens;
use crate::export::signature::{
    BorrowedCollectionKind, BorrowedCollectionShape, ClassifiedParameter, HostAccess,
    ParameterMode, ReturnMode, TypeShape,
};
use crate::signature::type_generic_args;

#[allow(clippy::too_many_arguments)]
pub(super) fn add_parameter_requirements(
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
pub(super) fn add_return_requirements(
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

pub(super) struct RequirementSpec {
    pub(super) ty: Type,
    pub(super) representation: TokenStream,
    pub(super) location: String,
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

pub(super) enum RegistrationSpec {
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

    pub(super) fn tokens(&self) -> TokenStream {
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

pub(super) fn requirement_ident(index: usize) -> syn::Ident {
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

pub(super) fn service_return_mode_tokens(
    mode: ReturnMode,
    shape: &TypeShape,
) -> Result<TokenStream> {
    let tokens = return_mode_tokens(mode, shape);
    let ReturnMode::ScopedHost {
        origin: crate::export::signature::BorrowOrigin::Parameter(index),
        child,
        parent,
    } = mode
    else {
        return Ok(tokens);
    };
    let adjusted = index;
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
