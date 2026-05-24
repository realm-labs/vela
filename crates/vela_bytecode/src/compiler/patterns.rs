use vela_syntax::{Pattern, RecordPatternField};

use super::{CompileError, CompileErrorKind, CompileResult};

pub(crate) fn enum_variant_path(path: &[String]) -> Option<(String, String)> {
    let (variant, enum_path) = path.split_last()?;
    if enum_path.is_empty() {
        return None;
    }
    Some((enum_path.join("."), variant.clone()))
}

pub(crate) fn record_pattern_binding(field: &RecordPatternField) -> CompileResult<String> {
    match &field.pattern {
        None => Ok(field.name.clone()),
        Some(Pattern::Binding(name)) => Ok(name.clone()),
        Some(_) => Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
            "record pattern",
        ))),
    }
}

pub(crate) fn tuple_variant_field_name(index: usize) -> String {
    index.to_string()
}

pub(crate) fn pattern_declares_locals(pattern: &Pattern) -> bool {
    match pattern {
        Pattern::Binding(_) => true,
        Pattern::TupleVariant { fields, .. } => fields.iter().any(pattern_declares_locals),
        Pattern::RecordVariant { fields, .. } => fields
            .iter()
            .any(|field| field.pattern.as_ref().is_none_or(pattern_declares_locals)),
        Pattern::Wildcard | Pattern::Literal(_) | Pattern::Path(_) => false,
    }
}
