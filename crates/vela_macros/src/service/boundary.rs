use syn::{ReturnType, Type};

use crate::export::signature::{
    BorrowOrigin, ClassifiedSignature, ReturnMode, ScopedReturnContainer, TypeShape,
};
use crate::signature::{type_generic_args, type_ident};

pub(super) fn validate_return(output: &ReturnType) -> syn::Result<()> {
    let ReturnType::Type(_, ty) = output else {
        return Ok(());
    };

    match admitted_scoped_envelope(ty)? {
        ScopedEnvelope::Owned
        | ScopedEnvelope::Direct
        | ScopedEnvelope::SharedOption
        | ScopedEnvelope::SharedResult => Ok(()),
        ScopedEnvelope::ExclusiveOption => Err(unsupported_envelope(
            ty,
            "optional exclusive",
            "Option::Some -> mutable borrow",
        )),
        ScopedEnvelope::ExclusiveResult => Err(unsupported_envelope(
            ty,
            "fallible exclusive",
            "Result::Ok -> mutable borrow",
        )),
        ScopedEnvelope::Nested(path) => Err(syn::Error::new_spanned(
            ty,
            format!(
                "service return contains a call-scoped host borrow inside an owned container\nreturn path: {}",
                path.join(" -> "),
            ),
        )),
    }
}

pub(super) fn validate_outer_scoped_return(
    output: &ReturnType,
    signature: &ClassifiedSignature,
) -> syn::Result<()> {
    let ReturnMode::ScopedHost {
        origin: BorrowOrigin::Parameter(origin),
        ..
    } = signature.returns.mode
    else {
        return Ok(());
    };
    let origin = signature
        .parameters
        .iter()
        .skip(1)
        .nth(usize::from(origin))
        .ok_or_else(|| {
            syn::Error::new_spanned(output, "service borrowed return origin is missing")
        })?;
    let child = match signature.scoped_return_container() {
        Some(ScopedReturnContainer::Direct) => &signature.returns.ty,
        Some(ScopedReturnContainer::Option) => match &signature.returns.ty {
            TypeShape::Option(child) => child.as_ref(),
            _ => return Err(envelope_mismatch(output)),
        },
        Some(ScopedReturnContainer::Result) => match &signature.returns.ty {
            TypeShape::Result(child, _) => child.as_ref(),
            _ => return Err(envelope_mismatch(output)),
        },
        None => return Err(envelope_mismatch(output)),
    };
    let origin_key = direct_rust_type(&origin.ty)
        .map(quote::ToTokens::to_token_stream)
        .map(|tokens| tokens.to_string());
    let child_key = direct_rust_type(child)
        .map(quote::ToTokens::to_token_stream)
        .map(|tokens| tokens.to_string());
    if origin_key.is_some() && origin_key == child_key {
        return Ok(());
    }
    Err(syn::Error::new_spanned(
        output,
        "service borrowed return must be the exact direct host parameter; projected child references cannot be restored to an unchanged Rust return type without fabricating a reference",
    ))
}

fn direct_rust_type(shape: &TypeShape) -> Option<&Type> {
    match shape {
        TypeShape::StorageDirectedShared(ty) | TypeShape::Host(ty, _) => Some(ty),
        TypeShape::BorrowedCollection(collection) => Some(&collection.rust_ty),
        _ => None,
    }
}

fn envelope_mismatch(output: &ReturnType) -> syn::Error {
    syn::Error::new_spanned(
        output,
        "service borrowed return envelope does not match its normalized payload",
    )
}

enum ScopedEnvelope {
    Owned,
    Direct,
    SharedOption,
    SharedResult,
    ExclusiveOption,
    ExclusiveResult,
    Nested(Vec<String>),
}

fn admitted_scoped_envelope(ty: &Type) -> syn::Result<ScopedEnvelope> {
    if matches!(ty, Type::Reference(_)) {
        return Ok(ScopedEnvelope::Direct);
    }

    if type_ident(ty).is_some_and(|ident| ident == "Option") {
        let args = type_generic_args(ty);
        let [payload] = args.as_slice() else {
            return Ok(ScopedEnvelope::Owned);
        };
        if let Type::Reference(reference) = payload {
            if reference.mutability.is_some() {
                return Ok(ScopedEnvelope::ExclusiveOption);
            }
            if is_borrowed_collection(&reference.elem) {
                return Ok(ScopedEnvelope::Nested(vec![
                    "Option::Some".to_owned(),
                    "borrowed collection".to_owned(),
                ]));
            }
            return Ok(ScopedEnvelope::SharedOption);
        }
    }

    if type_ident(ty).is_some_and(|ident| ident == "Result") {
        let args = type_generic_args(ty);
        if let [Type::Reference(reference), error] = args.as_slice() {
            if reference.mutability.is_some() {
                return Ok(ScopedEnvelope::ExclusiveResult);
            }
            if is_borrowed_collection(&reference.elem) {
                return Ok(ScopedEnvelope::Nested(vec![
                    "Result::Ok".to_owned(),
                    "borrowed collection".to_owned(),
                ]));
            }
            if let Some(path) = first_borrow_path(error, vec!["Result::Err".to_owned()]) {
                return Ok(ScopedEnvelope::Nested(path));
            }
            return Ok(ScopedEnvelope::SharedResult);
        }
    }

    Ok(first_borrow_path(ty, Vec::new())
        .map(ScopedEnvelope::Nested)
        .unwrap_or(ScopedEnvelope::Owned))
}

fn first_borrow_path(ty: &Type, mut path: Vec<String>) -> Option<Vec<String>> {
    match ty {
        Type::Reference(reference) => {
            path.push(if reference.mutability.is_some() {
                "mutable borrow".to_owned()
            } else {
                "shared borrow".to_owned()
            });
            Some(path)
        }
        Type::Array(array) => {
            path.push("Array::element".to_owned());
            first_borrow_path(&array.elem, path)
        }
        Type::Tuple(tuple) => tuple.elems.iter().enumerate().find_map(|(index, element)| {
            let mut element_path = path.clone();
            element_path.push(format!("Tuple::{index}"));
            first_borrow_path(element, element_path)
        }),
        Type::Path(_) => {
            let ident = type_ident(ty)?;
            let args = type_generic_args(ty);
            match ident.as_str() {
                "Option" => args.first().and_then(|payload| {
                    path.push("Option::Some".to_owned());
                    first_borrow_path(payload, path)
                }),
                "Result" => {
                    let success = args.first().and_then(|success| {
                        let mut success_path = path.clone();
                        success_path.push("Result::Ok".to_owned());
                        first_borrow_path(success, success_path)
                    });
                    success.or_else(|| {
                        args.get(1).and_then(|error| {
                            path.push("Result::Err".to_owned());
                            first_borrow_path(error, path)
                        })
                    })
                }
                "Vec" | "BTreeSet" | "HashSet" => args.first().and_then(|element| {
                    path.push(if ident == "Vec" {
                        "Array::element".to_owned()
                    } else {
                        "Set::element".to_owned()
                    });
                    first_borrow_path(element, path)
                }),
                "BTreeMap" | "HashMap" => {
                    let key = args.first().and_then(|key| {
                        let mut key_path = path.clone();
                        key_path.push("Map::key".to_owned());
                        first_borrow_path(key, key_path)
                    });
                    key.or_else(|| {
                        args.get(1).and_then(|value| {
                            path.push("Map::value".to_owned());
                            first_borrow_path(value, path)
                        })
                    })
                }
                _ => args.iter().enumerate().find_map(|(index, argument)| {
                    let mut argument_path = path.clone();
                    argument_path.push(format!("{ident}::argument[{index}]"));
                    first_borrow_path(argument, argument_path)
                }),
            }
        }
        _ => None,
    }
}

fn is_borrowed_collection(ty: &Type) -> bool {
    matches!(ty, Type::Array(_) | Type::Slice(_))
        || type_ident(ty).is_some_and(|ident| {
            matches!(
                ident.as_str(),
                "Vec" | "BTreeMap" | "HashMap" | "BTreeSet" | "HashSet"
            )
        })
}

fn unsupported_envelope(ty: &Type, family: &str, path: &str) -> syn::Error {
    syn::Error::new_spanned(
        ty,
        format!(
            "service return uses an unsupported {family} call-scoped host envelope\nreturn path: {path}",
        ),
    )
}
