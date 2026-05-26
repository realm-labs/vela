use std::collections::BTreeMap;

use vela_host::HostValue;

use crate::{ReflectError, ReflectErrorKind, ReflectResult, ReflectValue};

pub(crate) fn name(target: &ReflectValue) -> ReflectResult<Option<HostValue>> {
    let Some(name) = field(target, "name") else {
        return Ok(None);
    };
    match name {
        MetadataField::Host(HostValue::String(_))
        | MetadataField::Reflect(ReflectValue::Host(HostValue::String(_))) => {
            Ok(Some(host_value(name)?))
        }
        _ => Err(invalid_target()),
    }
}

pub(crate) fn attrs(target: &ReflectValue) -> ReflectResult<Option<HostValue>> {
    let Some(attrs) = field(target, "attrs") else {
        return Ok(None);
    };
    attrs_to_host_map(attrs).map(Some)
}

pub(crate) fn attr(target: &ReflectValue, name: &str) -> ReflectResult<Option<HostValue>> {
    let Some(HostValue::Map(attrs)) = attrs(target)? else {
        return Ok(None);
    };
    Ok(Some(attrs.get(name).cloned().unwrap_or(HostValue::Null)))
}

pub(crate) fn has_attr(target: &ReflectValue, name: &str) -> ReflectResult<Option<bool>> {
    let Some(HostValue::Map(attrs)) = attrs(target)? else {
        return Ok(None);
    };
    Ok(Some(attrs.contains_key(name)))
}

pub(crate) fn docs(target: &ReflectValue) -> ReflectResult<Option<HostValue>> {
    let Some(docs) = field(target, "docs") else {
        return Ok(None);
    };
    match docs {
        MetadataField::Host(HostValue::Null | HostValue::String(_))
        | MetadataField::Reflect(ReflectValue::Host(HostValue::Null | HostValue::String(_))) => {
            Ok(Some(host_value(docs)?))
        }
        _ => Err(invalid_target()),
    }
}

enum MetadataField<'a> {
    Host(&'a HostValue),
    Reflect(&'a ReflectValue),
}

fn field<'a>(target: &'a ReflectValue, name: &str) -> Option<MetadataField<'a>> {
    match target {
        ReflectValue::Host(HostValue::Record { type_name, fields })
            if is_reflect_metadata_record(type_name) =>
        {
            fields.get(name).map(MetadataField::Host)
        }
        ReflectValue::ScriptRecord { type_name, fields }
            if is_reflect_metadata_record(type_name) =>
        {
            fields.get(name).map(MetadataField::Reflect)
        }
        _ => None,
    }
}

fn is_reflect_metadata_record(type_name: &str) -> bool {
    type_name.starts_with("Reflect")
}

fn attrs_to_host_map(value: MetadataField<'_>) -> ReflectResult<HostValue> {
    match value {
        MetadataField::Host(HostValue::Map(attrs)) => Ok(HostValue::Map(attrs.clone())),
        MetadataField::Reflect(ReflectValue::Host(HostValue::Map(attrs))) => {
            Ok(HostValue::Map(attrs.clone()))
        }
        MetadataField::Reflect(ReflectValue::Record(attrs)) => attrs
            .iter()
            .map(|(key, value)| {
                let ReflectValue::Host(HostValue::String(value)) = value else {
                    return Err(invalid_target());
                };
                Ok((key.clone(), HostValue::String(value.clone())))
            })
            .collect::<ReflectResult<BTreeMap<_, _>>>()
            .map(HostValue::Map),
        _ => Err(invalid_target()),
    }
}

fn host_value(value: MetadataField<'_>) -> ReflectResult<HostValue> {
    match value {
        MetadataField::Host(value) | MetadataField::Reflect(ReflectValue::Host(value)) => {
            Ok(value.clone())
        }
        _ => Err(invalid_target()),
    }
}

fn invalid_target() -> ReflectError {
    ReflectError::new(ReflectErrorKind::InvalidTarget)
}
