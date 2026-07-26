use syn::{Attribute, Generics, Pat, PatType, Result, Signature, Type, spanned::Spanned};

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

pub(crate) fn type_generic_args(ty: &Type) -> Vec<&Type> {
    let Type::Path(path) = ty else {
        return Vec::new();
    };
    let Some(segment) = path.path.segments.last() else {
        return Vec::new();
    };
    let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
        return Vec::new();
    };
    args.args
        .iter()
        .filter_map(|arg| match arg {
            syn::GenericArgument::Type(ty) => Some(ty),
            _ => None,
        })
        .collect()
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
