use std::collections::BTreeMap;

use crate::candidates::{candidate_names, name_candidates, ranked_candidates};
use crate::error::{ReflectError, ReflectErrorKind, ReflectResult};
use crate::permissions::ReflectPolicy;
use crate::registry::{FieldDesc, MethodDesc, TypeRegistry};
use crate::value::ReflectValue;

#[derive(Clone, Copy)]
pub(crate) enum FieldCandidateAccess {
    Read,
    HostWrite,
    ScriptWrite,
}

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

pub(crate) fn script_record_unknown_field_with_policy(
    registry: &TypeRegistry,
    type_name: &str,
    field: &str,
    record: &BTreeMap<String, ReflectValue>,
    policy: &ReflectPolicy,
    access: FieldCandidateAccess,
) -> ReflectErrorKind {
    if let Some(desc) = registry.type_by_name(type_name) {
        return schema_unknown_field_with_policy(type_name, field, &desc.fields, policy, access);
    }
    record_unknown_field_with_type(type_name, field, record)
}

pub(crate) fn script_record_field<'a>(
    registry: &'a TypeRegistry,
    type_name: &str,
    field: &str,
) -> Option<&'a FieldDesc> {
    registry
        .type_by_name(type_name)?
        .fields
        .iter()
        .find(|candidate| candidate.name == field)
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

pub(crate) fn script_enum_unknown_field_with_policy(
    registry: &TypeRegistry,
    enum_name: &str,
    variant: &str,
    field: &str,
    record: &BTreeMap<String, ReflectValue>,
    policy: &ReflectPolicy,
    access: FieldCandidateAccess,
) -> ReflectErrorKind {
    if let Some(desc) = registry.type_by_name(enum_name)
        && let Some(variant_desc) = desc
            .variants
            .iter()
            .find(|candidate| candidate.name == variant)
    {
        return schema_unknown_field_with_policy(
            &format!("{enum_name}.{variant}"),
            field,
            &variant_desc.fields,
            policy,
            access,
        );
    }
    record_unknown_field_with_type(&format!("{enum_name}.{variant}"), field, record)
}

pub(crate) fn schema_unknown_field_with_policy(
    type_name: &str,
    field: &str,
    fields: &[FieldDesc],
    policy: &ReflectPolicy,
    access: FieldCandidateAccess,
) -> ReflectErrorKind {
    let related = ranked_candidates(
        field,
        fields
            .iter()
            .filter(|candidate| field_candidate_allowed(type_name, candidate, policy, access))
            .map(|field| (field.name.as_str(), field.source_span)),
    );
    ReflectErrorKind::UnknownField {
        type_name: type_name.to_owned(),
        field: field.to_owned(),
        candidates: candidate_names(&related),
        related,
    }
}

pub(crate) fn schema_unknown_method_with_policy(
    type_name: &str,
    method: &str,
    methods: &[MethodDesc],
    policy: &ReflectPolicy,
) -> ReflectErrorKind {
    let related = ranked_candidates(
        method,
        methods
            .iter()
            .filter(|candidate| policy.require_method_access(type_name, candidate).is_ok())
            .map(|method| (method.name.as_str(), method.source_span)),
    );
    ReflectErrorKind::UnknownMethod {
        type_name: type_name.to_owned(),
        method: method.to_owned(),
        candidates: candidate_names(&related),
        related,
    }
}

pub(crate) fn script_enum_field<'a>(
    registry: &'a TypeRegistry,
    enum_name: &str,
    variant: &str,
    field: &str,
) -> Option<&'a FieldDesc> {
    registry
        .type_by_name(enum_name)?
        .variants
        .iter()
        .find(|candidate| candidate.name == variant)?
        .fields
        .iter()
        .find(|candidate| candidate.name == field)
}

fn field_candidate_allowed(
    type_name: &str,
    field: &FieldDesc,
    policy: &ReflectPolicy,
    access: FieldCandidateAccess,
) -> bool {
    match access {
        FieldCandidateAccess::Read => policy.require_field_read_access(type_name, field).is_ok(),
        FieldCandidateAccess::HostWrite => {
            field.writable && policy.require_field_write_access(type_name, field).is_ok()
        }
        FieldCandidateAccess::ScriptWrite => {
            policy.require_field_write_access(type_name, field).is_ok()
        }
    }
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
