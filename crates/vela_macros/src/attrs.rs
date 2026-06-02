use std::collections::BTreeSet;

use proc_macro2::Span;
use quote::ToTokens;
use syn::{Attribute, LitStr, Meta, Result, Type, spanned::Spanned};

#[derive(Clone, Debug, Default)]
pub(crate) struct ScriptAttrs {
    pub(crate) has_script_attr: bool,
    pub(crate) skip: bool,
    pub(crate) name: Option<String>,
    pub(crate) path: Option<String>,
    pub(crate) alias: Option<String>,
    pub(crate) module: Option<String>,
    pub(crate) docs: Option<String>,
    pub(crate) attrs: Vec<(String, String)>,
    pub(crate) traits: Vec<String>,
    pub(crate) get: bool,
    pub(crate) set: bool,
    pub(crate) type_hint: Option<String>,
    pub(crate) permissions: Vec<String>,
}

impl ScriptAttrs {
    pub(crate) fn field_name(&self, rust_name: &str) -> String {
        self.name.clone().unwrap_or_else(|| rust_name.to_owned())
    }
}

pub(crate) fn parse_script_attrs(attrs: &[Attribute]) -> Result<ScriptAttrs> {
    let mut parsed = ScriptAttrs::default();
    let mut doc_lines = Vec::new();

    for attr in attrs {
        if attr.path().is_ident("doc") {
            if let Some(doc) = parse_doc_attr(attr)? {
                doc_lines.push(doc);
            }
            continue;
        }

        if !attr.path().is_ident("script") {
            continue;
        }

        parsed.has_script_attr = true;
        attr.parse_nested_meta(|meta| {
            if path_name(&meta.path, "skip") {
                parsed.skip = true;
                return Ok(());
            }
            if path_name(&meta.path, "get") {
                parsed.get = true;
                return Ok(());
            }
            if path_name(&meta.path, "set") {
                parsed.set = true;
                return Ok(());
            }

            let value = meta.value()?;
            if path_name(&meta.path, "name") {
                parsed.name = Some(value.parse::<LitStr>()?.value());
            } else if path_name(&meta.path, "path") {
                parsed.path = Some(parse_qualified_name(
                    value.parse::<LitStr>()?,
                    "script path",
                )?);
            } else if path_name(&meta.path, "alias") {
                parsed.alias = Some(parse_qualified_name(
                    value.parse::<LitStr>()?,
                    "script alias",
                )?);
            } else if path_name(&meta.path, "module") {
                parsed.module = Some(parse_qualified_name(
                    value.parse::<LitStr>()?,
                    "script module",
                )?);
            } else if path_name(&meta.path, "docs") {
                parsed.docs = Some(value.parse::<LitStr>()?.value());
            } else if path_name(&meta.path, "attr") {
                parsed
                    .attrs
                    .push(parse_key_value_attr(value.parse::<LitStr>()?, "script")?);
            } else if path_name(&meta.path, "implements") {
                parsed.traits.push(parse_qualified_name(
                    value.parse::<LitStr>()?,
                    "script implemented trait",
                )?);
            } else if path_name(&meta.path, "hint") || path_name(&meta.path, "type") {
                parsed.type_hint = Some(parse_type_hint(value.parse::<LitStr>()?, "script")?);
            } else if path_name(&meta.path, "permission") {
                parsed
                    .permissions
                    .push(parse_permission(value.parse::<LitStr>()?, "script")?);
            } else {
                return Err(meta.error("unsupported script attribute"));
            }
            Ok(())
        })?;
    }

    parsed.permissions.sort();
    parsed.permissions.dedup();
    parsed.traits.sort();
    parsed.traits.dedup();
    reject_duplicate_attr_keys(&parsed.attrs, "script")?;
    if parsed.docs.is_none() && !doc_lines.is_empty() {
        parsed.docs = Some(doc_lines.join("\n"));
    }

    Ok(parsed)
}

pub(crate) fn parse_permission(literal: LitStr, context: &str) -> Result<String> {
    let permission = literal.value();
    if permission.is_empty() {
        return Err(error(
            literal.span(),
            &format!("{context} permission cannot be empty"),
        ));
    }
    Ok(permission)
}

pub(crate) fn parse_qualified_name(literal: LitStr, context: &str) -> Result<String> {
    let name = literal.value();
    if !is_valid_qualified_name(&name) {
        return Err(error(
            literal.span(),
            &format!("{context} must be a non-empty `::` qualified name"),
        ));
    }
    Ok(name)
}

pub(crate) fn parse_key_value_attr(literal: LitStr, context: &str) -> Result<(String, String)> {
    let raw = literal.value();
    let Some((name, value)) = raw.split_once('=') else {
        return Err(error(
            literal.span(),
            &format!("{context} attr metadata must use `key=value`"),
        ));
    };
    let name = name.trim();
    if name.is_empty() {
        return Err(error(
            literal.span(),
            &format!("{context} attr metadata key cannot be empty"),
        ));
    }
    Ok((name.to_owned(), value.trim().to_owned()))
}

pub(crate) fn reject_duplicate_attr_keys(attrs: &[(String, String)], context: &str) -> Result<()> {
    let mut seen = BTreeSet::new();
    for (name, _) in attrs {
        if !seen.insert(name.as_str()) {
            return Err(error(
                Span::call_site(),
                &format!("{context} attr metadata key `{name}` is duplicated"),
            ));
        }
    }
    Ok(())
}

pub(crate) fn parse_type_hint(literal: LitStr, context: &str) -> Result<String> {
    let hint = literal.value();
    if hint.is_empty()
        || hint.trim() != hint
        || hint.contains('<')
        || hint.contains('>')
        || !is_valid_qualified_name(&hint)
    {
        return Err(error(
            literal.span(),
            &format!("{context} type hint must be a non-generic `::` qualified name"),
        ));
    }
    Ok(hint)
}

fn is_valid_qualified_name(name: &str) -> bool {
    !name.is_empty() && !name.contains('.') && name.split("::").all(|segment| !segment.is_empty())
}

fn parse_doc_attr(attr: &Attribute) -> Result<Option<String>> {
    match &attr.meta {
        Meta::NameValue(name_value) => {
            let syn::Expr::Lit(expr_lit) = &name_value.value else {
                return Ok(None);
            };
            let syn::Lit::Str(doc) = &expr_lit.lit else {
                return Ok(None);
            };
            Ok(Some(doc.value().trim().to_owned()))
        }
        Meta::Path(_) | Meta::List(_) => Ok(None),
    }
}

fn path_name(path: &syn::Path, expected: &str) -> bool {
    path.is_ident(expected) || path.to_token_stream().to_string() == expected
}

pub(crate) fn inferred_type_hint(ty: &Type) -> Option<String> {
    if matches!(ty, Type::Array(_)) {
        return Some("array".to_owned());
    }
    let Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    let ident = segment.ident.to_string();
    Some(match ident.as_str() {
        "bool" => "bool".to_owned(),
        "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16" | "u32" | "u64" | "u128"
        | "usize" => "int".to_owned(),
        "f32" | "f64" => "float".to_owned(),
        "String" => "string".to_owned(),
        "Vec" => "array".to_owned(),
        "BTreeMap" | "HashMap" => "map".to_owned(),
        "BTreeSet" | "HashSet" => "set".to_owned(),
        _ => ident,
    })
}

pub(crate) fn error(span: Span, message: &str) -> syn::Error {
    syn::Error::new(span, message)
}

pub(crate) fn spanned_error<T: Spanned>(target: &T, message: &str) -> syn::Error {
    syn::Error::new(target.span(), message)
}
