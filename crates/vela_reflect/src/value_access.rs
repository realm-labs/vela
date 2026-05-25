use std::collections::BTreeMap;

use crate::candidates::{candidate_names, name_candidates, ranked_candidates};
use crate::{ReflectError, ReflectErrorKind, ReflectResult, ReflectValue, TypeRegistry};

pub(crate) fn get_record_field(
    field: &str,
    record: &BTreeMap<String, ReflectValue>,
    unknown: impl FnOnce() -> ReflectErrorKind,
) -> ReflectResult<ReflectValue> {
    record
        .get(field)
        .cloned()
        .ok_or_else(|| ReflectError::new(unknown()))
}

pub(crate) fn set_record_field(
    field: &str,
    record: &BTreeMap<String, ReflectValue>,
    value: ReflectValue,
    unknown: impl FnOnce() -> ReflectErrorKind,
) -> ReflectResult<BTreeMap<String, ReflectValue>> {
    if !record.contains_key(field) {
        return Err(ReflectError::new(unknown()));
    }
    let mut record = record.clone();
    record.insert(field.to_owned(), value);
    Ok(record)
}

pub(crate) fn record_unknown_field(
    field: &str,
    record: &BTreeMap<String, ReflectValue>,
) -> ReflectErrorKind {
    record_unknown_field_with_type("record", field, record)
}

pub(crate) fn script_record_unknown_field(
    registry: &TypeRegistry,
    type_name: &str,
    field: &str,
    record: &BTreeMap<String, ReflectValue>,
) -> ReflectErrorKind {
    if let Some(desc) = registry.type_by_name(type_name) {
        let related = ranked_candidates(
            field,
            desc.fields
                .iter()
                .map(|field| (field.name.as_str(), field.source_span)),
        );
        return ReflectErrorKind::UnknownField {
            type_name: type_name.to_owned(),
            field: field.to_owned(),
            candidates: candidate_names(&related),
            related,
        };
    }
    record_unknown_field_with_type(type_name, field, record)
}

pub(crate) fn script_enum_unknown_field(
    registry: &TypeRegistry,
    enum_name: &str,
    variant: &str,
    field: &str,
    record: &BTreeMap<String, ReflectValue>,
) -> ReflectErrorKind {
    if let Some(desc) = registry.type_by_name(enum_name)
        && let Some(variant_desc) = desc
            .variants
            .iter()
            .find(|candidate| candidate.name == variant)
    {
        let related = ranked_candidates(
            field,
            variant_desc
                .fields
                .iter()
                .map(|field| (field.name.as_str(), field.source_span)),
        );
        return ReflectErrorKind::UnknownField {
            type_name: format!("{enum_name}.{variant}"),
            field: field.to_owned(),
            candidates: candidate_names(&related),
            related,
        };
    }
    record_unknown_field_with_type(&format!("{enum_name}.{variant}"), field, record)
}

fn record_unknown_field_with_type(
    type_name: &str,
    field: &str,
    record: &BTreeMap<String, ReflectValue>,
) -> ReflectErrorKind {
    ReflectErrorKind::UnknownField {
        type_name: type_name.to_owned(),
        field: field.to_owned(),
        candidates: name_candidates(field, record.keys().map(String::as_str)),
        related: Vec::new(),
    }
}
