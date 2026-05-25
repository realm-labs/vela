use syn::{Attribute, Generics, Pat, PatType, Result, Signature, Type, spanned::Spanned};

const UNSUPPORTED_SCRIPT_INTEGER_TYPES: &[&str] = &["i128", "isize", "u64", "u128", "usize"];

pub(crate) fn reject_script_reference_param(param: &PatType) -> Result<()> {
    if matches!(param.ty.as_ref(), Type::Reference(_)) {
        return Err(syn::Error::new(
            param.span(),
            "script-visible parameters cannot use Rust references; pass copied values, HostRef, HostPath, or PathProxy",
        ));
    }
    Ok(())
}

pub(crate) fn reject_generic_signature(generics: &Generics, context: &str) -> Result<()> {
    if generics.params.is_empty() && generics.where_clause.is_none() {
        return Ok(());
    }

    Err(syn::Error::new(
        generics.span(),
        format!("{context} does not support generic parameters or where clauses"),
    ))
}

pub(crate) fn reject_unsafe_signature(signature: &Signature, context: &str) -> Result<()> {
    if signature.unsafety.is_none() {
        return Ok(());
    }

    Err(syn::Error::new(
        signature.unsafety.span(),
        format!("{context} does not support unsafe functions"),
    ))
}

pub(crate) fn reject_extern_signature(signature: &Signature, context: &str) -> Result<()> {
    if signature.abi.is_none() {
        return Ok(());
    }

    Err(syn::Error::new(
        signature.abi.span(),
        format!("{context} does not support extern ABI functions"),
    ))
}

pub(crate) fn reject_unsupported_integer_type(ty: &Type) -> Result<()> {
    if let Some(unsupported) = unsupported_integer_type(ty) {
        return Err(syn::Error::new(
            ty.span(),
            format!(
                "script-visible native signatures do not support Rust integer type `{unsupported}`; use i64, i32, i16, i8, u32, u16, or u8"
            ),
        ));
    }
    Ok(())
}

pub(crate) fn param_name(param: &PatType) -> String {
    match param.pat.as_ref() {
        Pat::Ident(ident) => ident.ident.to_string().trim_start_matches('_').to_owned(),
        _ => "arg".to_owned(),
    }
}

pub(crate) fn type_ident(ty: &Type) -> Option<String> {
    match ty {
        Type::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        Type::Reference(reference) => type_ident(&reference.elem),
        _ => None,
    }
}

fn unsupported_integer_type(ty: &Type) -> Option<String> {
    match ty {
        Type::Path(path) => {
            let segment = path.path.segments.last()?;
            let ident = segment.ident.to_string();
            if UNSUPPORTED_SCRIPT_INTEGER_TYPES.contains(&ident.as_str()) {
                return Some(ident);
            }
            let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
                return None;
            };
            args.args.iter().find_map(|arg| match arg {
                syn::GenericArgument::Type(ty) => unsupported_integer_type(ty),
                _ => None,
            })
        }
        Type::Tuple(tuple) => tuple.elems.iter().find_map(unsupported_integer_type),
        Type::Array(array) => unsupported_integer_type(&array.elem),
        Type::Reference(reference) => unsupported_integer_type(&reference.elem),
        _ => None,
    }
}

pub(crate) fn wrapper_inner_type<'a>(ty: &'a Type, wrapper_names: &[&str]) -> Option<&'a Type> {
    let Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    let ident = segment.ident.to_string();
    if !wrapper_names.iter().any(|wrapper| *wrapper == ident) {
        return None;
    }
    let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    args.args.iter().find_map(|arg| match arg {
        syn::GenericArgument::Type(ty) => Some(ty),
        _ => None,
    })
}

pub(crate) fn docs_from_attrs(attrs: &[Attribute]) -> Option<String> {
    let docs = attrs
        .iter()
        .filter(|attr| attr.path().is_ident("doc"))
        .filter_map(doc_from_attr)
        .collect::<Vec<_>>();
    (!docs.is_empty()).then(|| docs.join("\n"))
}

fn doc_from_attr(attr: &Attribute) -> Option<String> {
    let syn::Meta::NameValue(name_value) = &attr.meta else {
        return None;
    };
    let syn::Expr::Lit(expr_lit) = &name_value.value else {
        return None;
    };
    let syn::Lit::Str(doc) = &expr_lit.lit else {
        return None;
    };
    Some(doc.value().trim().to_owned())
}
