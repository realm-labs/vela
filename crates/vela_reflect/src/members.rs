use vela_host::value::HostValue;

mod fields;
mod methods;
mod traits;
mod variants;

use crate::{
    candidates::{candidate_names, ranked_candidates},
    descriptor_targets,
    error::{ReflectError, ReflectErrorKind, ReflectResult},
    metadata::{attrs_value, docs_value, span_value},
    metadata_records,
    permissions::ReflectPolicy,
    registry::{MethodDesc, TypeDesc, TypeKind, TypeRegistry},
    value::{ReflectValue, type_of},
};

pub use fields::{
    all_fields, all_fields_with_policy, field, field_with_policy, fields_with_policy, has_field,
    has_field_with_policy,
};
pub use methods::{
    all_methods, all_methods_with_policy, has_method, has_method_with_policy, method,
    method_with_policy, methods, methods_with_policy,
};
pub use traits::{all_traits, has_trait, trait_by_name, traits};
pub use variants::{
    all_variants, all_variants_with_policy, has_variant, variant, variant_info,
    variant_info_with_policy, variant_is, variants, variants_with_policy,
};

pub fn name(registry: &TypeRegistry, target: &ReflectValue) -> ReflectResult<ReflectValue> {
    match target_type(registry, target) {
        Ok(desc) => Ok(ReflectValue::Host(HostValue::String(desc.key.name.clone()))),
        Err(error) => metadata_records::name(target)?
            .map(ReflectValue::Host)
            .ok_or(error),
    }
}

pub fn id(registry: &TypeRegistry, target: &ReflectValue) -> ReflectResult<ReflectValue> {
    match target_type(registry, target) {
        Ok(desc) => Ok(ReflectValue::Host(HostValue::Int(
            i64::try_from(desc.key.id.get()).unwrap_or(i64::MAX),
        ))),
        Err(error) => metadata_records::id(target)?
            .map(ReflectValue::Host)
            .ok_or(error),
    }
}

pub fn kind(registry: &TypeRegistry, target: &ReflectValue) -> ReflectResult<ReflectValue> {
    match target_type(registry, target) {
        Ok(desc) => Ok(ReflectValue::Host(HostValue::String(
            match desc.kind {
                TypeKind::Null => "null",
                TypeKind::Bool => "bool",
                TypeKind::Int => "int",
                TypeKind::Float => "float",
                TypeKind::String => "string",
                TypeKind::Array => "array",
                TypeKind::Map => "map",
                TypeKind::Set => "set",
                TypeKind::Range => "range",
                TypeKind::Function => "function",
                TypeKind::Closure => "closure",
                TypeKind::Host => "host",
                TypeKind::ScriptStruct => "script_struct",
                TypeKind::ScriptEnum => "script_enum",
            }
            .to_owned(),
        ))),
        Err(error) => metadata_records::kind(target)?
            .map(ReflectValue::Host)
            .ok_or(error),
    }
}

pub fn owner(_registry: &TypeRegistry, target: &ReflectValue) -> ReflectResult<ReflectValue> {
    metadata_records::owner(target)?
        .map(ReflectValue::Host)
        .ok_or_else(|| ReflectError::new(ReflectErrorKind::InvalidTarget))
}

pub fn origin(_registry: &TypeRegistry, target: &ReflectValue) -> ReflectResult<ReflectValue> {
    Ok(ReflectValue::Host(
        metadata_records::origin(target)?.unwrap_or(HostValue::Null),
    ))
}

pub fn attrs(registry: &TypeRegistry, target: &ReflectValue) -> ReflectResult<ReflectValue> {
    match target_type(registry, target) {
        Ok(desc) => Ok(ReflectValue::Host(attrs_value(&desc.attrs))),
        Err(error) => metadata_records::attrs(target)?
            .map(ReflectValue::Host)
            .ok_or(error),
    }
}

pub fn attr(
    registry: &TypeRegistry,
    target: &ReflectValue,
    name: &str,
) -> ReflectResult<ReflectValue> {
    match target_type(registry, target) {
        Ok(desc) => Ok(ReflectValue::Host(
            desc.attrs
                .get(name)
                .map_or(HostValue::Null, |value| HostValue::String(value.to_owned())),
        )),
        Err(error) => metadata_records::attr(target, name)?
            .map(ReflectValue::Host)
            .ok_or(error),
    }
}

pub fn has_attr(registry: &TypeRegistry, target: &ReflectValue, name: &str) -> ReflectResult<bool> {
    match target_type(registry, target) {
        Ok(desc) => Ok(desc.attrs.get(name).is_some()),
        Err(error) => metadata_records::has_attr(target, name)?.ok_or(error),
    }
}

pub fn docs(registry: &TypeRegistry, target: &ReflectValue) -> ReflectResult<ReflectValue> {
    match target_type(registry, target) {
        Ok(desc) => Ok(ReflectValue::Host(docs_value(desc.docs.as_deref()))),
        Err(error) => metadata_records::docs(target)?
            .map(ReflectValue::Host)
            .ok_or(error),
    }
}

pub fn source_span(registry: &TypeRegistry, target: &ReflectValue) -> ReflectResult<ReflectValue> {
    match target_type(registry, target) {
        Ok(desc) => Ok(ReflectValue::Host(span_value(desc.source_span))),
        Err(error) => metadata_records::source_span(target)?
            .map(ReflectValue::Host)
            .ok_or(error),
    }
}

pub fn access(_registry: &TypeRegistry, target: &ReflectValue) -> ReflectResult<ReflectValue> {
    metadata_records::access(target)?
        .map(ReflectValue::Host)
        .ok_or_else(|| ReflectError::new(ReflectErrorKind::InvalidTarget))
}

pub fn required_permissions(
    registry: &TypeRegistry,
    target: &ReflectValue,
) -> ReflectResult<ReflectValue> {
    match target_type(registry, target) {
        Ok(_) => Ok(ReflectValue::Host(HostValue::Array(Vec::new()))),
        Err(error) => metadata_records::required_permissions(target)?
            .map(ReflectValue::Host)
            .ok_or(error),
    }
}

pub fn effects(_registry: &TypeRegistry, target: &ReflectValue) -> ReflectResult<ReflectValue> {
    metadata_records::effects(target)?
        .map(ReflectValue::Host)
        .ok_or_else(|| ReflectError::new(ReflectErrorKind::InvalidTarget))
}

pub fn params(_registry: &TypeRegistry, target: &ReflectValue) -> ReflectResult<ReflectValue> {
    metadata_records::params(target)?
        .map(ReflectValue::Host)
        .ok_or_else(|| ReflectError::new(ReflectErrorKind::InvalidTarget))
}

pub fn returns(_registry: &TypeRegistry, target: &ReflectValue) -> ReflectResult<ReflectValue> {
    metadata_records::returns(target)?
        .map(ReflectValue::Host)
        .ok_or_else(|| ReflectError::new(ReflectErrorKind::InvalidTarget))
}

pub(super) fn target_type<'a>(
    registry: &'a TypeRegistry,
    target: &ReflectValue,
) -> ReflectResult<&'a TypeDesc> {
    if let Some(desc) = type_of(registry, target) {
        return Ok(desc);
    }
    if let Some(desc) = descriptor_targets::type_desc(registry, target)? {
        return Ok(desc);
    }
    match target {
        ReflectValue::HostRef(host_ref) => Err(ReflectError::new(ReflectErrorKind::UnknownType {
            host_type_id: host_ref.type_id,
        })),
        ReflectValue::Host(_)
        | ReflectValue::Closure
        | ReflectValue::Range
        | ReflectValue::Record(_)
        | ReflectValue::Set(_) => Err(ReflectError::new(ReflectErrorKind::InvalidTarget)),
        ReflectValue::ScriptRecord { .. } | ReflectValue::ScriptEnum { .. } => {
            Err(ReflectError::new(ReflectErrorKind::InvalidTarget))
        }
    }
}

pub(super) fn find_method<'a>(desc: &'a TypeDesc, method: &str) -> ReflectResult<&'a MethodDesc> {
    desc.methods
        .iter()
        .find(|candidate| candidate.name == method)
        .ok_or_else(|| {
            let related = method_candidates(desc, method);
            ReflectError::new(ReflectErrorKind::UnknownMethod {
                type_name: desc.key.name.clone(),
                method: method.to_owned(),
                candidates: candidate_names(&related),
                related,
            })
        })
}

pub(super) fn find_method_with_policy<'a>(
    desc: &'a TypeDesc,
    method: &str,
    policy: &ReflectPolicy,
) -> ReflectResult<&'a MethodDesc> {
    desc.methods
        .iter()
        .find(|candidate| candidate.name == method)
        .ok_or_else(|| {
            let related = method_candidates_with_policy(desc, method, policy);
            ReflectError::new(ReflectErrorKind::UnknownMethod {
                type_name: desc.key.name.clone(),
                method: method.to_owned(),
                candidates: candidate_names(&related),
                related,
            })
        })
}

fn method_candidates(desc: &TypeDesc, method: &str) -> Vec<crate::candidates::ReflectCandidate> {
    ranked_candidates(
        method,
        desc.methods
            .iter()
            .map(|method| (method.name.as_str(), method.source_span)),
    )
}

fn method_candidates_with_policy(
    desc: &TypeDesc,
    method: &str,
    policy: &ReflectPolicy,
) -> Vec<crate::candidates::ReflectCandidate> {
    ranked_candidates(
        method,
        desc.methods
            .iter()
            .filter(|method| policy.require_method_access(&desc.key.name, method).is_ok())
            .map(|method| (method.name.as_str(), method.source_span)),
    )
}

#[cfg(test)]
mod tests;
