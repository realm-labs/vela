use std::collections::BTreeMap;

use vela_host::value::HostValue;

use crate::{
    candidates, descriptor_targets,
    error::{ReflectError, ReflectErrorKind, ReflectResult},
    metadata_records,
    permissions::ReflectPolicy,
    registry::{FieldDesc, TypeDesc, TypeKey, TypeRegistry},
    value_access::{
        FieldCandidateAccess, get_record_field, record_unknown_field, script_enum_field,
        script_enum_unknown_field, script_enum_unknown_field_with_policy, script_record_field,
        script_record_unknown_field, script_record_unknown_field_with_policy, set_record_field,
    },
};

#[derive(Clone, Debug, PartialEq)]
pub enum ReflectValue {
    Host(HostValue),
    Closure,
    Range,
    Array(Vec<ReflectValue>),
    Map(Vec<ReflectMapEntry>),
    Record(BTreeMap<String, ReflectValue>),
    Set(Vec<ReflectValue>),
    ScriptRecord {
        type_name: String,
        fields: BTreeMap<String, ReflectValue>,
    },
    ScriptEnum {
        enum_name: String,
        variant: String,
        fields: BTreeMap<String, ReflectValue>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReflectMapEntry {
    pub key: ReflectValue,
    pub value: ReflectValue,
}

impl ReflectMapEntry {
    #[must_use]
    pub const fn new(key: ReflectValue, value: ReflectValue) -> Self {
        Self { key, value }
    }
}

impl PartialEq<HostValue> for ReflectValue {
    fn eq(&self, other: &HostValue) -> bool {
        matches!(self, Self::Host(value) if value == other)
    }
}

impl PartialEq<ReflectValue> for HostValue {
    fn eq(&self, other: &ReflectValue) -> bool {
        other == self
    }
}

pub struct ReflectContext<'a> {
    pub registry: &'a TypeRegistry,
}

pub fn type_of<'a>(registry: &'a TypeRegistry, value: &ReflectValue) -> Option<&'a TypeDesc> {
    match value {
        ReflectValue::Closure => registry.type_by_name("closure"),
        ReflectValue::Range => registry.type_by_name("range"),
        ReflectValue::ScriptRecord { type_name, .. } => registry.type_by_name(type_name),
        ReflectValue::ScriptEnum { enum_name, .. } => registry.type_by_name(enum_name),
        ReflectValue::Host(value) => type_of_host_value(registry, value),
        ReflectValue::Array(_) => registry.type_by_name("array"),
        ReflectValue::Map(_) => registry.type_by_name("map"),
        // Generic records remain object-shaped metadata maps.
        ReflectValue::Record(_) => registry.type_by_name("map"),
        ReflectValue::Set(_) => registry.type_by_name("set"),
    }
}

fn type_of_host_value<'a>(registry: &'a TypeRegistry, value: &HostValue) -> Option<&'a TypeDesc> {
    match value {
        HostValue::Unit => registry.type_by_name("()"),
        HostValue::Bool(_) => registry.type_by_name("bool"),
        HostValue::Char(_) => registry.type_by_name("char"),
        HostValue::Scalar(value) => registry.type_by_name(value.type_name()),
        HostValue::String(_) => registry.type_by_name("string"),
        HostValue::Bytes(_) => registry.type_by_name("bytes"),
        HostValue::Detached(value) => match value.as_ref() {
            vela_host::call_value::HostCallValue::Record { type_name, .. } => {
                registry.type_by_name(type_name)
            }
            vela_host::call_value::HostCallValue::Enum { enum_name, .. } => {
                registry.type_by_name(enum_name)
            }
            _ => None,
        },
    }
}

pub fn fields<'a>(registry: &'a TypeRegistry, key: &TypeKey) -> Option<&'a [FieldDesc]> {
    registry.fields(key)
}

pub fn get(
    ctx: &mut ReflectContext<'_>,
    target: &ReflectValue,
    field: &str,
) -> ReflectResult<ReflectValue> {
    get_impl(ctx, target, field, None)
}

pub fn get_with_policy(
    ctx: &mut ReflectContext<'_>,
    target: &ReflectValue,
    field: &str,
    policy: &ReflectPolicy,
) -> ReflectResult<ReflectValue> {
    get_impl(ctx, target, field, Some(policy))
}

fn get_impl(
    ctx: &mut ReflectContext<'_>,
    target: &ReflectValue,
    field: &str,
    policy: Option<&ReflectPolicy>,
) -> ReflectResult<ReflectValue> {
    match target {
        ReflectValue::Record(record) => {
            get_record_field(field, record, || record_unknown_field(field, record))
        }
        ReflectValue::Map(entries) => get_map_string_key(field, entries),
        ReflectValue::ScriptRecord { type_name, fields } => {
            if let Some(policy) = policy
                && let Some(field_desc) = script_record_field(ctx.registry, type_name, field)
            {
                policy.require_field_read_access(type_name, field_desc)?;
            }
            get_record_field(field, fields, || {
                if let Some(policy) = policy {
                    script_record_unknown_field_with_policy(
                        ctx.registry,
                        type_name,
                        field,
                        fields,
                        policy,
                        FieldCandidateAccess::Read,
                    )
                } else {
                    script_record_unknown_field(ctx.registry, type_name, field, fields)
                }
            })
        }
        ReflectValue::ScriptEnum {
            enum_name,
            variant,
            fields,
        } => {
            if let Some(policy) = policy
                && let Some(field_desc) = script_enum_field(ctx.registry, enum_name, variant, field)
            {
                policy.require_field_read_access(&format!("{enum_name}::{variant}"), field_desc)?;
            }
            get_record_field(field, fields, || {
                if let Some(policy) = policy {
                    script_enum_unknown_field_with_policy(
                        ctx.registry,
                        enum_name,
                        variant,
                        field,
                        fields,
                        policy,
                        FieldCandidateAccess::Read,
                    )
                } else {
                    script_enum_unknown_field(ctx.registry, enum_name, variant, field, fields)
                }
            })
        }
        ReflectValue::Host(_) | ReflectValue::Closure | ReflectValue::Range => {
            Err(ReflectError::new(ReflectErrorKind::InvalidTarget))
        }
        ReflectValue::Array(_) | ReflectValue::Set(_) => {
            Err(ReflectError::new(ReflectErrorKind::InvalidTarget))
        }
    }
}

pub fn set(
    ctx: &mut ReflectContext<'_>,
    target: &ReflectValue,
    field: &str,
    value: ReflectValue,
) -> ReflectResult<ReflectValue> {
    set_impl(ctx, target, field, value, None)
}

pub fn set_with_policy(
    ctx: &mut ReflectContext<'_>,
    target: &ReflectValue,
    field: &str,
    value: ReflectValue,
    policy: &ReflectPolicy,
) -> ReflectResult<ReflectValue> {
    set_impl(ctx, target, field, value, Some(policy))
}

fn set_impl(
    ctx: &mut ReflectContext<'_>,
    target: &ReflectValue,
    field: &str,
    value: ReflectValue,
    policy: Option<&ReflectPolicy>,
) -> ReflectResult<ReflectValue> {
    match target {
        ReflectValue::Record(fields) => Ok(ReflectValue::Record(set_record_field(
            field,
            fields,
            value,
            || record_unknown_field(field, fields),
        )?)),
        ReflectValue::ScriptRecord { type_name, fields } => {
            if metadata_records::is_reflect_metadata_record(type_name) {
                return Err(ReflectError::new(ReflectErrorKind::InvalidTarget));
            }
            if let Some(policy) = policy
                && let Some(field_desc) = script_record_field(ctx.registry, type_name, field)
            {
                policy.require_field_write_access(type_name, field_desc)?;
            }
            Ok(ReflectValue::ScriptRecord {
                type_name: type_name.clone(),
                fields: set_record_field(field, fields, value, || {
                    if let Some(policy) = policy {
                        script_record_unknown_field_with_policy(
                            ctx.registry,
                            type_name,
                            field,
                            fields,
                            policy,
                            FieldCandidateAccess::ScriptWrite,
                        )
                    } else {
                        script_record_unknown_field(ctx.registry, type_name, field, fields)
                    }
                })?,
            })
        }
        ReflectValue::ScriptEnum {
            enum_name,
            variant,
            fields,
        } => {
            if let Some(policy) = policy
                && let Some(field_desc) = script_enum_field(ctx.registry, enum_name, variant, field)
            {
                policy
                    .require_field_write_access(&format!("{enum_name}::{variant}"), field_desc)?;
            }
            Ok(ReflectValue::ScriptEnum {
                enum_name: enum_name.clone(),
                variant: variant.clone(),
                fields: set_record_field(field, fields, value, || {
                    if let Some(policy) = policy {
                        script_enum_unknown_field_with_policy(
                            ctx.registry,
                            enum_name,
                            variant,
                            field,
                            fields,
                            policy,
                            FieldCandidateAccess::ScriptWrite,
                        )
                    } else {
                        script_enum_unknown_field(ctx.registry, enum_name, variant, field, fields)
                    }
                })?,
            })
        }
        ReflectValue::Host(_) | ReflectValue::Closure | ReflectValue::Range => {
            Err(ReflectError::new(ReflectErrorKind::InvalidTarget))
        }
        ReflectValue::Array(_) | ReflectValue::Map(_) | ReflectValue::Set(_) => {
            Err(ReflectError::new(ReflectErrorKind::InvalidTarget))
        }
    }
}

pub fn implements(
    registry: &TypeRegistry,
    target: &ReflectValue,
    trait_target: &ReflectValue,
) -> ReflectResult<bool> {
    let trait_name = descriptor_targets::trait_name(trait_target)?;
    let known_traits = registry.known_trait_names();
    if !known_traits.iter().any(|candidate| candidate == trait_name) {
        let candidates = registry.known_trait_candidates();
        let related = candidates::ranked_candidates(
            trait_name,
            candidates
                .iter()
                .map(|(candidate, span)| (candidate.as_str(), *span)),
        );
        return Err(ReflectError::new(ReflectErrorKind::UnknownTrait {
            trait_name: trait_name.to_owned(),
            candidates: candidates::candidate_names(&related),
            related,
        }));
    }

    if let Some(desc) = descriptor_targets::type_desc(registry, target)? {
        return Ok(type_implements(desc, trait_name));
    }

    match target {
        ReflectValue::ScriptRecord { type_name, .. }
        | ReflectValue::ScriptEnum {
            enum_name: type_name,
            ..
        } => {
            let Some(desc) = registry.type_by_name(type_name) else {
                return Ok(false);
            };
            Ok(type_implements(desc, trait_name))
        }
        ReflectValue::Host(_)
        | ReflectValue::Closure
        | ReflectValue::Range
        | ReflectValue::Array(_)
        | ReflectValue::Map(_)
        | ReflectValue::Record(_)
        | ReflectValue::Set(_) => Err(ReflectError::new(ReflectErrorKind::InvalidTarget)),
    }
}

fn type_implements(desc: &TypeDesc, trait_name: &str) -> bool {
    desc.traits
        .iter()
        .any(|trait_desc| trait_desc.name == trait_name)
}

fn get_map_string_key(field: &str, entries: &[ReflectMapEntry]) -> ReflectResult<ReflectValue> {
    let lookup_key = ReflectValue::Host(HostValue::String(field.to_owned()));
    entries
        .iter()
        .find(|entry| entry.key == lookup_key)
        .map(|entry| entry.value.clone())
        .ok_or_else(|| ReflectError::new(map_unknown_string_key(field, entries)))
}

fn map_unknown_string_key(field: &str, entries: &[ReflectMapEntry]) -> ReflectErrorKind {
    let string_entries = entries
        .iter()
        .filter_map(|entry| match &entry.key {
            ReflectValue::Host(HostValue::String(key)) => Some((key.clone(), entry.value.clone())),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    record_unknown_field(field, &string_entries)
}
