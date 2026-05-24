use vela_syntax::{Pattern, RecordPatternField};

pub(crate) fn enum_variant_path(path: &[String]) -> Option<(String, String)> {
    let (variant, enum_path) = path.split_last()?;
    if enum_path.is_empty() {
        return None;
    }
    Some((enum_path.join("."), variant.clone()))
}

pub(crate) fn record_pattern_field_match(field: &RecordPatternField) -> Option<&Pattern> {
    match field.pattern.as_ref() {
        Some(Pattern::Wildcard | Pattern::Binding(_)) | None => None,
        Some(pattern) => Some(pattern),
    }
}

pub(crate) fn record_pattern_field_declares_locals(field: &RecordPatternField) -> bool {
    field.pattern.as_ref().is_none_or(pattern_declares_locals)
}

pub(crate) fn tuple_variant_field_name(index: usize) -> String {
    index.to_string()
}

pub(crate) fn pattern_declares_locals(pattern: &Pattern) -> bool {
    match pattern {
        Pattern::Binding(_) => true,
        Pattern::TupleVariant { fields, .. } => fields.iter().any(pattern_declares_locals),
        Pattern::RecordVariant { fields, .. } => {
            fields.iter().any(record_pattern_field_declares_locals)
        }
        Pattern::Wildcard | Pattern::Literal(_) | Pattern::Path(_) => false,
    }
}
