use vela_host::value::HostValue;

use crate::error::{ReflectError, ReflectErrorKind, ReflectResult};
use crate::value::ReflectValue;

pub(crate) fn name(target: &ReflectValue) -> ReflectResult<Option<HostValue>> {
    scalar_field(target, "name", is_string)
}

pub(crate) fn id(target: &ReflectValue) -> ReflectResult<Option<HostValue>> {
    scalar_field(target, "id", is_int)
}

pub(crate) fn kind(target: &ReflectValue) -> ReflectResult<Option<HostValue>> {
    let Some(type_name) = record_type_name(target) else {
        return Ok(None);
    };
    if let Some(kind) = field(target, "kind") {
        return scalar(kind, is_string).map(Some);
    }
    descriptor_kind(type_name)
        .map(|kind| HostValue::String(kind.to_owned()))
        .ok_or_else(invalid_target)
        .map(Some)
}

pub(crate) fn owner(target: &ReflectValue) -> ReflectResult<Option<HostValue>> {
    scalar_field(target, "owner", is_string)
}

pub(crate) fn origin(target: &ReflectValue) -> ReflectResult<Option<HostValue>> {
    scalar_field(target, "origin", is_null_or_string)
}

pub(crate) fn attrs(target: &ReflectValue) -> ReflectResult<Option<ReflectValue>> {
    let Some(attrs) = field(target, "attrs") else {
        return Ok(None);
    };
    attrs_record(attrs).map(Some)
}

pub(crate) fn attr(target: &ReflectValue, name: &str) -> ReflectResult<Option<HostValue>> {
    let Some(ReflectValue::Record(attrs)) = attrs(target)? else {
        return Ok(None);
    };
    let Some(value) = attrs.get(name) else {
        return Ok(Some(HostValue::Null));
    };
    scalar(value, is_string).map(Some)
}

pub(crate) fn has_attr(target: &ReflectValue, name: &str) -> ReflectResult<Option<bool>> {
    let Some(ReflectValue::Record(attrs)) = attrs(target)? else {
        return Ok(None);
    };
    Ok(Some(attrs.contains_key(name)))
}

pub(crate) fn docs(target: &ReflectValue) -> ReflectResult<Option<HostValue>> {
    scalar_field(target, "docs", is_null_or_string)
}

pub(crate) fn source_span(target: &ReflectValue) -> ReflectResult<Option<ReflectValue>> {
    let Some(source_span) = field(target, "source_span") else {
        return Ok(None);
    };
    source_span_value(source_span).map(Some)
}

pub(crate) fn access(target: &ReflectValue) -> ReflectResult<Option<ReflectValue>> {
    if let Some(access) = field(target, "access") {
        return access_record(access).map(Some);
    }
    if is_access_record(target) {
        return access_record(target).map(Some);
    }
    Ok(None)
}

pub(crate) fn required_permissions(target: &ReflectValue) -> ReflectResult<Option<ReflectValue>> {
    if let Some(access) = field(target, "access") {
        return required_permissions_from_access(access).map(Some);
    }
    if is_access_record(target) {
        return required_permissions_from_access(target).map(Some);
    }
    Ok(None)
}

pub(crate) fn effects(target: &ReflectValue) -> ReflectResult<Option<ReflectValue>> {
    if let Some(effects) = field(target, "effects") {
        return effect_set(effects).map(Some);
    }
    if record_type_name(target) == Some("ReflectEffectSet") {
        return effect_set(target).map(Some);
    }
    Ok(None)
}

pub(crate) fn params(target: &ReflectValue) -> ReflectResult<Option<ReflectValue>> {
    if let Some(params) = field(target, "params") {
        return param_array(params).map(Some);
    }
    if let ReflectValue::Array(_) = target {
        return param_array(target).map(Some);
    }
    Ok(None)
}

pub(crate) fn returns(target: &ReflectValue) -> ReflectResult<Option<HostValue>> {
    if let Some(returns) = field(target, "returns").or_else(|| field(target, "return")) {
        return scalar(returns, is_null_or_string).map(Some);
    }
    Ok(None)
}

fn scalar_field(
    target: &ReflectValue,
    name: &str,
    accepts: fn(&HostValue) -> bool,
) -> ReflectResult<Option<HostValue>> {
    let Some(value) = field(target, name) else {
        return Ok(None);
    };
    scalar(value, accepts).map(Some)
}

fn field<'a>(target: &'a ReflectValue, name: &str) -> Option<&'a ReflectValue> {
    match target {
        ReflectValue::ScriptRecord { type_name, fields }
            if is_reflect_metadata_record(type_name) =>
        {
            fields.get(name)
        }
        _ => None,
    }
}

fn record_type_name(target: &ReflectValue) -> Option<&str> {
    match target {
        ReflectValue::ScriptRecord { type_name, .. } if is_reflect_metadata_record(type_name) => {
            Some(type_name)
        }
        _ => None,
    }
}

pub(crate) fn is_reflect_metadata_record(type_name: &str) -> bool {
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

fn required_permissions_from_access(value: &ReflectValue) -> ReflectResult<ReflectValue> {
    let Some(required_permissions) = record_field(value, "required_permissions") else {
        return Err(invalid_target());
    };
    string_array(required_permissions)
}

fn record_field<'a>(value: &'a ReflectValue, name: &str) -> Option<&'a ReflectValue> {
    match value {
        ReflectValue::ScriptRecord { fields, .. } => fields.get(name),
        _ => None,
    }
}

fn attrs_record(value: &ReflectValue) -> ReflectResult<ReflectValue> {
    let ReflectValue::Record(attrs) = value else {
        return Err(invalid_target());
    };
    for value in attrs.values() {
        scalar(value, is_string)?;
    }
    Ok(value.clone())
}

fn string_array(value: &ReflectValue) -> ReflectResult<ReflectValue> {
    let ReflectValue::Array(values) = value else {
        return Err(invalid_target());
    };
    for value in values {
        scalar(value, is_string)?;
    }
    Ok(value.clone())
}

fn effect_set(value: &ReflectValue) -> ReflectResult<ReflectValue> {
    match value {
        ReflectValue::ScriptRecord { type_name, fields } if type_name == "ReflectEffectSet" => {
            for value in fields.values() {
                scalar(value, is_bool)?;
            }
            Ok(value.clone())
        }
        _ => Err(invalid_target()),
    }
}

fn access_record(value: &ReflectValue) -> ReflectResult<ReflectValue> {
    match value {
        ReflectValue::ScriptRecord { type_name, fields } if is_access_type(type_name) => {
            for (key, value) in fields {
                if key == "required_permissions" {
                    string_array(value)?;
                } else {
                    scalar(value, is_bool)?;
                }
            }
            Ok(value.clone())
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

fn param_array(value: &ReflectValue) -> ReflectResult<ReflectValue> {
    let ReflectValue::Array(values) = value else {
        return Err(invalid_target());
    };
    for value in values {
        param_record(value)?;
    }
    Ok(value.clone())
}

fn param_record(value: &ReflectValue) -> ReflectResult<()> {
    match value {
        ReflectValue::ScriptRecord { type_name, .. } if type_name == "ReflectParam" => Ok(()),
        _ => Err(invalid_target()),
    }
}

fn source_span_value(value: &ReflectValue) -> ReflectResult<ReflectValue> {
    match value {
        ReflectValue::Host(HostValue::Null) => Ok(value.clone()),
        ReflectValue::ScriptRecord { type_name, fields } if type_name == "ReflectSourceSpan" => {
            for value in fields.values() {
                scalar(value, is_int)?;
            }
            Ok(value.clone())
        }
        _ => Err(invalid_target()),
    }
}

fn scalar(value: &ReflectValue, accepts: fn(&HostValue) -> bool) -> ReflectResult<HostValue> {
    let ReflectValue::Host(value) = value else {
        return Err(invalid_target());
    };
    if accepts(value) {
        Ok(value.clone())
    } else {
        Err(invalid_target())
    }
}

fn is_null_or_string(value: &HostValue) -> bool {
    matches!(value, HostValue::Null | HostValue::String(_))
}

fn is_string(value: &HostValue) -> bool {
    matches!(value, HostValue::String(_))
}

fn is_int(value: &HostValue) -> bool {
    matches!(value, HostValue::Scalar(vela_common::ScalarValue::I64(_)))
}

fn is_bool(value: &HostValue) -> bool {
    matches!(value, HostValue::Bool(_))
}

fn invalid_target() -> ReflectError {
    ReflectError::new(ReflectErrorKind::InvalidTarget)
}
