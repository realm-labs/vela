use vela_host::HostValue;

use crate::{
    ReflectError, ReflectErrorKind, ReflectPolicy, ReflectResult, ReflectValue, TypeDesc,
    TypeRegistry, VariantDesc,
    candidates::{candidate_names, ranked_candidates},
    member_records::{variant_record_with_owner, variant_record_with_owner_and_fields},
    type_of,
};

use super::target_type;

pub fn variants(registry: &TypeRegistry, target: &ReflectValue) -> ReflectResult<ReflectValue> {
    let desc = target_type(registry, target)?;
    Ok(ReflectValue::Host(HostValue::Array(
        desc.variants
            .iter()
            .map(|variant| variant_record_with_owner(&desc.key.name, variant))
            .collect(),
    )))
}

pub fn variants_with_policy(
    registry: &TypeRegistry,
    target: &ReflectValue,
    policy: &ReflectPolicy,
) -> ReflectResult<ReflectValue> {
    let desc = target_type(registry, target)?;
    Ok(ReflectValue::Host(HostValue::Array(
        desc.variants
            .iter()
            .map(|variant| {
                variant_record_with_owner_and_fields(
                    &desc.key.name,
                    variant,
                    variant.fields.iter().filter(|field| {
                        policy
                            .require_field_read_access(&desc.key.name, field)
                            .is_ok()
                    }),
                )
            })
            .collect(),
    )))
}

pub fn variant_info(
    registry: &TypeRegistry,
    target: &ReflectValue,
    name: &str,
) -> ReflectResult<ReflectValue> {
    let desc = target_type(registry, target)?;
    let variant = find_variant(desc, name)?;
    Ok(ReflectValue::Host(variant_record_with_owner(
        &desc.key.name,
        variant,
    )))
}

pub fn variant_info_with_policy(
    registry: &TypeRegistry,
    target: &ReflectValue,
    name: &str,
    policy: &ReflectPolicy,
) -> ReflectResult<ReflectValue> {
    let desc = target_type(registry, target)?;
    let variant = find_variant(desc, name)?;
    Ok(ReflectValue::Host(variant_record_with_owner_and_fields(
        &desc.key.name,
        variant,
        variant.fields.iter().filter(|field| {
            policy
                .require_field_read_access(&desc.key.name, field)
                .is_ok()
        }),
    )))
}

pub fn has_variant(
    registry: &TypeRegistry,
    target: &ReflectValue,
    name: &str,
) -> ReflectResult<bool> {
    let desc = target_type(registry, target)?;
    Ok(desc.variants.iter().any(|variant| variant.name == name))
}

pub fn all_variants(registry: &TypeRegistry) -> ReflectValue {
    ReflectValue::Host(HostValue::Array(
        registry
            .types()
            .flat_map(|desc| {
                desc.variants
                    .iter()
                    .map(|variant| variant_record_with_owner(&desc.key.name, variant))
            })
            .collect(),
    ))
}

pub fn all_variants_with_policy(registry: &TypeRegistry, policy: &ReflectPolicy) -> ReflectValue {
    ReflectValue::Host(HostValue::Array(
        registry
            .types()
            .flat_map(|desc| {
                desc.variants.iter().map(|variant| {
                    variant_record_with_owner_and_fields(
                        &desc.key.name,
                        variant,
                        variant.fields.iter().filter(|field| {
                            policy
                                .require_field_read_access(&desc.key.name, field)
                                .is_ok()
                        }),
                    )
                })
            })
            .collect(),
    ))
}

pub fn variant(target: &ReflectValue) -> ReflectResult<ReflectValue> {
    Ok(ReflectValue::Host(HostValue::String(
        variant_name(target)?.to_owned(),
    )))
}

pub fn variant_is(
    registry: &TypeRegistry,
    target: &ReflectValue,
    name: &str,
) -> ReflectResult<bool> {
    let actual = variant_name(target)?;
    let Some(desc) = type_of(registry, target) else {
        return Ok(actual == name);
    };
    if desc.variants.iter().any(|variant| variant.name == name) {
        return Ok(actual == name);
    }
    let related = variant_candidates(desc, name);
    Err(ReflectError::new(ReflectErrorKind::UnknownVariant {
        type_name: desc.key.name.clone(),
        variant: name.to_owned(),
        candidates: candidate_names(&related),
        related,
    }))
}

pub(super) fn find_variant<'a>(
    desc: &'a TypeDesc,
    variant: &str,
) -> ReflectResult<&'a VariantDesc> {
    desc.variants
        .iter()
        .find(|candidate| candidate.name == variant)
        .ok_or_else(|| {
            let related = variant_candidates(desc, variant);
            ReflectError::new(ReflectErrorKind::UnknownVariant {
                type_name: desc.key.name.clone(),
                variant: variant.to_owned(),
                candidates: candidate_names(&related),
                related,
            })
        })
}

fn variant_name(target: &ReflectValue) -> ReflectResult<&str> {
    match target {
        ReflectValue::ScriptEnum { variant, .. } => Ok(variant),
        ReflectValue::Host(HostValue::Enum { variant, .. }) => Ok(variant),
        ReflectValue::Host(_)
        | ReflectValue::HostRef(_)
        | ReflectValue::Record(_)
        | ReflectValue::ScriptRecord { .. } => {
            Err(ReflectError::new(ReflectErrorKind::InvalidTarget))
        }
    }
}

fn variant_candidates(desc: &TypeDesc, variant: &str) -> Vec<crate::ReflectCandidate> {
    ranked_candidates(
        variant,
        desc.variants
            .iter()
            .map(|variant| (variant.name.as_str(), variant.source_span)),
    )
}
