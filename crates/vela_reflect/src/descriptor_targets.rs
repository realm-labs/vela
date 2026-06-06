use vela_host::value::HostValue;

use crate::{
    candidates::{candidate_names, ranked_candidates},
    error::{ReflectError, ReflectErrorKind, ReflectResult},
    registry::{TypeDesc, TypeRegistry},
    value::ReflectValue,
};

pub(crate) fn type_desc<'a>(
    registry: &'a TypeRegistry,
    target: &ReflectValue,
) -> ReflectResult<Option<&'a TypeDesc>> {
    let Some(name) = type_name(target)? else {
        return Ok(None);
    };
    registry
        .type_by_name(name)
        .ok_or_else(|| unknown_type_name(registry, name))
        .map(Some)
}

pub(crate) fn trait_name(target: &ReflectValue) -> ReflectResult<&str> {
    match target {
        ReflectValue::Host(HostValue::String(name)) => Ok(name),
        ReflectValue::ScriptRecord { type_name, fields } if type_name == "ReflectTrait" => {
            script_record_name(fields)
        }
        _ => Err(ReflectError::new(ReflectErrorKind::InvalidTarget)),
    }
}

fn type_name(target: &ReflectValue) -> ReflectResult<Option<&str>> {
    match target {
        ReflectValue::ScriptRecord { type_name, fields } if type_name == "ReflectType" => {
            match fields.get("name") {
                Some(ReflectValue::Host(HostValue::String(name))) => Ok(Some(name.as_str())),
                _ => Err(ReflectError::new(ReflectErrorKind::InvalidTarget)),
            }
        }
        _ => Ok(None),
    }
}

fn script_record_name(
    fields: &std::collections::BTreeMap<String, ReflectValue>,
) -> ReflectResult<&str> {
    match fields.get("name") {
        Some(ReflectValue::Host(HostValue::String(name))) => Ok(name),
        _ => Err(ReflectError::new(ReflectErrorKind::InvalidTarget)),
    }
}

fn unknown_type_name(registry: &TypeRegistry, type_name: &str) -> ReflectError {
    let related = ranked_candidates(
        type_name,
        registry
            .types()
            .map(|desc| (desc.key.name.as_str(), desc.source_span)),
    );
    ReflectError::new(ReflectErrorKind::UnknownTypeName {
        type_name: type_name.to_owned(),
        candidates: candidate_names(&related),
        related,
    })
}
