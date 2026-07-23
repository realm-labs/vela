use std::collections::BTreeSet;

use syn::{FnArg, PatType, Result, ReturnType, Signature, Type, TypePath};

use crate::signature::{param_name, type_generic_args, type_ident, wrapper_inner_type};

const BOUNDARY_WRAPPERS: &[&str] = &[
    "HostRef",
    "HostPath",
    "PathProxy",
    "HostLeaseRef",
    "HostLeaseMut",
    "HostAccess",
    "CallArgs",
    "OwnedValue",
    "HostValue",
    "VelaValue",
];

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum EffectName {
    Pure,
    HostRead,
    HostWrite,
    EventEmit,
    Time,
    Random,
    IoRead,
    IoWrite,
    ReflectionRead,
    ReflectionWrite,
    ReflectionCall,
}

impl EffectName {
    pub(super) fn parse(ident: &syn::Ident) -> Result<Self> {
        match ident.to_string().as_str() {
            "host_read" => Ok(Self::HostRead),
            "host_write" => Ok(Self::HostWrite),
            "event_emit" => Ok(Self::EventEmit),
            "time" => Ok(Self::Time),
            "random" => Ok(Self::Random),
            "io_read" => Ok(Self::IoRead),
            "io_write" => Ok(Self::IoWrite),
            "reflection_read" => Ok(Self::ReflectionRead),
            "reflection_write" => Ok(Self::ReflectionWrite),
            "reflection_call" => Ok(Self::ReflectionCall),
            "pure" => Ok(Self::Pure),
            _ => Err(syn::Error::new(
                ident.span(),
                "unsupported Vela callable effect",
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ParameterMode {
    Value,
    ReadOnlyValueBorrow,
    SharedHost,
    ExclusiveHost,
    HiddenContext,
}

#[derive(Clone)]
pub(super) struct ClassifiedParameter {
    pub(super) name: String,
    pub(super) ty: TypeShape,
    pub(super) mode: ParameterMode,
    pub(super) rust_ty: Option<Type>,
}

#[derive(Clone)]
pub(super) enum TypeShape {
    Unit,
    Bool,
    Char,
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
    String,
    Bytes,
    Array(Box<TypeShape>),
    Map(Box<TypeShape>, Box<TypeShape>),
    Set(Box<TypeShape>),
    Tuple(Vec<TypeShape>),
    Option(Box<TypeShape>),
    Result(Box<TypeShape>, Box<TypeShape>),
    Value(Type),
    Host(Type, HostAccess),
    BorrowedCollection(BorrowedCollectionShape),
    ReceiverHost,
}

#[derive(Clone)]
pub(super) struct BorrowedCollectionShape {
    pub(super) rust_ty: Type,
    pub(super) slice_element: Option<Box<Type>>,
    pub(super) kind: BorrowedCollectionKind,
    pub(super) access: HostAccess,
    pub(super) mutation: vela_common::CollectionViewMutation,
}

#[derive(Clone)]
pub(super) enum BorrowedCollectionKind {
    Array(Box<TypeShape>),
    Map(Box<TypeShape>, Box<TypeShape>),
    Set(Box<TypeShape>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum HostAccess {
    Shared,
    Exclusive,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ReturnMode {
    Owned,
    Structured,
    Boundary,
    ScopedHost {
        origin: BorrowOrigin,
        child: HostAccess,
        parent: HostAccess,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum BorrowOrigin {
    Receiver,
    Parameter(u16),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ErrorMode {
    Value,
    RuntimeResult,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScopedReturnContainer {
    Direct,
    Option,
    Result,
}

#[derive(Clone)]
pub(super) struct ClassifiedReturn {
    pub(super) ty: TypeShape,
    pub(super) mode: ReturnMode,
    pub(super) error_mode: ErrorMode,
}

#[derive(Clone)]
pub(crate) struct ClassifiedSignature {
    pub(super) parameters: Vec<ClassifiedParameter>,
    pub(super) returns: ClassifiedReturn,
    pub(super) effects: BTreeSet<EffectName>,
    pub(super) is_async: bool,
}

pub(crate) fn classify_function(
    signature: &Signature,
    additional_effects: &BTreeSet<EffectName>,
) -> Result<ClassifiedSignature> {
    classify_signature(signature, additional_effects, false)
}

pub(crate) fn classify_method(
    signature: &Signature,
    additional_effects: &BTreeSet<EffectName>,
) -> Result<ClassifiedSignature> {
    classify_signature(signature, additional_effects, true)
}

fn classify_signature(
    signature: &Signature,
    additional_effects: &BTreeSet<EffectName>,
    allow_receiver: bool,
) -> Result<ClassifiedSignature> {
    let mut parameters: Vec<ClassifiedParameter> = Vec::new();
    let mut host_origins = Vec::new();
    let mut receiver_access = None;
    for input in &signature.inputs {
        let classified = match input {
            FnArg::Typed(parameter) => classify_parameter(parameter)?,
            FnArg::Receiver(receiver) if allow_receiver => {
                if !parameters.is_empty() {
                    return Err(syn::Error::new_spanned(
                        receiver,
                        "exported Rust method receiver must be first",
                    ));
                }
                if receiver.reference.is_none() {
                    return Err(syn::Error::new_spanned(
                        receiver,
                        "exported Rust methods require `&self` or `&mut self`",
                    ));
                }
                let access = if receiver.mutability.is_some() {
                    HostAccess::Exclusive
                } else {
                    HostAccess::Shared
                };
                receiver_access = Some(access);
                ClassifiedParameter {
                    name: "self".to_owned(),
                    ty: TypeShape::ReceiverHost,
                    mode: if access == HostAccess::Exclusive {
                        ParameterMode::ExclusiveHost
                    } else {
                        ParameterMode::SharedHost
                    },
                    rust_ty: None,
                }
            }
            FnArg::Receiver(receiver) => {
                return Err(syn::Error::new_spanned(
                    receiver,
                    "#[vela::export] functions cannot use a self receiver",
                ));
            }
        };
        if classified.mode == ParameterMode::HiddenContext
            && parameters
                .iter()
                .any(|parameter| !matches!(parameter.ty, TypeShape::ReceiverHost))
        {
            return Err(syn::Error::new_spanned(
                input,
                "NativeCallContext must be the first exported Rust parameter",
            ));
        }
        if matches!(
            classified.mode,
            ParameterMode::SharedHost | ParameterMode::ExclusiveHost
        ) {
            let index = u16::try_from(parameters.len()).map_err(|_| {
                syn::Error::new_spanned(input, "exported callable has too many parameters")
            })?;
            host_origins.push((index, classified.mode));
        }
        parameters.push(classified);
    }

    let (return_shape, error_mode) = classify_return_type(&signature.output)?;
    let host_return = host_return_access(&return_shape)?;
    let mode = if let Some(child) = host_return {
        let (origin, parent) = if let Some(parent) = receiver_access {
            (BorrowOrigin::Receiver, parent)
        } else {
            let [(index, parent_mode)] = host_origins.as_slice() else {
                return Err(syn::Error::new_spanned(
                    &signature.output,
                    "borrowed host return requires exactly one unambiguous borrowed host parameter",
                ));
            };
            let parent = match parent_mode {
                ParameterMode::SharedHost => HostAccess::Shared,
                ParameterMode::ExclusiveHost => HostAccess::Exclusive,
                _ => unreachable!("host origins contain only host modes"),
            };
            (BorrowOrigin::Parameter(*index), parent)
        };
        if parent == HostAccess::Shared && child == HostAccess::Exclusive {
            return Err(syn::Error::new_spanned(
                &signature.output,
                "a shared host origin cannot return a mutable host borrow",
            ));
        }
        ReturnMode::ScopedHost {
            origin,
            child,
            parent,
        }
    } else if return_shape.is_structured() {
        ReturnMode::Structured
    } else if matches!(return_shape, TypeShape::Value(_)) {
        ReturnMode::Boundary
    } else {
        ReturnMode::Owned
    };

    let mut effects = inferred_effects(&parameters);
    effects.extend(additional_effects.iter().copied());
    if effects.contains(&EffectName::HostWrite) {
        effects.remove(&EffectName::HostRead);
    }
    if effects.len() > 1 {
        effects.remove(&EffectName::Pure);
    }

    Ok(ClassifiedSignature {
        parameters,
        returns: ClassifiedReturn {
            ty: return_shape,
            mode,
            error_mode,
        },
        effects,
        is_async: signature.asyncness.is_some(),
    })
}

fn classify_parameter(parameter: &PatType) -> Result<ClassifiedParameter> {
    let name = param_name(parameter);
    if type_ident(&parameter.ty).is_some_and(|ident| ident == "NativeCallContext") {
        let Type::Reference(reference) = parameter.ty.as_ref() else {
            return Err(syn::Error::new_spanned(
                &parameter.ty,
                "NativeCallContext must be `&mut NativeCallContext<'_, '_>`",
            ));
        };
        if reference.mutability.is_none() {
            return Err(syn::Error::new_spanned(
                &parameter.ty,
                "NativeCallContext must be a mutable hidden context parameter",
            ));
        }
        return Ok(ClassifiedParameter {
            name,
            ty: TypeShape::Unit,
            mode: ParameterMode::HiddenContext,
            rust_ty: Some(parameter.ty.as_ref().clone()),
        });
    }
    if let Type::Reference(reference) = parameter.ty.as_ref() {
        reject_explicit_lifetime(reference)?;
        if is_str(&reference.elem) {
            if reference.mutability.is_some() {
                return Err(syn::Error::new_spanned(
                    &parameter.ty,
                    "mutable string borrows are not supported at the Vela boundary",
                ));
            }
            return Ok(ClassifiedParameter {
                name,
                ty: TypeShape::String,
                mode: ParameterMode::ReadOnlyValueBorrow,
                rust_ty: Some(parameter.ty.as_ref().clone()),
            });
        }
        if is_u8_slice(&reference.elem) {
            if reference.mutability.is_some() {
                return Err(syn::Error::new_spanned(
                    &parameter.ty,
                    "mutable byte-slice borrows are not supported at the Vela boundary",
                ));
            }
            return Ok(ClassifiedParameter {
                name,
                ty: TypeShape::Bytes,
                mode: ParameterMode::ReadOnlyValueBorrow,
                rust_ty: Some(parameter.ty.as_ref().clone()),
            });
        }
        let access = if reference.mutability.is_some() {
            HostAccess::Exclusive
        } else {
            HostAccess::Shared
        };
        if let Some(collection) = borrowed_collection_type(&reference.elem, access)? {
            return Ok(ClassifiedParameter {
                name,
                ty: collection,
                mode: if access == HostAccess::Exclusive {
                    ParameterMode::ExclusiveHost
                } else {
                    ParameterMode::SharedHost
                },
                rust_ty: Some(parameter.ty.as_ref().clone()),
            });
        }
        let host_ty = direct_host_type(&reference.elem)?;
        return Ok(ClassifiedParameter {
            name,
            ty: TypeShape::Host(host_ty, access),
            mode: if access == HostAccess::Exclusive {
                ParameterMode::ExclusiveHost
            } else {
                ParameterMode::SharedHost
            },
            rust_ty: Some(parameter.ty.as_ref().clone()),
        });
    }
    Ok(ClassifiedParameter {
        name,
        ty: classify_owned_type(&parameter.ty)?,
        mode: ParameterMode::Value,
        rust_ty: Some(parameter.ty.as_ref().clone()),
    })
}

fn classify_return_type(output: &ReturnType) -> Result<(TypeShape, ErrorMode)> {
    let ReturnType::Type(_, ty) = output else {
        return Ok((TypeShape::Unit, ErrorMode::Value));
    };
    if let Some(inner) = wrapper_inner_type(ty, &["VmResult"]) {
        return Ok((classify_return_shape(inner)?, ErrorMode::RuntimeResult));
    }
    Ok((classify_return_shape(ty)?, ErrorMode::Value))
}

fn classify_return_shape(ty: &Type) -> Result<TypeShape> {
    if let Type::Reference(reference) = ty {
        reject_explicit_lifetime(reference)?;
        if is_str(&reference.elem) || is_u8_slice(&reference.elem) {
            return Err(syn::Error::new_spanned(
                ty,
                "borrowed scalar or container views cannot leave an exported Rust invocation",
            ));
        }
        let access = if reference.mutability.is_some() {
            HostAccess::Exclusive
        } else {
            HostAccess::Shared
        };
        if let Some(collection) = borrowed_collection_type(&reference.elem, access)? {
            return Ok(collection);
        }
        let host_ty = direct_host_type(&reference.elem)?;
        return Ok(TypeShape::Host(host_ty, access));
    }
    if let Some(inner) = wrapper_inner_type(ty, &["Option"]) {
        return Ok(TypeShape::Option(Box::new(classify_return_shape(inner)?)));
    }
    if let Type::Tuple(tuple) = ty {
        if tuple.elems.is_empty() {
            return Ok(TypeShape::Unit);
        }
        return tuple
            .elems
            .iter()
            .map(classify_return_shape)
            .collect::<Result<Vec<_>>>()
            .map(TypeShape::Tuple);
    }
    if type_ident(ty).is_some_and(|ident| ident == "Result") {
        let args = type_generic_args(ty);
        let [ok, err] = args.as_slice() else {
            return Err(syn::Error::new_spanned(
                ty,
                "Result boundary type requires exactly two type arguments",
            ));
        };
        return Ok(TypeShape::Result(
            Box::new(classify_return_shape(ok)?),
            Box::new(classify_owned_type(err)?),
        ));
    }
    classify_owned_type(ty)
}

fn classify_owned_type(ty: &Type) -> Result<TypeShape> {
    reject_boundary_wrapper(ty)?;
    if let Type::Tuple(tuple) = ty {
        if tuple.elems.is_empty() {
            return Ok(TypeShape::Unit);
        }
        return tuple
            .elems
            .iter()
            .map(classify_owned_type)
            .collect::<Result<Vec<_>>>()
            .map(TypeShape::Tuple);
    }
    if let Type::Array(array) = ty {
        return Ok(TypeShape::Array(Box::new(classify_owned_type(
            &array.elem,
        )?)));
    }
    let Some(ident) = type_ident(ty) else {
        return Err(syn::Error::new_spanned(
            ty,
            "unsupported exported Rust boundary type",
        ));
    };
    let primitive = match ident.as_str() {
        "bool" => Some(TypeShape::Bool),
        "char" => Some(TypeShape::Char),
        "i8" => Some(TypeShape::I8),
        "i16" => Some(TypeShape::I16),
        "i32" => Some(TypeShape::I32),
        "i64" => Some(TypeShape::I64),
        "u8" => Some(TypeShape::U8),
        "u16" => Some(TypeShape::U16),
        "u32" => Some(TypeShape::U32),
        "u64" => Some(TypeShape::U64),
        "f32" => Some(TypeShape::F32),
        "f64" => Some(TypeShape::F64),
        "String" => Some(TypeShape::String),
        "i128" | "u128" | "isize" | "usize" => {
            return Err(syn::Error::new_spanned(
                ty,
                "128-bit and pointer-sized integers are not supported at the Vela boundary",
            ));
        }
        _ => None,
    };
    if let Some(primitive) = primitive {
        return Ok(primitive);
    }
    let args = type_generic_args(ty);
    match ident.as_str() {
        "Vec" => unary_shape(ty, &args, TypeShape::Array),
        "Option" => unary_shape(ty, &args, TypeShape::Option),
        "BTreeSet" | "HashSet" => unary_shape(ty, &args, TypeShape::Set),
        "BTreeMap" | "HashMap" => {
            let [key, value] = args.as_slice() else {
                return Err(syn::Error::new_spanned(
                    ty,
                    "map boundary type requires exactly two type arguments",
                ));
            };
            Ok(TypeShape::Map(
                Box::new(classify_owned_type(key)?),
                Box::new(classify_owned_type(value)?),
            ))
        }
        "Result" => {
            let [ok, err] = args.as_slice() else {
                return Err(syn::Error::new_spanned(
                    ty,
                    "Result boundary type requires exactly two type arguments",
                ));
            };
            Ok(TypeShape::Result(
                Box::new(classify_owned_type(ok)?),
                Box::new(classify_owned_type(err)?),
            ))
        }
        _ if args.is_empty() => Ok(TypeShape::Value(ty.clone())),
        _ => Ok(TypeShape::Value(ty.clone())),
    }
}

fn unary_shape(
    ty: &Type,
    args: &[&Type],
    constructor: impl FnOnce(Box<TypeShape>) -> TypeShape,
) -> Result<TypeShape> {
    let [inner] = args else {
        return Err(syn::Error::new_spanned(
            ty,
            "boundary container requires exactly one type argument",
        ));
    };
    Ok(constructor(Box::new(classify_owned_type(inner)?)))
}

fn direct_host_type(ty: &Type) -> Result<Type> {
    reject_boundary_wrapper(ty)?;
    let Type::Path(TypePath { qself: None, path }) = ty else {
        return Err(syn::Error::new_spanned(
            ty,
            "host reference must name one exact concrete Rust type",
        ));
    };
    if path
        .segments
        .last()
        .is_some_and(|segment| !matches!(segment.arguments, syn::PathArguments::None))
    {
        return Err(syn::Error::new_spanned(
            ty,
            "generic host reference types are unsupported",
        ));
    }
    let ident = path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
        .unwrap_or_default();
    if matches!(
        ident.as_str(),
        "bool"
            | "char"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "f32"
            | "f64"
            | "String"
    ) {
        return Err(syn::Error::new_spanned(
            ty,
            "only registered direct host objects may use shared or mutable host parameters",
        ));
    }
    Ok(ty.clone())
}

fn borrowed_collection_type(ty: &Type, access: HostAccess) -> Result<Option<TypeShape>> {
    if let Type::Slice(slice) = ty {
        if type_ident(&slice.elem).is_some_and(|ident| ident == "u8") {
            return Err(syn::Error::new_spanned(
                ty,
                "borrowed [u8] views are not supported yet; use an owned byte value",
            ));
        }
        return Ok(Some(TypeShape::BorrowedCollection(
            BorrowedCollectionShape {
                rust_ty: ty.clone(),
                slice_element: Some(Box::new((*slice.elem).clone())),
                kind: BorrowedCollectionKind::Array(Box::new(classify_owned_type(&slice.elem)?)),
                access,
                mutation: vela_common::CollectionViewMutation::Fixed,
            },
        )));
    }
    if let Type::Array(array) = ty {
        if type_ident(&array.elem).is_some_and(|ident| ident == "u8") {
            return Err(syn::Error::new_spanned(
                ty,
                "borrowed [u8; N] views are not supported yet; use an owned byte value",
            ));
        }
        return Ok(Some(TypeShape::BorrowedCollection(
            BorrowedCollectionShape {
                rust_ty: ty.clone(),
                slice_element: None,
                kind: BorrowedCollectionKind::Array(Box::new(classify_owned_type(&array.elem)?)),
                access,
                mutation: vela_common::CollectionViewMutation::Fixed,
            },
        )));
    }
    let Some(ident) = type_ident(ty) else {
        return Ok(None);
    };
    let args = type_generic_args(ty);
    let kind = match ident.as_str() {
        "Vec" => {
            let [element] = args.as_slice() else {
                return Err(syn::Error::new_spanned(
                    ty,
                    "borrowed Vec boundary type requires exactly one type argument",
                ));
            };
            if type_ident(element).is_some_and(|ident| ident == "u8") {
                return Err(syn::Error::new_spanned(
                    ty,
                    "borrowed Vec<u8> views are not supported yet; use an owned byte value",
                ));
            }
            BorrowedCollectionKind::Array(Box::new(classify_owned_type(element)?))
        }
        "BTreeMap" | "HashMap" => {
            let [key, value] = args.as_slice() else {
                return Err(syn::Error::new_spanned(
                    ty,
                    "borrowed map boundary type requires exactly two type arguments",
                ));
            };
            BorrowedCollectionKind::Map(
                Box::new(classify_owned_type(key)?),
                Box::new(classify_owned_type(value)?),
            )
        }
        "BTreeSet" | "HashSet" => {
            let [element] = args.as_slice() else {
                return Err(syn::Error::new_spanned(
                    ty,
                    "borrowed set boundary type requires exactly one type argument",
                ));
            };
            BorrowedCollectionKind::Set(Box::new(classify_owned_type(element)?))
        }
        _ => return Ok(None),
    };
    Ok(Some(TypeShape::BorrowedCollection(
        BorrowedCollectionShape {
            rust_ty: ty.clone(),
            slice_element: None,
            kind,
            access,
            mutation: vela_common::CollectionViewMutation::Growable,
        },
    )))
}

fn reject_boundary_wrapper(ty: &Type) -> Result<()> {
    if type_ident(ty).is_some_and(|ident| BOUNDARY_WRAPPERS.contains(&ident.as_str())) {
        return Err(syn::Error::new_spanned(
            ty,
            "ordinary exported signatures cannot mention a Vela boundary wrapper",
        ));
    }
    Ok(())
}

fn reject_explicit_lifetime(reference: &syn::TypeReference) -> Result<()> {
    if reference
        .lifetime
        .as_ref()
        .is_some_and(|lifetime| lifetime.ident != "_")
    {
        return Err(syn::Error::new_spanned(
            reference,
            "exported Rust references cannot expose an explicit named lifetime",
        ));
    }
    Ok(())
}

fn inferred_effects(parameters: &[ClassifiedParameter]) -> BTreeSet<EffectName> {
    let mut effects = BTreeSet::new();
    if parameters
        .iter()
        .any(|parameter| parameter.mode == ParameterMode::ExclusiveHost)
    {
        effects.insert(EffectName::HostWrite);
    } else if parameters
        .iter()
        .any(|parameter| parameter.mode == ParameterMode::SharedHost)
    {
        effects.insert(EffectName::HostRead);
    } else {
        effects.insert(EffectName::Pure);
    }
    effects
}

fn host_return_access(shape: &TypeShape) -> Result<Option<HostAccess>> {
    match shape {
        TypeShape::Host(_, access) => Ok(Some(*access)),
        TypeShape::BorrowedCollection(collection) => Ok(Some(collection.access)),
        TypeShape::Option(inner) => host_return_access(inner),
        TypeShape::Result(ok, _) => host_return_access(ok),
        TypeShape::Tuple(elements) => {
            let mut access = None;
            for element in elements {
                let Some((_, element_access)) = element.host_boundary() else {
                    if host_return_access(element)?.is_some() || access.is_some() {
                        return Err(syn::Error::new(
                            proc_macro2::Span::call_site(),
                            "borrowed host tuples must contain only direct host references",
                        ));
                    }
                    continue;
                };
                if access.is_some_and(|access| access != element_access) {
                    return Err(syn::Error::new(
                        proc_macro2::Span::call_site(),
                        "borrowed host tuples must use one shared or exclusive access mode",
                    ));
                }
                access = Some(element_access);
            }
            if access.is_some() && elements.iter().any(|item| item.host_boundary().is_none()) {
                return Err(syn::Error::new(
                    proc_macro2::Span::call_site(),
                    "borrowed host tuples cannot mix owned values and host references",
                ));
            }
            Ok(access)
        }
        TypeShape::Unit
        | TypeShape::Bool
        | TypeShape::Char
        | TypeShape::I8
        | TypeShape::I16
        | TypeShape::I32
        | TypeShape::I64
        | TypeShape::U8
        | TypeShape::U16
        | TypeShape::U32
        | TypeShape::U64
        | TypeShape::F32
        | TypeShape::F64
        | TypeShape::String
        | TypeShape::Bytes
        | TypeShape::Array(_)
        | TypeShape::Map(_, _)
        | TypeShape::Set(_)
        | TypeShape::Value(_)
        | TypeShape::ReceiverHost => Ok(None),
    }
}

impl TypeShape {
    pub(super) fn host_boundary(&self) -> Option<(&Type, HostAccess)> {
        match self {
            Self::Host(ty, access) => Some((ty, *access)),
            Self::BorrowedCollection(collection) => Some((&collection.rust_ty, collection.access)),
            _ => None,
        }
    }

    pub(super) fn borrowed_slice_element(&self) -> Option<&Type> {
        match self {
            Self::BorrowedCollection(collection) => {
                collection.slice_element.as_ref().map(Box::as_ref)
            }
            _ => None,
        }
    }

    fn is_structured(&self) -> bool {
        matches!(
            self,
            Self::Array(_)
                | Self::Map(_, _)
                | Self::Set(_)
                | Self::Tuple(_)
                | Self::Option(_)
                | Self::Result(_, _)
        )
    }
}

impl ClassifiedSignature {
    pub(crate) fn supports_value_adapter(&self) -> bool {
        self.parameters.iter().all(|parameter| {
            matches!(
                parameter.mode,
                ParameterMode::Value | ParameterMode::HiddenContext
            )
        })
    }

    pub(crate) fn has_hidden_context(&self) -> bool {
        self.parameters
            .iter()
            .any(|parameter| parameter.mode == ParameterMode::HiddenContext)
    }

    pub(crate) fn supports_sync_host_adapter(&self) -> bool {
        !self.is_async
            && matches!(
                self.returns.mode,
                ReturnMode::Owned | ReturnMode::Structured | ReturnMode::Boundary
            )
            && self.parameters.iter().all(|parameter| {
                matches!(
                    parameter.mode,
                    ParameterMode::Value
                        | ParameterMode::SharedHost
                        | ParameterMode::ExclusiveHost
                        | ParameterMode::HiddenContext
                )
            })
            && self.parameters.iter().any(|parameter| {
                matches!(
                    parameter.mode,
                    ParameterMode::SharedHost | ParameterMode::ExclusiveHost
                )
            })
    }

    pub(crate) fn supports_async_host_adapter(&self) -> bool {
        self.is_async
            && !self.has_hidden_context()
            && matches!(
                self.returns.mode,
                ReturnMode::Owned | ReturnMode::Structured | ReturnMode::Boundary
            )
            && self.parameters.iter().all(|parameter| {
                matches!(
                    parameter.mode,
                    ParameterMode::Value | ParameterMode::SharedHost | ParameterMode::ExclusiveHost
                )
            })
            && self.parameters.iter().any(|parameter| {
                matches!(
                    parameter.mode,
                    ParameterMode::SharedHost | ParameterMode::ExclusiveHost
                )
            })
    }

    pub(crate) fn supports_sync_scoped_host_adapter(&self) -> bool {
        !self.is_async
            && !self.has_hidden_context()
            && matches!(
                self.returns.mode,
                ReturnMode::ScopedHost {
                    origin: BorrowOrigin::Parameter(_),
                    ..
                }
            )
            && self.scoped_return_container().is_some()
            && self.parameters.iter().all(|parameter| {
                matches!(
                    parameter.mode,
                    ParameterMode::Value | ParameterMode::SharedHost | ParameterMode::ExclusiveHost
                )
            })
    }

    pub(crate) fn scoped_return_container(&self) -> Option<ScopedReturnContainer> {
        match &self.returns.ty {
            shape if shape.host_boundary().is_some() => Some(ScopedReturnContainer::Direct),
            TypeShape::Option(inner) if inner.host_boundary().is_some() => {
                Some(ScopedReturnContainer::Option)
            }
            TypeShape::Result(ok, _) if ok.host_boundary().is_some() => {
                Some(ScopedReturnContainer::Result)
            }
            TypeShape::Tuple(elements)
                if elements.iter().all(|item| item.host_boundary().is_some()) =>
            {
                Some(ScopedReturnContainer::Direct)
            }
            TypeShape::Option(inner) if matches!(&**inner, TypeShape::Tuple(elements) if elements.iter().all(|item| item.host_boundary().is_some())) => {
                Some(ScopedReturnContainer::Option)
            }
            TypeShape::Result(ok, _) if matches!(&**ok, TypeShape::Tuple(elements) if elements.iter().all(|item| item.host_boundary().is_some())) => {
                Some(ScopedReturnContainer::Result)
            }
            _ => None,
        }
    }

    pub(crate) fn supports_sync_scoped_method_adapter(&self) -> bool {
        !self.is_async
            && !self.has_hidden_context()
            && matches!(
                self.returns.mode,
                ReturnMode::ScopedHost {
                    origin: BorrowOrigin::Receiver,
                    ..
                }
            )
            && self.scoped_return_container().is_some()
            && self
                .parameters
                .first()
                .is_some_and(|parameter| matches!(parameter.ty, TypeShape::ReceiverHost))
            && self
                .parameters
                .iter()
                .skip(1)
                .all(|parameter| parameter.mode == ParameterMode::Value)
    }

    pub(crate) fn supports_sync_method_adapter(&self) -> bool {
        self.supports_sync_host_adapter()
            && !self.has_hidden_context()
            && self
                .parameters
                .first()
                .is_some_and(|parameter| matches!(parameter.ty, TypeShape::ReceiverHost))
    }

    pub(crate) fn supports_async_method_adapter(&self) -> bool {
        self.is_async
            && matches!(
                self.returns.mode,
                ReturnMode::Owned | ReturnMode::Structured | ReturnMode::Boundary
            )
            && self
                .parameters
                .first()
                .is_some_and(|parameter| matches!(parameter.ty, TypeShape::ReceiverHost))
            && self.parameters.iter().all(|parameter| {
                matches!(
                    parameter.mode,
                    ParameterMode::Value
                        | ParameterMode::SharedHost
                        | ParameterMode::ExclusiveHost
                        | ParameterMode::HiddenContext
                )
            })
    }
}

fn is_str(ty: &Type) -> bool {
    type_ident(ty).is_some_and(|ident| ident == "str")
}

fn is_u8_slice(ty: &Type) -> bool {
    matches!(ty, Type::Slice(slice) if type_ident(&slice.elem).is_some_and(|ident| ident == "u8"))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use quote::quote;
    use syn::{ItemFn, parse2};

    use super::{
        BorrowedCollectionKind, EffectName, HostAccess, ParameterMode, ReturnMode, TypeShape,
        classify_function, classify_method,
    };

    fn classify(tokens: proc_macro2::TokenStream) -> super::ClassifiedSignature {
        let item = parse2::<ItemFn>(tokens).expect("test function parses");
        classify_function(&item.sig, &BTreeSet::new()).expect("signature classifies")
    }

    #[test]
    fn signature_infers_normalized_effects() {
        let pure = classify(quote! { fn normalize(value: i64) -> i64 { value } });
        let shared = classify(quote! { fn level(player: &Player) -> i64 { 0 } });
        let exclusive = classify(quote! { fn grant(player: &mut Player) {} });

        assert_eq!(pure.effects, BTreeSet::from([EffectName::Pure]));
        assert_eq!(shared.effects, BTreeSet::from([EffectName::HostRead]));
        assert_eq!(exclusive.effects, BTreeSet::from([EffectName::HostWrite]));
        assert_eq!(exclusive.parameters[0].mode, ParameterMode::ExclusiveHost);
    }

    #[test]
    fn standard_collection_references_classify_as_host_backed_views() {
        let classified = classify(quote! {
            fn patch(
                values: &Vec<i64>,
                scores: &mut BTreeMap<String, i64>,
                tags: &HashSet<String>,
            ) {}
        });

        assert!(matches!(
            &classified.parameters[0].ty,
            TypeShape::BorrowedCollection(collection)
                if collection.access == HostAccess::Shared
                    && matches!(collection.kind, BorrowedCollectionKind::Array(_))
        ));
        assert!(matches!(
            &classified.parameters[1].ty,
            TypeShape::BorrowedCollection(collection)
                if collection.access == HostAccess::Exclusive
                    && matches!(collection.kind, BorrowedCollectionKind::Map(_, _))
        ));
        assert!(matches!(
            &classified.parameters[2].ty,
            TypeShape::BorrowedCollection(collection)
                if collection.access == HostAccess::Shared
                    && matches!(collection.kind, BorrowedCollectionKind::Set(_))
        ));
        assert_eq!(classified.effects, BTreeSet::from([EffectName::HostWrite]));
    }

    #[test]
    fn explicit_effects_only_add_to_inferred_base() {
        let item = parse2::<ItemFn>(quote! {
            fn notify(player: &mut Player) {}
        })
        .expect("test function parses");
        let classified = classify_function(
            &item.sig,
            &BTreeSet::from([EffectName::Random, EffectName::EventEmit]),
        )
        .expect("signature classifies");

        assert_eq!(
            classified.effects,
            BTreeSet::from([
                EffectName::HostWrite,
                EffectName::Random,
                EffectName::EventEmit,
            ])
        );
    }

    #[test]
    fn borrowed_return_requires_one_owner_origin() {
        let item = parse2::<ItemFn>(quote! {
            fn player(left: &Game, right: &Game) -> &Player { todo!() }
        })
        .expect("test function parses");
        let error = match classify_function(&item.sig, &BTreeSet::new()) {
            Ok(_) => panic!("ambiguous borrowed return must fail"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("exactly one unambiguous"));
    }

    #[test]
    fn method_receiver_infers_effect_and_borrow_origin() {
        let item = parse2::<syn::ImplItemFn>(quote! {
            pub fn player(&self) -> &Player { todo!() }
        })
        .expect("test method parses");
        let classified =
            classify_method(&item.sig, &BTreeSet::new()).expect("method signature classifies");

        assert_eq!(classified.effects, BTreeSet::from([EffectName::HostRead]));
        assert!(matches!(
            classified.returns.mode,
            ReturnMode::ScopedHost {
                origin: super::BorrowOrigin::Receiver,
                ..
            }
        ));
    }
}
