use vela_host::value::HostValue;

use crate::{
    candidates::{candidate_names, ranked_candidates},
    error::{ReflectError, ReflectErrorKind, ReflectResult},
    permissions::ReflectPolicy,
    registry::TypeRegistry,
    value::ReflectValue,
};

use super::descriptors::ModuleDesc;
use super::records::{
    function_record, function_record_host, module_record, module_record_host,
    module_record_host_with_exports, module_record_with_exports,
};

pub fn module(registry: &TypeRegistry, name: &str) -> ReflectResult<ReflectValue> {
    let desc = registry.module_by_name(name).ok_or_else(|| {
        let related = module_candidates(registry, name);
        ReflectError::new(ReflectErrorKind::UnknownModule {
            module: name.to_owned(),
            candidates: candidate_names(&related),
            related,
        })
    })?;
    Ok(module_record(desc))
}

pub fn has_module(registry: &TypeRegistry, name: &str) -> bool {
    registry.module_by_name(name).is_some()
}

pub fn has_module_with_policy(
    registry: &TypeRegistry,
    name: &str,
    _policy: &ReflectPolicy,
) -> bool {
    has_module(registry, name)
}

pub fn modules(registry: &TypeRegistry) -> ReflectValue {
    ReflectValue::Host(HostValue::Array(
        registry.modules().map(module_record_host).collect(),
    ))
}

pub fn module_with_policy(
    registry: &TypeRegistry,
    name: &str,
    policy: &ReflectPolicy,
) -> ReflectResult<ReflectValue> {
    let desc = registry.module_by_name(name).ok_or_else(|| {
        let related = module_candidates(registry, name);
        ReflectError::new(ReflectErrorKind::UnknownModule {
            module: name.to_owned(),
            candidates: candidate_names(&related),
            related,
        })
    })?;
    Ok(module_record_with_exports(
        desc,
        visible_export_names(registry, desc, policy),
    ))
}

pub fn modules_with_policy(registry: &TypeRegistry, policy: &ReflectPolicy) -> ReflectValue {
    ReflectValue::Host(HostValue::Array(
        registry
            .modules()
            .map(|module| {
                module_record_host_with_exports(
                    module,
                    visible_export_names(registry, module, policy),
                )
            })
            .collect(),
    ))
}

pub fn exports(registry: &TypeRegistry, module_name: &str) -> ReflectResult<ReflectValue> {
    let desc = registry.module_by_name(module_name).ok_or_else(|| {
        let related = module_candidates(registry, module_name);
        ReflectError::new(ReflectErrorKind::UnknownModule {
            module: module_name.to_owned(),
            candidates: candidate_names(&related),
            related,
        })
    })?;
    Ok(ReflectValue::Host(HostValue::Array(
        desc.exports
            .iter()
            .map(|export| HostValue::String(export.name.clone()))
            .collect(),
    )))
}

pub fn exports_for_target(
    registry: &TypeRegistry,
    target: &ReflectValue,
) -> ReflectResult<ReflectValue> {
    exports(registry, module_target_name(target)?)
}

pub fn exports_with_policy(
    registry: &TypeRegistry,
    module_name: &str,
    policy: &ReflectPolicy,
) -> ReflectResult<ReflectValue> {
    let desc = registry.module_by_name(module_name).ok_or_else(|| {
        let related = module_candidates(registry, module_name);
        ReflectError::new(ReflectErrorKind::UnknownModule {
            module: module_name.to_owned(),
            candidates: candidate_names(&related),
            related,
        })
    })?;
    Ok(ReflectValue::Host(HostValue::Array(
        visible_export_names(registry, desc, policy)
            .into_iter()
            .map(HostValue::String)
            .collect(),
    )))
}

pub fn exports_for_target_with_policy(
    registry: &TypeRegistry,
    target: &ReflectValue,
    policy: &ReflectPolicy,
) -> ReflectResult<ReflectValue> {
    exports_with_policy(registry, module_target_name(target)?, policy)
}

pub fn function(registry: &TypeRegistry, name: &str) -> ReflectResult<ReflectValue> {
    let desc = registry.function_by_name(name).ok_or_else(|| {
        let related = function_candidates(registry, name);
        ReflectError::new(ReflectErrorKind::UnknownFunction {
            function: name.to_owned(),
            candidates: candidate_names(&related),
            related,
        })
    })?;
    Ok(function_record(desc))
}

pub fn has_function(registry: &TypeRegistry, name: &str) -> bool {
    registry.function_by_name(name).is_some()
}

pub fn has_function_with_policy(
    registry: &TypeRegistry,
    name: &str,
    policy: &ReflectPolicy,
) -> bool {
    registry
        .function_by_name(name)
        .is_some_and(|desc| policy.require_function_access(desc).is_ok())
}

pub fn functions(registry: &TypeRegistry) -> ReflectValue {
    ReflectValue::Host(HostValue::Array(
        registry.functions().map(function_record_host).collect(),
    ))
}

pub fn function_with_policy(
    registry: &TypeRegistry,
    name: &str,
    policy: &ReflectPolicy,
) -> ReflectResult<ReflectValue> {
    let desc = registry.function_by_name(name).ok_or_else(|| {
        let related = function_candidates(registry, name);
        ReflectError::new(ReflectErrorKind::UnknownFunction {
            function: name.to_owned(),
            candidates: candidate_names(&related),
            related,
        })
    })?;
    policy.require_function_access(desc)?;
    Ok(function_record(desc))
}

pub fn functions_with_policy(registry: &TypeRegistry, policy: &ReflectPolicy) -> ReflectValue {
    ReflectValue::Host(HostValue::Array(
        registry
            .functions()
            .filter(|function| policy.require_function_access(function).is_ok())
            .map(function_record_host)
            .collect(),
    ))
}

pub fn callable_function_name_with_policy(
    registry: &TypeRegistry,
    target: &ReflectValue,
    policy: &ReflectPolicy,
) -> ReflectResult<Option<String>> {
    let Some(name) = function_target_name(target)? else {
        return Ok(None);
    };
    let desc = registry.function_by_name(name).ok_or_else(|| {
        let related = function_candidates(registry, name);
        ReflectError::new(ReflectErrorKind::UnknownFunction {
            function: name.to_owned(),
            candidates: candidate_names(&related),
            related,
        })
    })?;
    policy.require_function_call_access(desc)?;
    Ok(Some(desc.name.clone()))
}

fn module_candidates(
    registry: &TypeRegistry,
    name: &str,
) -> Vec<crate::candidates::ReflectCandidate> {
    ranked_candidates(
        name,
        registry
            .modules()
            .map(|module| (module.name.as_str(), module.source_span)),
    )
}

fn function_candidates(
    registry: &TypeRegistry,
    name: &str,
) -> Vec<crate::candidates::ReflectCandidate> {
    ranked_candidates(
        name,
        registry
            .functions()
            .map(|function| (function.name.as_str(), function.source_span)),
    )
}

fn visible_export_names(
    registry: &TypeRegistry,
    desc: &ModuleDesc,
    policy: &ReflectPolicy,
) -> Vec<String> {
    desc.exports
        .iter()
        .filter(|export| {
            let Some(function_id) = export.function else {
                return true;
            };
            registry
                .function_by_id(function_id)
                .is_some_and(|function| policy.require_function_access(function).is_ok())
        })
        .map(|export| export.name.clone())
        .collect()
}

fn function_target_name(target: &ReflectValue) -> ReflectResult<Option<&str>> {
    match target {
        ReflectValue::Host(HostValue::Record { type_name, fields })
            if type_name == "ReflectFunction" =>
        {
            match fields.get("name") {
                Some(HostValue::String(name)) => Ok(Some(name.as_str())),
                _ => Err(ReflectError::new(ReflectErrorKind::InvalidTarget)),
            }
        }
        ReflectValue::ScriptRecord { type_name, fields } if type_name == "ReflectFunction" => {
            match fields.get("name") {
                Some(ReflectValue::Host(HostValue::String(name))) => Ok(Some(name.as_str())),
                _ => Err(ReflectError::new(ReflectErrorKind::InvalidTarget)),
            }
        }
        _ => Ok(None),
    }
}

fn module_target_name(target: &ReflectValue) -> ReflectResult<&str> {
    match target {
        ReflectValue::Host(HostValue::String(name)) => Ok(name),
        ReflectValue::Host(HostValue::Record { type_name, fields })
            if type_name == "ReflectModule" =>
        {
            match fields.get("name") {
                Some(HostValue::String(name)) => Ok(name),
                _ => Err(ReflectError::new(ReflectErrorKind::InvalidTarget)),
            }
        }
        ReflectValue::ScriptRecord { type_name, fields } if type_name == "ReflectModule" => {
            match fields.get("name") {
                Some(ReflectValue::Host(HostValue::String(name))) => Ok(name),
                _ => Err(ReflectError::new(ReflectErrorKind::InvalidTarget)),
            }
        }
        _ => Err(ReflectError::new(ReflectErrorKind::InvalidTarget)),
    }
}
