use std::collections::BTreeMap;

use vela_host::adapter::ScriptStateAdapter;
use vela_host::path::{HostPath, HostRef};
use vela_host::tx::PatchTx;
use vela_host::value::HostValue;

use crate::{
    candidates, descriptor_targets,
    error::{ReflectError, ReflectErrorKind, ReflectResult},
    metadata_records,
    permissions::ReflectPolicy,
    registry::{FieldDesc, TypeDesc, TypeKey, TypeRegistry},
    value_access::{
        FieldCandidateAccess, get_record_field, record_unknown_field,
        schema_unknown_field_with_policy, schema_unknown_method_with_policy, script_enum_field,
        script_enum_unknown_field, script_enum_unknown_field_with_policy, script_record_field,
        script_record_unknown_field, script_record_unknown_field_with_policy, set_record_field,
    },
};

#[derive(Clone, Debug, PartialEq)]
pub enum ReflectValue {
    Host(HostValue),
    HostRef(HostRef),
    Closure,
    Range,
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

pub struct ReflectContext<'a> {
    pub registry: &'a TypeRegistry,
    pub adapter: &'a mut dyn ScriptStateAdapter,
    pub tx: &'a mut PatchTx,
}

pub fn type_of<'a>(registry: &'a TypeRegistry, value: &ReflectValue) -> Option<&'a TypeDesc> {
    match value {
        ReflectValue::HostRef(host_ref) => registry.type_of_host(*host_ref),
        ReflectValue::Closure => registry.type_by_name("closure"),
        ReflectValue::Range => registry.type_by_name("range"),
        ReflectValue::ScriptRecord { type_name, .. } => registry.type_by_name(type_name),
        ReflectValue::ScriptEnum { enum_name, .. } => registry.type_by_name(enum_name),
        ReflectValue::Host(value) => type_of_host_value(registry, value),
        // Generic records are the reflect-layer representation for script maps.
        ReflectValue::Record(_) => registry.type_by_name("map"),
        ReflectValue::Set(_) => registry.type_by_name("set"),
    }
}

fn type_of_host_value<'a>(registry: &'a TypeRegistry, value: &HostValue) -> Option<&'a TypeDesc> {
    match value {
        HostValue::Null => registry.type_by_name("null"),
        HostValue::Bool(_) => registry.type_by_name("bool"),
        HostValue::Int(_) => registry.type_by_name("int"),
        HostValue::Float(_) => registry.type_by_name("float"),
        HostValue::String(_) => registry.type_by_name("string"),
        HostValue::Array(_) => registry.type_by_name("array"),
        HostValue::Map(_) => registry.type_by_name("map"),
        HostValue::Record { type_name, .. } => registry.type_by_name(type_name),
        HostValue::Enum { enum_name, .. } => registry.type_by_name(enum_name),
        HostValue::HostRef(host_ref) => registry.type_of_host(*host_ref),
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
        ReflectValue::HostRef(host_ref) => {
            let (field_desc, type_name) = if let Some(policy) = policy {
                let desc = host_type(ctx.registry, *host_ref)?;
                (
                    find_field_with_policy(desc, field, policy, FieldCandidateAccess::Read)?,
                    desc.key.name.clone(),
                )
            } else {
                let field_desc = ctx.registry.host_field(*host_ref, field)?;
                let type_name = ctx
                    .registry
                    .type_of_host(*host_ref)
                    .map_or_else(|| "<unknown>".to_owned(), |desc| desc.key.name.clone());
                (field_desc, type_name)
            };
            if let Some(policy) = policy {
                policy.require_field_read_access(&type_name, field_desc)?;
            } else if !field_desc.access.reflect_readable {
                return Err(ReflectError::new(
                    ReflectErrorKind::FieldNotReflectReadable {
                        type_name,
                        field: field.to_owned(),
                        source_span: field_desc.source_span,
                    },
                ));
            }
            let value = ctx
                .tx
                .read_path(ctx.adapter, &HostPath::new(*host_ref).field(field_desc.id))
                .map_err(|error| ReflectError::new(ReflectErrorKind::Host(error.to_string())))?;
            Ok(ReflectValue::Host(value))
        }
        ReflectValue::Record(record) => {
            get_record_field(field, record, || record_unknown_field(field, record))
        }
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
        ReflectValue::Set(_) => Err(ReflectError::new(ReflectErrorKind::InvalidTarget)),
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
        ReflectValue::HostRef(host_ref) => {
            let (field_desc, type_name) = if let Some(policy) = policy {
                let desc = host_type(ctx.registry, *host_ref)?;
                (
                    find_field_with_policy(desc, field, policy, FieldCandidateAccess::HostWrite)?,
                    desc.key.name.clone(),
                )
            } else {
                let field_desc = ctx.registry.host_field(*host_ref, field)?;
                let type_name = ctx
                    .registry
                    .type_of_host(*host_ref)
                    .map_or_else(|| "<unknown>".to_owned(), |desc| desc.key.name.clone());
                (field_desc, type_name)
            };
            if !field_desc.writable {
                return Err(ReflectError::new(ReflectErrorKind::FieldNotWritable {
                    type_name,
                    field: field.to_owned(),
                    source_span: field_desc.source_span,
                }));
            }
            if let Some(policy) = policy {
                policy.require_field_write_access(&type_name, field_desc)?;
            } else if !field_desc.access.reflect_writable {
                return Err(ReflectError::new(
                    ReflectErrorKind::FieldNotReflectWritable {
                        type_name,
                        field: field.to_owned(),
                        source_span: field_desc.source_span,
                    },
                ));
            }
            let ReflectValue::Host(value) = value else {
                return Err(ReflectError::new(ReflectErrorKind::InvalidValue));
            };
            ctx.tx
                .set_path(
                    ctx.adapter,
                    HostPath::new(*host_ref).field(field_desc.id),
                    value,
                    None,
                )
                .map_err(|error| ReflectError::new(ReflectErrorKind::Host(error.to_string())))?;
            Ok(ReflectValue::Host(HostValue::Null))
        }
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
        ReflectValue::Set(_) => Err(ReflectError::new(ReflectErrorKind::InvalidTarget)),
    }
}

pub fn call(
    ctx: &mut ReflectContext<'_>,
    target: &ReflectValue,
    method: &str,
    args: Vec<ReflectValue>,
) -> ReflectResult<ReflectValue> {
    call_impl(ctx, target, method, args, None)
}

pub fn call_with_policy(
    ctx: &mut ReflectContext<'_>,
    target: &ReflectValue,
    method: &str,
    args: Vec<ReflectValue>,
    policy: &ReflectPolicy,
) -> ReflectResult<ReflectValue> {
    call_impl(ctx, target, method, args, Some(policy))
}

fn call_impl(
    ctx: &mut ReflectContext<'_>,
    target: &ReflectValue,
    method: &str,
    args: Vec<ReflectValue>,
    policy: Option<&ReflectPolicy>,
) -> ReflectResult<ReflectValue> {
    let ReflectValue::HostRef(host_ref) = target else {
        return Err(ReflectError::new(ReflectErrorKind::InvalidTarget));
    };
    let type_name = ctx
        .registry
        .type_of_host(*host_ref)
        .map_or_else(|| "<unknown>".to_owned(), |desc| desc.key.name.clone());
    let method_desc = if let Some(policy) = policy {
        find_method_with_policy(host_type(ctx.registry, *host_ref)?, method, policy)?
    } else {
        ctx.registry.host_method(*host_ref, method)?
    };
    if let Some(policy) = policy {
        policy.require_method_call_access(&type_name, method_desc)?;
    }
    let args = args
        .into_iter()
        .map(host_arg)
        .collect::<ReflectResult<Vec<_>>>()?;
    let result = ctx
        .tx
        .call_method(
            ctx.adapter,
            HostPath::new(*host_ref),
            method_desc.id,
            args,
            None,
        )
        .map_err(|error| ReflectError::new(ReflectErrorKind::Host(error.to_string())))?;
    Ok(ReflectValue::Host(result))
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
        ReflectValue::HostRef(host_ref) => {
            let desc = registry.type_of_host(*host_ref).ok_or_else(|| {
                ReflectError::new(ReflectErrorKind::UnknownType {
                    host_type_id: host_ref.type_id,
                })
            })?;
            Ok(type_implements(desc, trait_name))
        }
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
        | ReflectValue::Record(_)
        | ReflectValue::Set(_) => Err(ReflectError::new(ReflectErrorKind::InvalidTarget)),
    }
}

fn host_type(registry: &TypeRegistry, host_ref: HostRef) -> ReflectResult<&TypeDesc> {
    registry.type_of_host(host_ref).ok_or_else(|| {
        ReflectError::new(ReflectErrorKind::UnknownType {
            host_type_id: host_ref.type_id,
        })
    })
}

fn find_field_with_policy<'a>(
    desc: &'a TypeDesc,
    field: &str,
    policy: &ReflectPolicy,
    access: FieldCandidateAccess,
) -> ReflectResult<&'a FieldDesc> {
    desc.fields
        .iter()
        .find(|candidate| candidate.name == field)
        .ok_or_else(|| {
            ReflectError::new(schema_unknown_field_with_policy(
                &desc.key.name,
                field,
                &desc.fields,
                policy,
                access,
            ))
        })
}

fn find_method_with_policy<'a>(
    desc: &'a TypeDesc,
    method: &str,
    policy: &ReflectPolicy,
) -> ReflectResult<&'a crate::registry::MethodDesc> {
    desc.methods
        .iter()
        .find(|candidate| candidate.name == method)
        .ok_or_else(|| {
            ReflectError::new(schema_unknown_method_with_policy(
                &desc.key.name,
                method,
                &desc.methods,
                policy,
            ))
        })
}

fn type_implements(desc: &TypeDesc, trait_name: &str) -> bool {
    desc.traits
        .iter()
        .any(|trait_desc| trait_desc.name == trait_name)
}

fn host_arg(value: ReflectValue) -> ReflectResult<HostValue> {
    let ReflectValue::Host(value) = value else {
        return Err(ReflectError::new(ReflectErrorKind::InvalidValue));
    };
    Ok(value)
}
