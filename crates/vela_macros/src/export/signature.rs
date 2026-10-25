use std::collections::BTreeSet;

use syn::{FnArg, PatType, Result, ReturnType, Signature, Type};

use crate::signature::{
    param_name, reject_async_signature, type_generic_args, type_ident, wrapper_inner_type,
};

const BOUNDARY_WRAPPERS: &[&str] = &[
    "HostRef",
    "HostPath",
    "PathProxy",
    "HostLeaseRef",
    "HostLeaseMut",
    "Host",
    "View",
    "MutView",
    "ScopedHost",
    "HostTargetPlan",
    "HostCallValue",
    "HostAccess",
    "CallArgs",
    "OwnedValue",
    "HostValue",
    "VelaValue",
];

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum EffectName {
    Pure,
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
    pub(crate) fn parse(ident: &syn::Ident) -> Result<Self> {
        match ident.to_string().as_str() {
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
pub(crate) enum ParameterMode {
    Value,
    ReadOnlyValueBorrow,
    HiddenContext,
}

#[derive(Clone)]
pub(crate) struct ClassifiedParameter {
    pub(crate) name: String,
    pub(crate) ty: TypeShape,
    pub(crate) mode: ParameterMode,
    pub(crate) rust_ty: Type,
}

#[derive(Clone)]
pub(crate) enum TypeShape {
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
    Array(Box<TypeShape>),
    Map(Box<TypeShape>, Box<TypeShape>),
    Set(Box<TypeShape>),
    Tuple(Vec<TypeShape>),
    Option(Box<TypeShape>),
    Result(Box<TypeShape>, Box<TypeShape>),
    Value(Type),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReturnMode {
    Owned,
    Structured,
    Boundary,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ErrorMode {
    Value,
    RuntimeResult,
}

#[derive(Clone)]
pub(crate) struct ClassifiedReturn {
    pub(crate) ty: TypeShape,
    pub(crate) mode: ReturnMode,
    pub(crate) error_mode: ErrorMode,
}

#[derive(Clone)]
pub(crate) struct ClassifiedSignature {
    pub(crate) parameters: Vec<ClassifiedParameter>,
    pub(crate) returns: ClassifiedReturn,
    pub(crate) effects: BTreeSet<EffectName>,
}

pub(crate) fn classify_function(
    signature: &Signature,
    additional_effects: &BTreeSet<EffectName>,
) -> Result<ClassifiedSignature> {
    reject_async_signature(signature)?;
    let mut parameters = Vec::new();
    for input in &signature.inputs {
        let FnArg::Typed(parameter) = input else {
            return Err(syn::Error::new_spanned(
                input,
                "#[vela_macros::export] functions cannot use a self receiver",
            ));
        };
        let classified = classify_parameter(parameter)?;
        if classified.mode == ParameterMode::HiddenContext && !parameters.is_empty() {
            return Err(syn::Error::new_spanned(
                input,
                "NativeCallContext must be the first exported Rust parameter",
            ));
        }
        parameters.push(classified);
    }
    let (return_shape, error_mode) = classify_return_type(&signature.output)?;
    let mode = if return_shape.is_structured() {
        ReturnMode::Structured
    } else if matches!(return_shape, TypeShape::Value(_)) {
        ReturnMode::Boundary
    } else {
        ReturnMode::Owned
    };
    let mut effects = additional_effects.clone();
    if effects.is_empty() {
        effects.insert(EffectName::Pure);
    }
    Ok(ClassifiedSignature {
        parameters,
        returns: ClassifiedReturn {
            ty: return_shape,
            mode,
            error_mode,
        },
        effects,
    })
}

fn classify_parameter(parameter: &PatType) -> Result<ClassifiedParameter> {
    let name = param_name(parameter);
    if type_ident(&parameter.ty).is_some_and(|ident| ident == "NativeCallContext") {
        let Type::Reference(reference) = parameter.ty.as_ref() else {
            return Err(syn::Error::new_spanned(
                &parameter.ty,
                "NativeCallContext must be `&mut NativeCallContext<'_>`",
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
            rust_ty: parameter.ty.as_ref().clone(),
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
                rust_ty: parameter.ty.as_ref().clone(),
            });
        }
        if reference.mutability.is_some() {
            return Err(syn::Error::new_spanned(
                &parameter.ty,
                "mutable borrowed parameters are not supported at the Vela value boundary",
            ));
        }
        if matches!(reference.elem.as_ref(), Type::Slice(_) | Type::Array(_))
            || type_ident(&reference.elem).is_some_and(|ident| {
                matches!(
                    ident.as_str(),
                    "Vec" | "BTreeMap" | "HashMap" | "BTreeSet" | "HashSet"
                )
            })
        {
            return Err(syn::Error::new_spanned(
                &parameter.ty,
                "borrowed collection parameters are removed; pass an owned collection value",
            ));
        }
        return Ok(ClassifiedParameter {
            name,
            ty: classify_owned_type(&reference.elem)?,
            mode: ParameterMode::ReadOnlyValueBorrow,
            rust_ty: parameter.ty.as_ref().clone(),
        });
    }
    Ok(ClassifiedParameter {
        name,
        ty: classify_owned_type(&parameter.ty)?,
        mode: ParameterMode::Value,
        rust_ty: parameter.ty.as_ref().clone(),
    })
}

fn classify_return_type(output: &ReturnType) -> Result<(TypeShape, ErrorMode)> {
    let ReturnType::Type(_, ty) = output else {
        return Ok((TypeShape::Unit, ErrorMode::Value));
    };
    if let Some(inner) = wrapper_inner_type(ty, &["VmResult"]) {
        return Ok((classify_owned_type(inner)?, ErrorMode::RuntimeResult));
    }
    Ok((classify_owned_type(ty)?, ErrorMode::Value))
}

fn classify_owned_type(ty: &Type) -> Result<TypeShape> {
    reject_boundary_wrapper(ty)?;
    if matches!(ty, Type::Reference(_)) {
        return Err(syn::Error::new_spanned(
            ty,
            "borrowed returns and nested references are removed; use owned values",
        ));
    }
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
        _ => {
            for arg in args {
                classify_owned_type(arg)?;
            }
            Ok(TypeShape::Value(ty.clone()))
        }
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

impl TypeShape {
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
    pub(crate) fn has_hidden_context(&self) -> bool {
        self.parameters
            .iter()
            .any(|parameter| parameter.mode == ParameterMode::HiddenContext)
    }
}

fn is_str(ty: &Type) -> bool {
    type_ident(ty).is_some_and(|ident| ident == "str")
}

#[cfg(test)]
mod tests;
