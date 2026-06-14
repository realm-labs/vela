use std::collections::BTreeSet;

use proc_macro2::Span;
use quote::ToTokens;
use syn::{Attribute, LitStr, Meta, Result, Type, spanned::Spanned};

use crate::signature::type_generic_args;

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
    if !is_valid_type_hint_contract(&hint) {
        return Err(error(
            literal.span(),
            &format!(
                "{context} type hint must be a non-generic name or supported builtin type-hint contract"
            ),
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
    if let Type::Array(array) = ty {
        return inferred_type_hint(&array.elem)
            .map(|element| format!("Array<{element}>"))
            .or_else(|| Some("Array".to_owned()));
    }
    let Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    let ident = segment.ident.to_string();
    Some(match ident.as_str() {
        "bool" => "bool".to_owned(),
        "char" | "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "f32" | "f64" => {
            ident
        }
        "i128" | "isize" | "u128" | "usize" => return None,
        "String" => "String".to_owned(),
        "Vec" => {
            let args = type_generic_args(ty);
            return args
                .first()
                .and_then(|arg| inferred_type_hint(arg))
                .map(|element| format!("Array<{element}>"))
                .or_else(|| Some("Array".to_owned()));
        }
        "BTreeMap" | "HashMap" => {
            let args = type_generic_args(ty);
            return match args.as_slice() {
                [key, value] => {
                    let key = inferred_type_hint(key)?;
                    let value = inferred_type_hint(value)?;
                    is_keyable_type_hint(&key).then(|| format!("Map<{key}, {value}>"))
                }
                _ => Some("Map".to_owned()),
            };
        }
        "BTreeSet" | "HashSet" => {
            let args = type_generic_args(ty);
            return args
                .first()
                .and_then(|arg| inferred_type_hint(arg))
                .filter(|element| is_keyable_type_hint(element))
                .map(|element| format!("Set<{element}>"))
                .or_else(|| Some("Set".to_owned()));
        }
        "Option" => {
            let args = type_generic_args(ty);
            return args
                .first()
                .and_then(|arg| inferred_type_hint(arg))
                .map(|payload| format!("Option<{payload}>"));
        }
        "Result" => {
            let args = type_generic_args(ty);
            return match args.as_slice() {
                [ok, err] => Some(format!(
                    "Result<{}, {}>",
                    inferred_type_hint(ok)?,
                    inferred_type_hint(err)?
                )),
                _ => None,
            };
        }
        _ => ident,
    })
}

fn is_valid_type_hint_contract(hint: &str) -> bool {
    if hint.is_empty() || hint.trim() != hint {
        return false;
    }
    if !hint.contains('<') && !hint.contains('>') {
        return is_valid_qualified_name(hint);
    }
    TypeHintParser::new(hint).parse_complete()
}

struct TypeHintParser<'a> {
    input: &'a str,
    cursor: usize,
}

impl<'a> TypeHintParser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, cursor: 0 }
    }

    fn parse_complete(mut self) -> bool {
        self.parse_hint_def()
            .is_some_and(|hint| hint.is_valid_contract())
            && {
                self.skip_ws();
                self.cursor == self.input.len()
            }
    }

    fn parse_hint_def(&mut self) -> Option<ParsedTypeHint> {
        self.skip_ws();
        let name = self.parse_name()?;
        self.skip_ws();
        if !self.consume('<') {
            return Some(ParsedTypeHint {
                name,
                args: Vec::new(),
            });
        }
        let mut args = Vec::new();
        loop {
            self.skip_ws();
            args.push(self.parse_hint_def()?);
            self.skip_ws();
            if !self.consume(',') {
                break;
            }
        }
        self.skip_ws();
        if !self.consume('>') {
            return None;
        }
        Some(ParsedTypeHint { name, args })
    }

    fn parse_name(&mut self) -> Option<String> {
        let start = self.cursor;
        while let Some(ch) = self.peek() {
            if ch == '<' || ch == '>' || ch == ',' || ch.is_whitespace() {
                break;
            }
            self.cursor += ch.len_utf8();
        }
        let name = &self.input[start..self.cursor];
        (!name.is_empty() && is_valid_qualified_name(name)).then(|| name.to_owned())
    }

    fn skip_ws(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            let ch = self.peek().expect("peek checked");
            self.cursor += ch.len_utf8();
        }
    }

    fn consume(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.cursor += expected.len_utf8();
            true
        } else {
            false
        }
    }

    fn peek(&self) -> Option<char> {
        self.input[self.cursor..].chars().next()
    }
}

fn is_keyable_type_hint(hint: &str) -> bool {
    let mut parser = TypeHintParser::new(hint);
    let Some(parsed) = parser.parse_hint_def() else {
        return false;
    };
    parser.skip_ws();
    parser.cursor == hint.len() && parsed.is_valid_contract() && parsed.is_keyable()
}

#[derive(Clone, Debug)]
struct ParsedTypeHint {
    name: String,
    args: Vec<ParsedTypeHint>,
}

impl ParsedTypeHint {
    fn is_valid_contract(&self) -> bool {
        match self.name.as_str() {
            "Array" | "Iterator" | "Option" => {
                matches!(self.args.as_slice(), [element] if element.is_valid_contract())
            }
            "Set" => {
                matches!(self.args.as_slice(), [element] if element.is_valid_contract() && element.is_keyable())
            }
            "Result" => {
                matches!(self.args.as_slice(), [ok, err] if ok.is_valid_contract() && err.is_valid_contract())
            }
            "Map" => {
                matches!(self.args.as_slice(), [key, value] if key.is_valid_contract() && key.is_keyable() && value.is_valid_contract())
            }
            _ => self.args.is_empty(),
        }
    }

    fn is_keyable(&self) -> bool {
        !matches!(
            self.name.as_str(),
            "Range" | "Function" | "PathProxy" | "path_proxy"
        )
    }
}

pub(crate) fn error(span: Span, message: &str) -> syn::Error {
    syn::Error::new(span, message)
}

pub(crate) fn spanned_error<T: Spanned>(target: &T, message: &str) -> syn::Error {
    syn::Error::new(target.span(), message)
}
