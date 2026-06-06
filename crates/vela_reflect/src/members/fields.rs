use crate::{
    candidates::{candidate_names, ranked_candidates},
    error::{ReflectError, ReflectErrorKind, ReflectResult},
    member_records::field_record_with_owner,
    permissions::ReflectPolicy,
    registry::{FieldDesc, TypeDesc, TypeRegistry, VariantDesc},
    value::ReflectValue,
};

use super::{target_type, variants::find_variant};

pub fn field(
    registry: &TypeRegistry,
    target: &ReflectValue,
    name: &str,
) -> ReflectResult<ReflectValue> {
    let desc = target_type(registry, target)?;
    if let Some(variant) = active_variant_desc(desc, target)? {
        let owner = variant_owner_name(desc, variant);
        let field = find_variant_field(desc, variant, name)?;
        return Ok(field_record_with_owner(&owner, field));
    }
    let field = find_field(desc, name)?;
    Ok(field_record_with_owner(&desc.key.name, field))
}

pub fn field_with_policy(
    registry: &TypeRegistry,
    target: &ReflectValue,
    name: &str,
    policy: &ReflectPolicy,
) -> ReflectResult<ReflectValue> {
    let desc = target_type(registry, target)?;
    if let Some(variant) = active_variant_desc(desc, target)? {
        let owner = variant_owner_name(desc, variant);
        let field = find_variant_field_with_policy(desc, variant, name, policy)?;
        policy.require_field_read_access(&owner, field)?;
        return Ok(field_record_with_owner(&owner, field));
    }
    let field = find_field_with_policy(desc, name, policy)?;
    policy.require_field_read_access(&desc.key.name, field)?;
    Ok(field_record_with_owner(&desc.key.name, field))
}

pub fn fields_with_policy(
    registry: &TypeRegistry,
    target: &ReflectValue,
    policy: &ReflectPolicy,
) -> ReflectResult<ReflectValue> {
    let desc = target_type(registry, target)?;
    if let Some(variant) = active_variant_desc(desc, target)? {
        let owner = variant_owner_name(desc, variant);
        return Ok(ReflectValue::Array(
            variant
                .fields
                .iter()
                .filter(|field| policy.require_field_read_access(&owner, field).is_ok())
                .map(|field| field_record_with_owner(&owner, field))
                .collect(),
        ));
    }
    Ok(ReflectValue::Array(
        desc.fields
            .iter()
            .filter(|field| {
                policy
                    .require_field_read_access(&desc.key.name, field)
                    .is_ok()
            })
            .map(|field| field_record_with_owner(&desc.key.name, field))
            .collect(),
    ))
}

pub fn all_fields(registry: &TypeRegistry) -> ReflectValue {
    ReflectValue::Array(registry.types().flat_map(field_records_for_type).collect())
}

pub fn all_fields_with_policy(registry: &TypeRegistry, policy: &ReflectPolicy) -> ReflectValue {
    ReflectValue::Array(
        registry
            .types()
            .flat_map(|desc| field_records_for_type_with_policy(desc, policy))
            .collect(),
    )
}

pub fn has_field(
    registry: &TypeRegistry,
    target: &ReflectValue,
    name: &str,
) -> ReflectResult<bool> {
    let desc = target_type(registry, target)?;
    if let Some(variant) = active_variant_desc(desc, target)? {
        return Ok(variant.fields.iter().any(|field| field.name == name));
    }
    Ok(desc.fields.iter().any(|field| field.name == name))
}

pub fn has_field_with_policy(
    registry: &TypeRegistry,
    target: &ReflectValue,
    name: &str,
    policy: &ReflectPolicy,
) -> ReflectResult<bool> {
    let desc = target_type(registry, target)?;
    if let Some(variant) = active_variant_desc(desc, target)? {
        let owner = variant_owner_name(desc, variant);
        return Ok(variant.fields.iter().any(|field| {
            field.name == name && policy.require_field_read_access(&owner, field).is_ok()
        }));
    }
    Ok(desc.fields.iter().any(|field| {
        field.name == name
            && policy
                .require_field_read_access(&desc.key.name, field)
                .is_ok()
    }))
}

fn find_field<'a>(desc: &'a TypeDesc, field: &str) -> ReflectResult<&'a FieldDesc> {
    desc.fields
        .iter()
        .find(|candidate| candidate.name == field)
        .ok_or_else(|| {
            let related = field_candidates(desc, field);
            ReflectError::new(ReflectErrorKind::UnknownField {
                type_name: desc.key.name.clone(),
                field: field.to_owned(),
                candidates: candidate_names(&related),
                related,
            })
        })
}

fn field_records_for_type(desc: &TypeDesc) -> Vec<ReflectValue> {
    let mut fields = desc
        .fields
        .iter()
        .map(|field| field_record_with_owner(&desc.key.name, field))
        .collect::<Vec<_>>();
    fields.extend(desc.variants.iter().flat_map(|variant| {
        let owner = variant_owner_name(desc, variant);
        variant
            .fields
            .iter()
            .map(move |field| field_record_with_owner(&owner, field))
    }));
    fields
}

fn field_records_for_type_with_policy(
    desc: &TypeDesc,
    policy: &ReflectPolicy,
) -> Vec<ReflectValue> {
    let mut fields = desc
        .fields
        .iter()
        .filter(|field| {
            policy
                .require_field_read_access(&desc.key.name, field)
                .is_ok()
        })
        .map(|field| field_record_with_owner(&desc.key.name, field))
        .collect::<Vec<_>>();
    fields.extend(desc.variants.iter().flat_map(|variant| {
        let owner = variant_owner_name(desc, variant);
        variant
            .fields
            .iter()
            .filter(|field| policy.require_field_read_access(&owner, field).is_ok())
            .map(|field| field_record_with_owner(&owner, field))
            .collect::<Vec<_>>()
    }));
    fields
}

fn find_field_with_policy<'a>(
    desc: &'a TypeDesc,
    field: &str,
    policy: &ReflectPolicy,
) -> ReflectResult<&'a FieldDesc> {
    desc.fields
        .iter()
        .find(|candidate| candidate.name == field)
        .ok_or_else(|| {
            let related = field_candidates_with_policy(desc, field, policy);
            ReflectError::new(ReflectErrorKind::UnknownField {
                type_name: desc.key.name.clone(),
                field: field.to_owned(),
                candidates: candidate_names(&related),
                related,
            })
        })
}

fn active_variant_desc<'a>(
    desc: &'a TypeDesc,
    target: &ReflectValue,
) -> ReflectResult<Option<&'a VariantDesc>> {
    match target {
        ReflectValue::ScriptEnum { variant, .. } => find_variant(desc, variant).map(Some),
        _ => Ok(None),
    }
}

fn variant_owner_name(desc: &TypeDesc, variant: &VariantDesc) -> String {
    format!("{}::{}", desc.key.name, variant.name)
}

fn find_variant_field<'a>(
    desc: &TypeDesc,
    variant: &'a VariantDesc,
    field: &str,
) -> ReflectResult<&'a FieldDesc> {
    variant
        .fields
        .iter()
        .find(|candidate| candidate.name == field)
        .ok_or_else(|| {
            let related = variant_field_candidates(variant, field);
            ReflectError::new(ReflectErrorKind::UnknownField {
                type_name: variant_owner_name(desc, variant),
                field: field.to_owned(),
                candidates: candidate_names(&related),
                related,
            })
        })
}

fn find_variant_field_with_policy<'a>(
    desc: &TypeDesc,
    variant: &'a VariantDesc,
    field: &str,
    policy: &ReflectPolicy,
) -> ReflectResult<&'a FieldDesc> {
    variant
        .fields
        .iter()
        .find(|candidate| candidate.name == field)
        .ok_or_else(|| {
            let related = variant_field_candidates_with_policy(desc, variant, field, policy);
            ReflectError::new(ReflectErrorKind::UnknownField {
                type_name: variant_owner_name(desc, variant),
                field: field.to_owned(),
                candidates: candidate_names(&related),
                related,
            })
        })
}

fn field_candidates(desc: &TypeDesc, field: &str) -> Vec<crate::candidates::ReflectCandidate> {
    ranked_candidates(
        field,
        desc.fields
            .iter()
            .map(|field| (field.name.as_str(), field.source_span)),
    )
}

fn field_candidates_with_policy(
    desc: &TypeDesc,
    field: &str,
    policy: &ReflectPolicy,
) -> Vec<crate::candidates::ReflectCandidate> {
    ranked_candidates(
        field,
        desc.fields
            .iter()
            .filter(|candidate| {
                policy
                    .require_field_read_access(&desc.key.name, candidate)
                    .is_ok()
            })
            .map(|field| (field.name.as_str(), field.source_span)),
    )
}

fn variant_field_candidates(
    variant: &VariantDesc,
    field: &str,
) -> Vec<crate::candidates::ReflectCandidate> {
    ranked_candidates(
        field,
        variant
            .fields
            .iter()
            .map(|field| (field.name.as_str(), field.source_span)),
    )
}

fn variant_field_candidates_with_policy(
    desc: &TypeDesc,
    variant: &VariantDesc,
    field: &str,
    policy: &ReflectPolicy,
) -> Vec<crate::candidates::ReflectCandidate> {
    let owner = variant_owner_name(desc, variant);
    ranked_candidates(
        field,
        variant
            .fields
            .iter()
            .filter(|candidate| policy.require_field_read_access(&owner, candidate).is_ok())
            .map(|field| (field.name.as_str(), field.source_span)),
    )
}
