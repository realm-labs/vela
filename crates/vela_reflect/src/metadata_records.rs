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

pub(crate) fn id(target: &ReflectValue) -> ReflectResult<Option<HostValue>> {
    let Some(id) = field(target, "id") else {
        return Ok(None);
    };
    match id {
        MetadataField::Host(HostValue::Int(_))
        | MetadataField::Reflect(ReflectValue::Host(HostValue::Int(_))) => {
            Ok(Some(host_value(id)?))
        }
        _ => Err(invalid_target()),
    }
}

pub(crate) fn kind(target: &ReflectValue) -> ReflectResult<Option<HostValue>> {
    let Some(type_name) = record_type_name(target) else {
        return Ok(None);
    };
    if let Some(kind) = field(target, "kind") {
        return match kind {
            MetadataField::Host(HostValue::String(_))
            | MetadataField::Reflect(ReflectValue::Host(HostValue::String(_))) => {
                Ok(Some(host_value(kind)?))
            }
            _ => Err(invalid_target()),
        };
    }
    descriptor_kind(type_name)
        .map(|kind| HostValue::String(kind.to_owned()))
        .ok_or_else(invalid_target)
        .map(Some)
}

pub(crate) fn owner(target: &ReflectValue) -> ReflectResult<Option<HostValue>> {
    let Some(owner) = field(target, "owner") else {
        return Ok(None);
    };
    match owner {
        MetadataField::Host(HostValue::String(_))
        | MetadataField::Reflect(ReflectValue::Host(HostValue::String(_))) => {
            Ok(Some(host_value(owner)?))
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

pub(crate) fn source_span(target: &ReflectValue) -> ReflectResult<Option<HostValue>> {
    let Some(source_span) = field(target, "source_span") else {
        return Ok(None);
    };
    source_span_value(source_span).map(Some)
}

pub(crate) fn access(target: &ReflectValue) -> ReflectResult<Option<HostValue>> {
    if let Some(access) = field(target, "access") {
        return access_record(access).map(Some);
    }
    if is_access_record(target) {
        return access_record(MetadataField::Reflect(target)).map(Some);
    }
    Ok(None)
}

pub(crate) fn required_permissions(target: &ReflectValue) -> ReflectResult<Option<HostValue>> {
    if let Some(access) = field(target, "access") {
        return required_permissions_from_access(access).map(Some);
    }
    if is_access_record(target) {
        return required_permissions_from_access(MetadataField::Reflect(target)).map(Some);
    }
    Ok(None)
}

pub(crate) fn effects(target: &ReflectValue) -> ReflectResult<Option<HostValue>> {
    if let Some(effects) = field(target, "effects") {
        return effect_set(effects).map(Some);
    }
    if record_type_name(target) == Some("ReflectEffectSet") {
        return effect_set(MetadataField::Reflect(target)).map(Some);
    }
    Ok(None)
}

pub(crate) fn params(target: &ReflectValue) -> ReflectResult<Option<HostValue>> {
    if let Some(params) = field(target, "params") {
        return param_array(params).map(Some);
    }
    if let ReflectValue::Host(HostValue::Array(_)) = target {
        return param_array(MetadataField::Reflect(target)).map(Some);
    }
    Ok(None)
}

pub(crate) fn returns(target: &ReflectValue) -> ReflectResult<Option<HostValue>> {
    if let Some(returns) = field(target, "returns").or_else(|| field(target, "return")) {
        return return_type(returns).map(Some);
    }
    Ok(None)
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

fn record_type_name(target: &ReflectValue) -> Option<&str> {
    match target {
        ReflectValue::Host(HostValue::Record { type_name, .. })
        | ReflectValue::ScriptRecord { type_name, .. }
            if is_reflect_metadata_record(type_name) =>
        {
            Some(type_name)
        }
        _ => None,
    }
}

fn is_reflect_metadata_record(type_name: &str) -> bool {
    type_name.starts_with("Reflect")
}

fn descriptor_kind(type_name: &str) -> Option<&'static str> {
    match type_name {
        "ReflectField" => Some("field"),
        "ReflectFieldAccess" => Some("field_access"),
        "ReflectMethod" => Some("method"),
        "ReflectMethodAccess" => Some("method_access"),
        "ReflectParam" => Some("param"),
        "ReflectEffectSet" => Some("effect_set"),
        "ReflectTrait" => Some("trait"),
        "ReflectTraitMethod" => Some("trait_method"),
        "ReflectVariant" => Some("variant"),
        "ReflectModule" => Some("module"),
        "ReflectFunction" => Some("function"),
        "ReflectFunctionAccess" => Some("function_access"),
        "ReflectSourceSpan" => Some("source_span"),
        _ => None,
    }
}

fn is_access_record(target: &ReflectValue) -> bool {
    matches!(
        record_type_name(target),
        Some("ReflectFieldAccess" | "ReflectMethodAccess" | "ReflectFunctionAccess")
    )
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

fn required_permissions_from_access(value: MetadataField<'_>) -> ReflectResult<HostValue> {
    let Some(required_permissions) = record_field(value, "required_permissions") else {
        return Err(invalid_target());
    };
    string_array(required_permissions)
}

fn record_field<'a>(value: MetadataField<'a>, name: &str) -> Option<MetadataField<'a>> {
    match value {
        MetadataField::Host(HostValue::Record { fields, .. }) => {
            fields.get(name).map(MetadataField::Host)
        }
        MetadataField::Reflect(ReflectValue::Host(HostValue::Record { fields, .. })) => {
            fields.get(name).map(MetadataField::Host)
        }
        MetadataField::Reflect(ReflectValue::ScriptRecord { fields, .. }) => {
            fields.get(name).map(MetadataField::Reflect)
        }
        _ => None,
    }
}

fn string_array(value: MetadataField<'_>) -> ReflectResult<HostValue> {
    match value {
        MetadataField::Host(HostValue::Array(values))
        | MetadataField::Reflect(ReflectValue::Host(HostValue::Array(values))) => values
            .iter()
            .map(|value| match value {
                HostValue::String(value) => Ok(HostValue::String(value.clone())),
                _ => Err(invalid_target()),
            })
            .collect::<ReflectResult<Vec<_>>>()
            .map(HostValue::Array),
        _ => Err(invalid_target()),
    }
}

fn effect_set(value: MetadataField<'_>) -> ReflectResult<HostValue> {
    match value {
        MetadataField::Host(HostValue::Record { type_name, fields })
        | MetadataField::Reflect(ReflectValue::Host(HostValue::Record { type_name, fields }))
            if type_name == "ReflectEffectSet" =>
        {
            Ok(HostValue::Record {
                type_name: type_name.clone(),
                fields: fields.clone(),
            })
        }
        MetadataField::Reflect(ReflectValue::ScriptRecord { type_name, fields })
            if type_name == "ReflectEffectSet" =>
        {
            fields
                .iter()
                .map(|(key, value)| {
                    let ReflectValue::Host(HostValue::Bool(value)) = value else {
                        return Err(invalid_target());
                    };
                    Ok((key.clone(), HostValue::Bool(*value)))
                })
                .collect::<ReflectResult<BTreeMap<_, _>>>()
                .map(|fields| HostValue::Record {
                    type_name: type_name.clone(),
                    fields,
                })
        }
        _ => Err(invalid_target()),
    }
}

fn access_record(value: MetadataField<'_>) -> ReflectResult<HostValue> {
    match value {
        MetadataField::Host(HostValue::Record { type_name, fields })
        | MetadataField::Reflect(ReflectValue::Host(HostValue::Record { type_name, fields }))
            if is_access_type(type_name) =>
        {
            Ok(HostValue::Record {
                type_name: type_name.clone(),
                fields: fields.clone(),
            })
        }
        MetadataField::Reflect(ReflectValue::ScriptRecord { type_name, fields })
            if is_access_type(type_name) =>
        {
            fields
                .iter()
                .map(|(key, value)| {
                    let ReflectValue::Host(value) = value else {
                        return Err(invalid_target());
                    };
                    Ok((key.clone(), value.clone()))
                })
                .collect::<ReflectResult<BTreeMap<_, _>>>()
                .map(|fields| HostValue::Record {
                    type_name: type_name.clone(),
                    fields,
                })
        }
        _ => Err(invalid_target()),
    }
}

fn is_access_type(type_name: &str) -> bool {
    matches!(
        type_name,
        "ReflectFieldAccess" | "ReflectMethodAccess" | "ReflectFunctionAccess"
    )
}

fn param_array(value: MetadataField<'_>) -> ReflectResult<HostValue> {
    match value {
        MetadataField::Host(HostValue::Array(values))
        | MetadataField::Reflect(ReflectValue::Host(HostValue::Array(values))) => values
            .iter()
            .map(param_record)
            .collect::<ReflectResult<Vec<_>>>()
            .map(HostValue::Array),
        _ => Err(invalid_target()),
    }
}

fn param_record(value: &HostValue) -> ReflectResult<HostValue> {
    let HostValue::Record { type_name, fields } = value else {
        return Err(invalid_target());
    };
    if type_name != "ReflectParam" {
        return Err(invalid_target());
    }
    Ok(HostValue::Record {
        type_name: type_name.clone(),
        fields: fields.clone(),
    })
}

fn return_type(value: MetadataField<'_>) -> ReflectResult<HostValue> {
    match value {
        MetadataField::Host(HostValue::Null | HostValue::String(_))
        | MetadataField::Reflect(ReflectValue::Host(HostValue::Null | HostValue::String(_))) => {
            host_value(value)
        }
        _ => Err(invalid_target()),
    }
}

fn source_span_value(value: MetadataField<'_>) -> ReflectResult<HostValue> {
    match value {
        MetadataField::Host(HostValue::Null)
        | MetadataField::Reflect(ReflectValue::Host(HostValue::Null)) => Ok(HostValue::Null),
        MetadataField::Host(HostValue::Record { type_name, fields })
        | MetadataField::Reflect(ReflectValue::Host(HostValue::Record { type_name, fields }))
            if type_name == "ReflectSourceSpan" =>
        {
            Ok(HostValue::Record {
                type_name: type_name.clone(),
                fields: fields.clone(),
            })
        }
        MetadataField::Reflect(ReflectValue::ScriptRecord { type_name, fields })
            if type_name == "ReflectSourceSpan" =>
        {
            fields
                .iter()
                .map(|(key, value)| {
                    let ReflectValue::Host(value) = value else {
                        return Err(invalid_target());
                    };
                    Ok((key.clone(), value.clone()))
                })
                .collect::<ReflectResult<BTreeMap<_, _>>>()
                .map(|fields| HostValue::Record {
                    type_name: type_name.clone(),
                    fields,
                })
        }
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
