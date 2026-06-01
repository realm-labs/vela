use vela_host::value::HostValue;

use crate::error::ReflectResult;
use crate::member_records::method_record_with_owner;
use crate::permissions::ReflectPolicy;
use crate::registry::TypeRegistry;
use crate::value::ReflectValue;

use super::{find_method, target_type};

pub fn methods(registry: &TypeRegistry, target: &ReflectValue) -> ReflectResult<ReflectValue> {
    let desc = target_type(registry, target)?;
    Ok(ReflectValue::Host(HostValue::Array(
        desc.methods
            .iter()
            .map(|method| method_record_with_owner(&desc.key.name, method))
            .collect(),
    )))
}

pub fn method(
    registry: &TypeRegistry,
    target: &ReflectValue,
    name: &str,
) -> ReflectResult<ReflectValue> {
    let desc = target_type(registry, target)?;
    let method = find_method(desc, name)?;
    Ok(ReflectValue::Host(method_record_with_owner(
        &desc.key.name,
        method,
    )))
}

pub fn method_with_policy(
    registry: &TypeRegistry,
    target: &ReflectValue,
    name: &str,
    policy: &ReflectPolicy,
) -> ReflectResult<ReflectValue> {
    let desc = target_type(registry, target)?;
    let method = find_method(desc, name)?;
    policy.require_method_access(&desc.key.name, method)?;
    Ok(ReflectValue::Host(method_record_with_owner(
        &desc.key.name,
        method,
    )))
}

pub fn methods_with_policy(
    registry: &TypeRegistry,
    target: &ReflectValue,
    policy: &ReflectPolicy,
) -> ReflectResult<ReflectValue> {
    let desc = target_type(registry, target)?;
    Ok(ReflectValue::Host(HostValue::Array(
        desc.methods
            .iter()
            .filter(|method| policy.require_method_access(&desc.key.name, method).is_ok())
            .map(|method| method_record_with_owner(&desc.key.name, method))
            .collect(),
    )))
}

pub fn all_methods(registry: &TypeRegistry) -> ReflectValue {
    ReflectValue::Host(HostValue::Array(
        registry
            .types()
            .flat_map(|desc| {
                desc.methods
                    .iter()
                    .map(|method| method_record_with_owner(&desc.key.name, method))
            })
            .collect(),
    ))
}

pub fn all_methods_with_policy(registry: &TypeRegistry, policy: &ReflectPolicy) -> ReflectValue {
    ReflectValue::Host(HostValue::Array(
        registry
            .types()
            .flat_map(|desc| {
                desc.methods
                    .iter()
                    .filter(|method| policy.require_method_access(&desc.key.name, method).is_ok())
                    .map(|method| method_record_with_owner(&desc.key.name, method))
            })
            .collect(),
    ))
}

pub fn has_method(
    registry: &TypeRegistry,
    target: &ReflectValue,
    name: &str,
) -> ReflectResult<bool> {
    let desc = target_type(registry, target)?;
    Ok(desc.methods.iter().any(|method| method.name == name))
}

pub fn has_method_with_policy(
    registry: &TypeRegistry,
    target: &ReflectValue,
    name: &str,
    policy: &ReflectPolicy,
) -> ReflectResult<bool> {
    let desc = target_type(registry, target)?;
    Ok(desc.methods.iter().any(|method| {
        method.name == name && policy.require_method_access(&desc.key.name, method).is_ok()
    }))
}
