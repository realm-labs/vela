use std::collections::BTreeMap;

use vela_host::HostValue;

use crate::{
    FieldDesc, MethodDesc, ReflectError, ReflectErrorKind, ReflectPolicy, ReflectResult,
    ReflectValue, TraitDesc, TraitMethodDesc, TypeDesc, TypeKind, TypeRegistry, VariantDesc,
    candidates::{candidate_names, ranked_candidates},
    metadata::{attrs_value, docs_value, span_value},
    metadata_records, type_of,
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
        Ok(desc) => Ok(ReflectValue::Host(HostValue::Int(i64::from(
            desc.key.id.get(),
        )))),
        Err(error) => metadata_records::id(target)?
            .map(ReflectValue::Host)
            .ok_or(error),
    }
}

pub fn kind(registry: &TypeRegistry, target: &ReflectValue) -> ReflectResult<ReflectValue> {
    match target_type(registry, target) {
        Ok(desc) => Ok(ReflectValue::Host(HostValue::String(
            match desc.kind {
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

pub fn field(
    registry: &TypeRegistry,
    target: &ReflectValue,
    name: &str,
) -> ReflectResult<ReflectValue> {
    let desc = target_type(registry, target)?;
    let field = find_field(desc, name)?;
    Ok(ReflectValue::Host(field_record(field)))
}

pub fn field_with_policy(
    registry: &TypeRegistry,
    target: &ReflectValue,
    name: &str,
    policy: &ReflectPolicy,
) -> ReflectResult<ReflectValue> {
    let desc = target_type(registry, target)?;
    let field = find_field(desc, name)?;
    policy.require_field_read_access(&desc.key.name, field)?;
    Ok(ReflectValue::Host(field_record(field)))
}

pub fn field_names_with_policy(
    registry: &TypeRegistry,
    target: &ReflectValue,
    policy: &ReflectPolicy,
) -> ReflectResult<ReflectValue> {
    let desc = target_type(registry, target)?;
    Ok(ReflectValue::Host(HostValue::Array(
        desc.fields
            .iter()
            .filter(|field| {
                policy
                    .require_field_read_access(&desc.key.name, field)
                    .is_ok()
            })
            .map(|field| HostValue::String(field.name.clone()))
            .collect(),
    )))
}

pub fn all_fields(registry: &TypeRegistry) -> ReflectValue {
    ReflectValue::Host(HostValue::Array(
        registry
            .types()
            .flat_map(|desc| {
                desc.fields
                    .iter()
                    .map(|field| field_record_with_owner(&desc.key.name, field))
            })
            .collect(),
    ))
}

pub fn all_fields_with_policy(registry: &TypeRegistry, policy: &ReflectPolicy) -> ReflectValue {
    ReflectValue::Host(HostValue::Array(
        registry
            .types()
            .flat_map(|desc| {
                desc.fields
                    .iter()
                    .filter(|field| {
                        policy
                            .require_field_read_access(&desc.key.name, field)
                            .is_ok()
                    })
                    .map(|field| field_record_with_owner(&desc.key.name, field))
            })
            .collect(),
    ))
}

pub fn has_field(
    registry: &TypeRegistry,
    target: &ReflectValue,
    name: &str,
) -> ReflectResult<bool> {
    let desc = target_type(registry, target)?;
    Ok(desc.fields.iter().any(|field| field.name == name))
}

pub fn has_field_with_policy(
    registry: &TypeRegistry,
    target: &ReflectValue,
    name: &str,
    policy: &ReflectPolicy,
) -> ReflectResult<bool> {
    let desc = target_type(registry, target)?;
    Ok(desc.fields.iter().any(|field| {
        field.name == name
            && policy
                .require_field_read_access(&desc.key.name, field)
                .is_ok()
    }))
}

pub fn methods(registry: &TypeRegistry, target: &ReflectValue) -> ReflectResult<ReflectValue> {
    let desc = target_type(registry, target)?;
    Ok(ReflectValue::Host(HostValue::Array(
        desc.methods.iter().map(method_record).collect(),
    )))
}

pub fn method(
    registry: &TypeRegistry,
    target: &ReflectValue,
    name: &str,
) -> ReflectResult<ReflectValue> {
    let desc = target_type(registry, target)?;
    let method = find_method(desc, name)?;
    Ok(ReflectValue::Host(method_record(method)))
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
    Ok(ReflectValue::Host(method_record(method)))
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
            .map(method_record)
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

pub fn traits(registry: &TypeRegistry, target: &ReflectValue) -> ReflectResult<ReflectValue> {
    let desc = target_type(registry, target)?;
    Ok(ReflectValue::Host(HostValue::Array(
        desc.traits.iter().map(trait_record).collect(),
    )))
}

pub fn all_traits(registry: &TypeRegistry) -> ReflectValue {
    let mut traits = BTreeMap::new();
    for desc in registry.traits() {
        traits.insert(desc.name.clone(), desc);
    }
    for desc in registry
        .types()
        .flat_map(|type_desc| type_desc.traits.iter())
    {
        traits.entry(desc.name.clone()).or_insert(desc);
    }
    ReflectValue::Host(HostValue::Array(
        traits.into_values().map(trait_record).collect(),
    ))
}

pub fn trait_by_name(registry: &TypeRegistry, name: &str) -> ReflectResult<ReflectValue> {
    let desc = registry.trait_metadata_by_name(name).ok_or_else(|| {
        let candidates = registry.known_trait_candidates();
        let related = ranked_candidates(
            name,
            candidates
                .iter()
                .map(|(candidate, span)| (candidate.as_str(), *span)),
        );
        ReflectError::new(ReflectErrorKind::UnknownTrait {
            trait_name: name.to_owned(),
            candidates: candidate_names(&related),
            related,
        })
    })?;
    Ok(ReflectValue::Host(trait_record(desc)))
}

pub fn has_trait(registry: &TypeRegistry, name: &str) -> bool {
    registry.trait_metadata_by_name(name).is_some()
}

pub fn variants(registry: &TypeRegistry, target: &ReflectValue) -> ReflectResult<ReflectValue> {
    let desc = target_type(registry, target)?;
    Ok(ReflectValue::Host(HostValue::Array(
        desc.variants.iter().map(variant_record).collect(),
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
                variant_record_with_fields(
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
    Ok(ReflectValue::Host(variant_record(variant)))
}

pub fn variant_info_with_policy(
    registry: &TypeRegistry,
    target: &ReflectValue,
    name: &str,
    policy: &ReflectPolicy,
) -> ReflectResult<ReflectValue> {
    let desc = target_type(registry, target)?;
    let variant = find_variant(desc, name)?;
    Ok(ReflectValue::Host(variant_record_with_fields(
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

fn target_type<'a>(
    registry: &'a TypeRegistry,
    target: &ReflectValue,
) -> ReflectResult<&'a TypeDesc> {
    if let Some(desc) = type_of(registry, target) {
        return Ok(desc);
    }
    match target {
        ReflectValue::HostRef(host_ref) => Err(ReflectError::new(ReflectErrorKind::UnknownType {
            host_type_id: host_ref.type_id,
        })),
        ReflectValue::Host(_) | ReflectValue::Record(_) => {
            Err(ReflectError::new(ReflectErrorKind::InvalidTarget))
        }
        ReflectValue::ScriptRecord { .. } | ReflectValue::ScriptEnum { .. } => {
            Err(ReflectError::new(ReflectErrorKind::InvalidTarget))
        }
    }
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

fn find_field<'a>(desc: &'a TypeDesc, field: &str) -> ReflectResult<&'a FieldDesc> {
    desc.fields
        .iter()
        .find(|candidate| candidate.name == field)
        .ok_or_else(|| {
            let related = field_candidates(desc, field);
            ReflectError::new(ReflectErrorKind::UnknownField {
                type_name: desc.key.name.clone(),
                field: field.to_owned(),
                candidates: candidate_names(&related),
                related,
            })
        })
}

fn find_method<'a>(desc: &'a TypeDesc, method: &str) -> ReflectResult<&'a MethodDesc> {
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

fn find_variant<'a>(desc: &'a TypeDesc, variant: &str) -> ReflectResult<&'a VariantDesc> {
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

fn field_candidates(desc: &TypeDesc, field: &str) -> Vec<crate::ReflectCandidate> {
    ranked_candidates(
        field,
        desc.fields
            .iter()
            .map(|field| (field.name.as_str(), field.source_span)),
    )
}

fn method_candidates(desc: &TypeDesc, method: &str) -> Vec<crate::ReflectCandidate> {
    ranked_candidates(
        method,
        desc.methods
            .iter()
            .map(|method| (method.name.as_str(), method.source_span)),
    )
}

fn variant_candidates(desc: &TypeDesc, variant: &str) -> Vec<crate::ReflectCandidate> {
    ranked_candidates(
        variant,
        desc.variants
            .iter()
            .map(|variant| (variant.name.as_str(), variant.source_span)),
    )
}

fn method_record(method: &MethodDesc) -> HostValue {
    method_record_from_fields(method_record_fields(method))
}

fn method_record_with_owner(type_name: &str, method: &MethodDesc) -> HostValue {
    let mut fields = BTreeMap::new();
    fields.insert("owner".to_owned(), HostValue::String(type_name.to_owned()));
    fields.extend(method_record_fields(method));
    method_record_from_fields(fields)
}

fn method_record_fields(method: &MethodDesc) -> BTreeMap<String, HostValue> {
    let mut fields = BTreeMap::new();
    fields.insert("id".to_owned(), HostValue::Int(i64::from(method.id.get())));
    fields.insert("name".to_owned(), HostValue::String(method.name.clone()));
    fields.insert(
        "params".to_owned(),
        HostValue::Array(method.params.iter().map(method_param_record).collect()),
    );
    fields.insert(
        "return".to_owned(),
        method
            .return_type
            .as_ref()
            .map_or(HostValue::Null, |return_type| {
                HostValue::String(return_type.clone())
            }),
    );
    fields.insert(
        "returns".to_owned(),
        method
            .return_type
            .as_ref()
            .map_or(HostValue::Null, |return_type| {
                HostValue::String(return_type.clone())
            }),
    );
    fields.insert("effects".to_owned(), method_effects_record(method));
    fields.insert("access".to_owned(), method_access_record(method));
    fields.insert("docs".to_owned(), docs_value(method.docs.as_deref()));
    fields.insert("attrs".to_owned(), attrs_value(&method.attrs));
    fields.insert("source_span".to_owned(), span_value(method.source_span));
    fields
}

fn method_record_from_fields(fields: BTreeMap<String, HostValue>) -> HostValue {
    HostValue::Record {
        type_name: "ReflectMethod".to_owned(),
        fields,
    }
}

fn method_param_record(param: &crate::MethodParamDesc) -> HostValue {
    let mut fields = BTreeMap::new();
    fields.insert("name".to_owned(), HostValue::String(param.name.clone()));
    fields.insert(
        "type".to_owned(),
        param
            .type_hint
            .as_ref()
            .map_or(HostValue::Null, |hint| HostValue::String(hint.clone())),
    );
    fields.insert("defaulted".to_owned(), HostValue::Bool(param.has_default));
    HostValue::Record {
        type_name: "ReflectParam".to_owned(),
        fields,
    }
}

fn method_effects_record(method: &MethodDesc) -> HostValue {
    HostValue::Record {
        type_name: "ReflectEffectSet".to_owned(),
        fields: BTreeMap::from([
            (
                "reads_host".to_owned(),
                HostValue::Bool(method.effects.reads_host),
            ),
            (
                "writes_host".to_owned(),
                HostValue::Bool(method.effects.writes_host),
            ),
            (
                "emits_events".to_owned(),
                HostValue::Bool(method.effects.emits_events),
            ),
        ]),
    }
}

fn method_access_record(method: &MethodDesc) -> HostValue {
    HostValue::Record {
        type_name: "ReflectMethodAccess".to_owned(),
        fields: BTreeMap::from([
            ("public".to_owned(), HostValue::Bool(method.access.public)),
            (
                "reflect_callable".to_owned(),
                HostValue::Bool(method.access.reflect_callable),
            ),
            (
                "required_permissions".to_owned(),
                HostValue::Array(
                    method
                        .access
                        .required_permissions()
                        .iter()
                        .map(|permission| HostValue::String(permission.clone()))
                        .collect(),
                ),
            ),
        ]),
    }
}

fn trait_record(trait_desc: &TraitDesc) -> HostValue {
    let mut fields = BTreeMap::new();
    fields.insert(
        "id".to_owned(),
        HostValue::Int(i64::from(trait_desc.id.get())),
    );
    fields.insert(
        "name".to_owned(),
        HostValue::String(trait_desc.name.clone()),
    );
    fields.insert(
        "methods".to_owned(),
        HostValue::Array(trait_desc.methods.iter().map(trait_method_record).collect()),
    );
    fields.insert("docs".to_owned(), docs_value(trait_desc.docs.as_deref()));
    fields.insert("attrs".to_owned(), attrs_value(&trait_desc.attrs));
    fields.insert("source_span".to_owned(), span_value(trait_desc.source_span));
    HostValue::Record {
        type_name: "ReflectTrait".to_owned(),
        fields,
    }
}

fn trait_method_record(method: &TraitMethodDesc) -> HostValue {
    let mut fields = BTreeMap::new();
    fields.insert("id".to_owned(), HostValue::Int(i64::from(method.id.get())));
    fields.insert("name".to_owned(), HostValue::String(method.name.clone()));
    fields.insert(
        "params".to_owned(),
        HostValue::Array(method.params.iter().map(method_param_record).collect()),
    );
    fields.insert(
        "return".to_owned(),
        method
            .return_type
            .as_ref()
            .map_or(HostValue::Null, |return_type| {
                HostValue::String(return_type.clone())
            }),
    );
    fields.insert(
        "returns".to_owned(),
        method
            .return_type
            .as_ref()
            .map_or(HostValue::Null, |return_type| {
                HostValue::String(return_type.clone())
            }),
    );
    fields.insert("defaulted".to_owned(), HostValue::Bool(method.has_default));
    fields.insert("docs".to_owned(), docs_value(method.docs.as_deref()));
    fields.insert("attrs".to_owned(), attrs_value(&method.attrs));
    fields.insert("source_span".to_owned(), span_value(method.source_span));
    HostValue::Record {
        type_name: "ReflectTraitMethod".to_owned(),
        fields,
    }
}

fn variant_record(variant: &VariantDesc) -> HostValue {
    variant_record_with_fields(variant, variant.fields.iter())
}

fn variant_record_with_owner(type_name: &str, variant: &VariantDesc) -> HostValue {
    variant_record_with_owner_and_fields(type_name, variant, variant.fields.iter())
}

fn variant_record_with_owner_and_fields<'a>(
    type_name: &str,
    variant: &VariantDesc,
    variant_fields: impl IntoIterator<Item = &'a FieldDesc>,
) -> HostValue {
    let mut fields = variant_record_fields(variant, variant_fields);
    fields.insert("owner".to_owned(), HostValue::String(type_name.to_owned()));
    variant_record_from_fields(fields)
}

fn variant_record_with_fields<'a>(
    variant: &VariantDesc,
    variant_fields: impl IntoIterator<Item = &'a FieldDesc>,
) -> HostValue {
    variant_record_from_fields(variant_record_fields(variant, variant_fields))
}

fn variant_record_fields<'a>(
    variant: &VariantDesc,
    variant_fields: impl IntoIterator<Item = &'a FieldDesc>,
) -> BTreeMap<String, HostValue> {
    let mut fields = BTreeMap::new();
    fields.insert("id".to_owned(), HostValue::Int(i64::from(variant.id.get())));
    fields.insert("name".to_owned(), HostValue::String(variant.name.clone()));
    fields.insert(
        "fields".to_owned(),
        HostValue::Array(variant_fields.into_iter().map(field_record).collect()),
    );
    fields.insert("docs".to_owned(), docs_value(variant.docs.as_deref()));
    fields.insert("attrs".to_owned(), attrs_value(&variant.attrs));
    fields.insert("source_span".to_owned(), span_value(variant.source_span));
    fields
}

fn variant_record_from_fields(fields: BTreeMap<String, HostValue>) -> HostValue {
    HostValue::Record {
        type_name: "ReflectVariant".to_owned(),
        fields,
    }
}

fn field_record(field: &FieldDesc) -> HostValue {
    field_record_from_fields(field_record_fields(field))
}

fn field_record_with_owner(type_name: &str, field: &FieldDesc) -> HostValue {
    let mut fields = BTreeMap::new();
    fields.insert("owner".to_owned(), HostValue::String(type_name.to_owned()));
    fields.extend(field_record_fields(field));
    field_record_from_fields(fields)
}

fn field_record_fields(field: &FieldDesc) -> BTreeMap<String, HostValue> {
    let mut fields = BTreeMap::new();
    fields.insert("id".to_owned(), HostValue::Int(i64::from(field.id.get())));
    fields.insert("name".to_owned(), HostValue::String(field.name.clone()));
    fields.insert(
        "type".to_owned(),
        field
            .type_hint
            .as_ref()
            .filter(|hint| !hint.is_empty())
            .map_or(HostValue::Null, |hint| HostValue::String(hint.clone())),
    );
    fields.insert("writable".to_owned(), HostValue::Bool(field.writable));
    fields.insert("defaulted".to_owned(), HostValue::Bool(field.has_default));
    fields.insert("access".to_owned(), field_access_record(field));
    fields.insert("docs".to_owned(), docs_value(field.docs.as_deref()));
    fields.insert("attrs".to_owned(), attrs_value(&field.attrs));
    fields.insert("source_span".to_owned(), span_value(field.source_span));
    fields
}

fn field_record_from_fields(fields: BTreeMap<String, HostValue>) -> HostValue {
    HostValue::Record {
        type_name: "ReflectField".to_owned(),
        fields,
    }
}

fn field_access_record(field: &FieldDesc) -> HostValue {
    HostValue::Record {
        type_name: "ReflectFieldAccess".to_owned(),
        fields: BTreeMap::from([
            (
                "readable".to_owned(),
                HostValue::Bool(field.access.readable),
            ),
            (
                "writable".to_owned(),
                HostValue::Bool(field.access.writable),
            ),
            (
                "reflect_readable".to_owned(),
                HostValue::Bool(field.access.reflect_readable),
            ),
            (
                "reflect_writable".to_owned(),
                HostValue::Bool(field.access.reflect_writable),
            ),
            (
                "required_permissions".to_owned(),
                HostValue::Array(
                    field
                        .access
                        .required_permissions()
                        .iter()
                        .map(|permission| HostValue::String(permission.clone()))
                        .collect(),
                ),
            ),
        ]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_common::{
        FieldId, HostMethodId, HostObjectId, HostTypeId, MethodId, SourceId, Span, TypeId,
        VariantId,
    };
    use vela_host::HostRef;

    use crate::{MethodParamDesc, TraitMethodDesc, TypeKey, TypeKind};

    fn player_ref() -> HostRef {
        HostRef::new(HostTypeId::new(1), HostObjectId::new(7), 3)
    }

    fn registry() -> TypeRegistry {
        let mut registry = TypeRegistry::new();
        registry.register(
            TypeDesc::new(TypeKey::new(TypeId::new(100), "Player"))
                .host_type(HostTypeId::new(1))
                .docs("A player host object.")
                .attr("domain", "gameplay")
                .field(FieldDesc::new(FieldId::new(1), "id"))
                .field(
                    FieldDesc::new(FieldId::new(2), "level")
                        .writable(true)
                        .type_hint("int")
                        .source_span(Span::new(SourceId::new(8), 50, 55))
                        .docs("Current level.")
                        .attr("unit", "level"),
                )
                .method(
                    MethodDesc::new(HostMethodId::new(5), "grant_exp")
                        .param(MethodParamDesc::new("amount").type_hint("int"))
                        .return_type("bool")
                        .source_span(Span::new(SourceId::new(8), 60, 80))
                        .effects(crate::MethodEffectSet::host_write())
                        .access(
                            crate::MethodAccess::new()
                                .reflect_callable(true)
                                .require_permission("player.grant_exp"),
                        )
                        .docs("Grant experience.")
                        .attr("effect", "write"),
                )
                .trait_impl(
                    TraitDesc::new("Damageable")
                        .source_span(Span::new(SourceId::new(8), 20, 40))
                        .docs("Can take damage.")
                        .attr("combat", "true")
                        .method(
                            TraitMethodDesc::new(MethodId::new(9), "damage")
                                .param(MethodParamDesc::new("amount").type_hint("int"))
                                .return_type("int")
                                .defaulted(true)
                                .docs("Apply damage.")
                                .attr("default", "true"),
                        ),
                ),
        );
        registry.register(
            TypeDesc::new(TypeKey::new(TypeId::new(200), "QuestProgress"))
                .kind(TypeKind::ScriptEnum)
                .variant(
                    VariantDesc::new(VariantId::new(11), "Active")
                        .source_span(Span::new(SourceId::new(8), 90, 100))
                        .docs("Quest is active.")
                        .attr("state", "open")
                        .field(FieldDesc::new(FieldId::new(12), "count")),
                )
                .variant(VariantDesc::new(VariantId::new(13), "Finished")),
        );
        registry
    }

    #[test]
    fn name_kind_and_field_queries_return_copied_metadata() {
        let registry = registry();
        let target = ReflectValue::HostRef(player_ref());

        assert_eq!(
            name(&registry, &target).expect("name"),
            ReflectValue::Host(HostValue::String("Player".to_owned()))
        );
        assert_eq!(
            kind(&registry, &target).expect("kind"),
            ReflectValue::Host(HostValue::String("host".to_owned()))
        );
        assert_eq!(
            docs(&registry, &target).expect("docs"),
            ReflectValue::Host(HostValue::String("A player host object.".to_owned()))
        );
        assert_eq!(
            attrs(&registry, &target).expect("attrs"),
            ReflectValue::Host(HostValue::Map(BTreeMap::from([(
                "domain".to_owned(),
                HostValue::String("gameplay".to_owned())
            )])))
        );
        assert!(has_field(&registry, &target, "level").expect("has field"));

        let field_metadata = field(&registry, &target, "level").expect("field");
        let ReflectValue::Host(HostValue::Record { fields, .. }) = &field_metadata else {
            panic!("field metadata should be a record");
        };
        assert_eq!(fields.get("writable"), Some(&HostValue::Bool(true)));
        assert_eq!(
            fields.get("type"),
            Some(&HostValue::String("int".to_owned()))
        );
        assert_eq!(
            fields.get("access"),
            Some(&HostValue::Record {
                type_name: "ReflectFieldAccess".to_owned(),
                fields: BTreeMap::from([
                    ("readable".to_owned(), HostValue::Bool(true)),
                    ("writable".to_owned(), HostValue::Bool(true)),
                    ("reflect_readable".to_owned(), HostValue::Bool(true)),
                    ("reflect_writable".to_owned(), HostValue::Bool(true)),
                    ("required_permissions".to_owned(), HostValue::Array(vec![])),
                ]),
            })
        );
        assert_eq!(
            fields.get("docs"),
            Some(&HostValue::String("Current level.".to_owned()))
        );
        assert_eq!(
            fields.get("attrs"),
            Some(&HostValue::Map(BTreeMap::from([(
                "unit".to_owned(),
                HostValue::String("level".to_owned())
            )])))
        );
        assert_eq!(
            fields.get("source_span"),
            Some(&span_value(Some(Span::new(SourceId::new(8), 50, 55))))
        );
        assert_eq!(
            source_span(&registry, &field_metadata).expect("field source span"),
            ReflectValue::Host(span_value(Some(Span::new(SourceId::new(8), 50, 55))))
        );
        assert_eq!(
            docs(&registry, &field_metadata).expect("field docs"),
            ReflectValue::Host(HostValue::String("Current level.".to_owned()))
        );
        assert_eq!(
            name(&registry, &field_metadata).expect("field metadata name"),
            ReflectValue::Host(HostValue::String("level".to_owned()))
        );
        assert_eq!(
            kind(&registry, &field_metadata).expect("field metadata kind"),
            ReflectValue::Host(HostValue::String("field".to_owned()))
        );
        assert_eq!(
            attrs(&registry, &field_metadata).expect("field attrs"),
            ReflectValue::Host(HostValue::Map(BTreeMap::from([(
                "unit".to_owned(),
                HostValue::String("level".to_owned())
            )])))
        );
        assert_eq!(
            attr(&registry, &field_metadata, "unit").expect("field attr"),
            ReflectValue::Host(HostValue::String("level".to_owned()))
        );
        assert!(has_attr(&registry, &field_metadata, "unit").expect("field has attr"));
        assert_eq!(
            attr(&registry, &field_metadata, "missing").expect("missing field attr"),
            ReflectValue::Host(HostValue::Null)
        );
        assert!(!has_attr(&registry, &field_metadata, "missing").expect("missing field attr"));
        let ReflectValue::Host(HostValue::Array(all_fields)) = all_fields(&registry) else {
            panic!("field list should be an array");
        };
        assert_eq!(all_fields.len(), 2);
        let HostValue::Record {
            fields: field_list_item,
            ..
        } = &all_fields[1]
        else {
            panic!("field list item should be a record");
        };
        assert_eq!(
            field_list_item.get("owner"),
            Some(&HostValue::String("Player".to_owned()))
        );
        assert_eq!(
            field_list_item.get("name"),
            Some(&HostValue::String("level".to_owned()))
        );

        let error = field(&registry, &target, "levle").expect_err("unknown field");
        assert_eq!(
            error.kind,
            ReflectErrorKind::UnknownField {
                type_name: "Player".to_owned(),
                field: "levle".to_owned(),
                candidates: vec!["level".to_owned(), "id".to_owned()],
                related: vec![
                    crate::ReflectCandidate::new(
                        "level",
                        Some(Span::new(SourceId::new(8), 50, 55))
                    ),
                    crate::ReflectCandidate::new("id", None),
                ],
            }
        );
    }

    #[test]
    fn method_trait_and_variant_queries_return_copied_metadata() {
        let registry = registry();

        assert!(
            has_method(&registry, &ReflectValue::HostRef(player_ref()), "grant_exp")
                .expect("has method")
        );
        let ReflectValue::Host(HostValue::Array(methods)) =
            methods(&registry, &ReflectValue::HostRef(player_ref())).expect("methods")
        else {
            panic!("methods should be an array");
        };
        assert_eq!(methods.len(), 1);
        let HostValue::Record { fields, .. } = &methods[0] else {
            panic!("method metadata should be a record");
        };
        assert_eq!(
            fields.get("return"),
            Some(&HostValue::String("bool".to_owned()))
        );
        assert_eq!(
            fields.get("returns"),
            Some(&HostValue::String("bool".to_owned()))
        );
        let Some(HostValue::Array(params)) = fields.get("params") else {
            panic!("method params should be an array");
        };
        assert_eq!(params.len(), 1);
        let HostValue::Record {
            fields: param_fields,
            ..
        } = &params[0]
        else {
            panic!("method param should be a record");
        };
        assert_eq!(
            param_fields.get("name"),
            Some(&HostValue::String("amount".to_owned()))
        );
        assert_eq!(
            param_fields.get("type"),
            Some(&HostValue::String("int".to_owned()))
        );
        let Some(HostValue::Record {
            fields: effect_fields,
            ..
        }) = fields.get("effects")
        else {
            panic!("method effects should be a record");
        };
        assert_eq!(
            effect_fields.get("writes_host"),
            Some(&HostValue::Bool(true))
        );
        let Some(HostValue::Record {
            fields: access_fields,
            ..
        }) = fields.get("access")
        else {
            panic!("method access should be a record");
        };
        assert_eq!(
            access_fields.get("reflect_callable"),
            Some(&HostValue::Bool(true))
        );
        assert_eq!(
            access_fields.get("required_permissions"),
            Some(&HostValue::Array(vec![HostValue::String(
                "player.grant_exp".to_owned()
            )]))
        );
        assert_eq!(
            fields.get("source_span"),
            Some(&span_value(Some(Span::new(SourceId::new(8), 60, 80))))
        );
        let ReflectValue::Host(HostValue::Array(all_methods)) = all_methods(&registry) else {
            panic!("method list should be an array");
        };
        assert_eq!(all_methods.len(), 1);
        let HostValue::Record {
            fields: method_list_item,
            ..
        } = &all_methods[0]
        else {
            panic!("method list item should be a record");
        };
        assert_eq!(
            method_list_item.get("owner"),
            Some(&HostValue::String("Player".to_owned()))
        );
        assert_eq!(
            method_list_item.get("name"),
            Some(&HostValue::String("grant_exp".to_owned()))
        );
        let ReflectValue::Host(HostValue::Record {
            fields: single_method,
            ..
        }) = method(&registry, &ReflectValue::HostRef(player_ref()), "grant_exp")
            .expect("method metadata")
        else {
            panic!("single method metadata should be a record");
        };
        assert_eq!(
            single_method.get("name"),
            Some(&HostValue::String("grant_exp".to_owned()))
        );
        assert_eq!(
            single_method.get("attrs"),
            Some(&HostValue::Map(BTreeMap::from([(
                "effect".to_owned(),
                HostValue::String("write".to_owned())
            )])))
        );
        let unknown = method(&registry, &ReflectValue::HostRef(player_ref()), "grant_xp")
            .expect_err("unknown method");
        assert_eq!(
            unknown.kind,
            ReflectErrorKind::UnknownMethod {
                type_name: "Player".to_owned(),
                method: "grant_xp".to_owned(),
                candidates: vec!["grant_exp".to_owned()],
                related: vec![crate::ReflectCandidate::new(
                    "grant_exp",
                    Some(Span::new(SourceId::new(8), 60, 80))
                )],
            }
        );

        let ReflectValue::Host(HostValue::Array(traits)) =
            traits(&registry, &ReflectValue::HostRef(player_ref())).expect("traits")
        else {
            panic!("traits should be an array");
        };
        assert_eq!(traits.len(), 1);
        assert!(has_trait(&registry, "Damageable"));
        assert!(!has_trait(&registry, "Trackable"));

        let target = ReflectValue::ScriptEnum {
            enum_name: "QuestProgress".to_owned(),
            variant: "Active".to_owned(),
            fields: BTreeMap::new(),
        };
        assert_eq!(
            variant(&target).expect("variant"),
            ReflectValue::Host(HostValue::String("Active".to_owned()))
        );
        assert!(variant_is(&registry, &target, "Active").expect("variant is"));
        let ReflectValue::Host(HostValue::Array(variants)) =
            variants(&registry, &target).expect("variants")
        else {
            panic!("variants should be an array");
        };
        assert_eq!(variants.len(), 2);
        assert!(has_variant(&registry, &target, "Active").expect("has active"));
        assert!(!has_variant(&registry, &target, "Paused").expect("has paused"));
        let HostValue::Record {
            fields: variant_fields,
            ..
        } = &variants[0]
        else {
            panic!("variant metadata should be a record");
        };
        assert_eq!(
            variant_fields.get("source_span"),
            Some(&span_value(Some(Span::new(SourceId::new(8), 90, 100))))
        );
        let ReflectValue::Host(HostValue::Record {
            fields: single_variant,
            ..
        }) = variant_info(&registry, &target, "Active").expect("variant info")
        else {
            panic!("single variant metadata should be a record");
        };
        assert_eq!(
            single_variant.get("name"),
            Some(&HostValue::String("Active".to_owned()))
        );
        assert_eq!(
            single_variant.get("source_span"),
            Some(&span_value(Some(Span::new(SourceId::new(8), 90, 100))))
        );
        let ReflectValue::Host(HostValue::Array(all_variants)) = all_variants(&registry) else {
            panic!("variant list should be an array");
        };
        assert_eq!(all_variants.len(), 2);
        let HostValue::Record {
            fields: variant_list_item,
            ..
        } = &all_variants[0]
        else {
            panic!("variant list item should be a record");
        };
        assert_eq!(
            variant_list_item.get("owner"),
            Some(&HostValue::String("QuestProgress".to_owned()))
        );
        assert_eq!(
            variant_list_item.get("name"),
            Some(&HostValue::String("Active".to_owned()))
        );
    }

    #[test]
    fn unknown_variants_include_candidate_hints() {
        let registry = registry();
        let target = ReflectValue::ScriptEnum {
            enum_name: "QuestProgress".to_owned(),
            variant: "Active".to_owned(),
            fields: BTreeMap::new(),
        };

        let error =
            variant_is(&registry, &target, "Actve").expect_err("unknown variant should diagnose");

        assert_eq!(
            error.kind,
            ReflectErrorKind::UnknownVariant {
                type_name: "QuestProgress".to_owned(),
                variant: "Actve".to_owned(),
                candidates: vec!["Active".to_owned(), "Finished".to_owned()],
                related: vec![
                    crate::ReflectCandidate::new(
                        "Active",
                        Some(Span::new(SourceId::new(8), 90, 100))
                    ),
                    crate::ReflectCandidate::new("Finished", None),
                ],
            }
        );
        let error = variant_info(&registry, &target, "Actve").expect_err("unknown variant info");
        assert_eq!(
            error.kind,
            ReflectErrorKind::UnknownVariant {
                type_name: "QuestProgress".to_owned(),
                variant: "Actve".to_owned(),
                candidates: vec!["Active".to_owned(), "Finished".to_owned()],
                related: vec![
                    crate::ReflectCandidate::new(
                        "Active",
                        Some(Span::new(SourceId::new(8), 90, 100))
                    ),
                    crate::ReflectCandidate::new("Finished", None),
                ],
            }
        );
    }

    #[test]
    fn trait_query_returns_metadata_and_unknown_trait_candidates() {
        let mut registry = registry();
        registry.register_trait(TraitDesc::new("Trackable").source_span(Span::new(
            SourceId::new(9),
            10,
            30,
        )));

        let ReflectValue::Host(HostValue::Array(traits)) = all_traits(&registry) else {
            panic!("trait list should be an array");
        };
        assert_eq!(traits.len(), 2);
        let HostValue::Record {
            fields: first_trait,
            ..
        } = &traits[0]
        else {
            panic!("trait list item should be a record");
        };
        assert_eq!(
            first_trait.get("name"),
            Some(&HostValue::String("Damageable".to_owned()))
        );

        let ReflectValue::Host(HostValue::Record { fields, .. }) =
            trait_by_name(&registry, "Damageable").expect("trait metadata")
        else {
            panic!("trait metadata should be a record");
        };
        assert_eq!(
            fields.get("name"),
            Some(&HostValue::String("Damageable".to_owned()))
        );
        assert_eq!(
            fields.get("source_span"),
            Some(&span_value(Some(Span::new(SourceId::new(8), 20, 40))))
        );
        assert!(matches!(
            fields.get("methods"),
            Some(HostValue::Array(methods)) if methods.len() == 1
        ));
        let Some(HostValue::Array(methods)) = fields.get("methods") else {
            panic!("trait methods should be an array");
        };
        let HostValue::Record {
            fields: method_fields,
            ..
        } = &methods[0]
        else {
            panic!("trait method should be a record");
        };
        assert_eq!(
            method_fields.get("return"),
            Some(&HostValue::String("int".to_owned()))
        );
        assert_eq!(
            method_fields.get("returns"),
            Some(&HostValue::String("int".to_owned()))
        );
        let Some(HostValue::Array(params)) = method_fields.get("params") else {
            panic!("trait method params should be an array");
        };
        assert_eq!(params.len(), 1);
        assert!(has_trait(&registry, "Damageable"));
        assert!(has_trait(&registry, "Trackable"));
        assert!(!has_trait(&registry, "Damagable"));

        let error = trait_by_name(&registry, "Damagable").expect_err("unknown trait");
        assert_eq!(
            error.kind,
            ReflectErrorKind::UnknownTrait {
                trait_name: "Damagable".to_owned(),
                candidates: vec!["Damageable".to_owned(), "Trackable".to_owned()],
                related: vec![
                    crate::ReflectCandidate::new(
                        "Damageable",
                        Some(Span::new(SourceId::new(8), 20, 40))
                    ),
                    crate::ReflectCandidate::new(
                        "Trackable",
                        Some(Span::new(SourceId::new(9), 10, 30))
                    ),
                ],
            }
        );
    }

    #[test]
    fn methods_with_policy_hide_inaccessible_methods() {
        let mut registry = TypeRegistry::new();
        registry.register(
            TypeDesc::new(TypeKey::new(TypeId::new(500), "Player"))
                .host_type(HostTypeId::new(5))
                .method(MethodDesc::new(HostMethodId::new(1), "visible"))
                .method(
                    MethodDesc::new(HostMethodId::new(2), "hidden")
                        .access(crate::MethodAccess::new().reflect_callable(false)),
                )
                .method(
                    MethodDesc::new(HostMethodId::new(3), "private").access(
                        crate::MethodAccess::new()
                            .public(false)
                            .reflect_callable(true),
                    ),
                )
                .method(
                    MethodDesc::new(HostMethodId::new(4), "admin")
                        .access(crate::MethodAccess::new().require_permission("player.admin")),
                ),
        );
        let target =
            ReflectValue::HostRef(HostRef::new(HostTypeId::new(5), HostObjectId::new(1), 1));

        let ReflectValue::Host(HostValue::Array(raw_methods)) =
            methods(&registry, &target).expect("raw methods")
        else {
            panic!("methods should be an array");
        };
        assert_eq!(raw_methods.len(), 4);
        assert!(has_method(&registry, &target, "private").expect("raw has private"));

        let ReflectValue::Host(HostValue::Array(policy_methods)) =
            methods_with_policy(&registry, &target, &ReflectPolicy::read_only())
                .expect("policy methods")
        else {
            panic!("methods should be an array");
        };
        let ReflectValue::Host(HostValue::Array(policy_all_methods)) =
            all_methods_with_policy(&registry, &ReflectPolicy::read_only())
        else {
            panic!("all methods should be an array");
        };
        assert_eq!(policy_methods.len(), 1);
        assert_eq!(policy_all_methods.len(), 1);
        let HostValue::Record { fields, .. } = &policy_methods[0] else {
            panic!("method metadata should be a record");
        };
        assert_eq!(
            fields.get("name"),
            Some(&HostValue::String("visible".to_owned()))
        );
        let ReflectValue::Host(HostValue::Record {
            fields: method_fields,
            ..
        }) = method_with_policy(&registry, &target, "visible", &ReflectPolicy::read_only())
            .expect("visible method")
        else {
            panic!("method metadata should be a record");
        };
        assert_eq!(
            method_fields.get("name"),
            Some(&HostValue::String("visible".to_owned()))
        );
        let HostValue::Record {
            fields: all_method_fields,
            ..
        } = &policy_all_methods[0]
        else {
            panic!("all method metadata should be a record");
        };
        assert_eq!(
            all_method_fields.get("owner"),
            Some(&HostValue::String("Player".to_owned()))
        );
        assert_eq!(
            all_method_fields.get("name"),
            Some(&HostValue::String("visible".to_owned()))
        );
        assert!(
            has_method_with_policy(&registry, &target, "visible", &ReflectPolicy::read_only())
                .expect("has visible")
        );
        assert!(
            !has_method_with_policy(&registry, &target, "private", &ReflectPolicy::read_only())
                .expect("has private")
        );
        assert!(
            !has_method_with_policy(&registry, &target, "admin", &ReflectPolicy::read_only())
                .expect("has admin")
        );
        assert!(
            method_with_policy(&registry, &target, "admin", &ReflectPolicy::read_only()).is_err()
        );

        let admin_policy = ReflectPolicy::new(
            crate::ReflectPermissionSet::read_only().with(crate::ReflectPermission::AccessPrivate),
        )
        .with_method_permission("player.admin");
        let ReflectValue::Host(HostValue::Array(admin_methods)) =
            methods_with_policy(&registry, &target, &admin_policy).expect("admin methods")
        else {
            panic!("methods should be an array");
        };
        assert_eq!(admin_methods.len(), 3);
        assert!(
            has_method_with_policy(&registry, &target, "private", &admin_policy)
                .expect("has private")
        );
        assert!(
            has_method_with_policy(&registry, &target, "admin", &admin_policy).expect("has admin")
        );
        let ReflectValue::Host(HostValue::Record {
            fields: admin_fields,
            ..
        }) = method_with_policy(&registry, &target, "admin", &admin_policy).expect("admin method")
        else {
            panic!("admin method metadata should be a record");
        };
        assert_eq!(
            admin_fields.get("name"),
            Some(&HostValue::String("admin".to_owned()))
        );
    }

    #[test]
    fn fields_with_policy_hide_non_reflect_readable_fields() {
        let mut registry = TypeRegistry::new();
        registry.register(
            TypeDesc::new(TypeKey::new(TypeId::new(600), "Player"))
                .host_type(HostTypeId::new(6))
                .field(FieldDesc::new(FieldId::new(1), "level"))
                .field(
                    FieldDesc::new(FieldId::new(2), "secret")
                        .access(crate::FieldAccess::new().reflect_readable(false)),
                ),
        );
        let target =
            ReflectValue::HostRef(HostRef::new(HostTypeId::new(6), HostObjectId::new(1), 1));

        assert!(has_field(&registry, &target, "secret").expect("raw has field"));
        let ReflectValue::Host(HostValue::Array(fields)) =
            field_names_with_policy(&registry, &target, &ReflectPolicy::read_only())
                .expect("field names")
        else {
            panic!("fields should be an array");
        };
        assert_eq!(fields, vec![HostValue::String("level".to_owned())]);
        let ReflectValue::Host(HostValue::Array(all_fields)) =
            all_fields_with_policy(&registry, &ReflectPolicy::read_only())
        else {
            panic!("all fields should be an array");
        };
        assert_eq!(all_fields.len(), 1);
        let HostValue::Record {
            fields: field_list_item,
            ..
        } = &all_fields[0]
        else {
            panic!("field list item should be a record");
        };
        assert_eq!(
            field_list_item.get("owner"),
            Some(&HostValue::String("Player".to_owned()))
        );
        assert_eq!(
            field_list_item.get("name"),
            Some(&HostValue::String("level".to_owned()))
        );
        assert!(
            has_field_with_policy(&registry, &target, "level", &ReflectPolicy::read_only())
                .expect("has level")
        );
        assert!(
            !has_field_with_policy(&registry, &target, "secret", &ReflectPolicy::read_only())
                .expect("has secret")
        );

        let error = field_with_policy(&registry, &target, "secret", &ReflectPolicy::read_only())
            .expect_err("hidden field metadata");
        assert_eq!(
            error.kind,
            ReflectErrorKind::FieldNotReflectReadable {
                type_name: "Player".to_owned(),
                field: "secret".to_owned()
            }
        );

        let ReflectValue::Host(HostValue::Record { fields, .. }) =
            field(&registry, &target, "secret").expect("raw field metadata")
        else {
            panic!("field metadata should be a record");
        };
        assert_eq!(
            fields.get("name"),
            Some(&HostValue::String("secret".to_owned()))
        );
    }

    #[test]
    fn fields_with_policy_require_field_permissions() {
        let mut registry = TypeRegistry::new();
        registry.register(
            TypeDesc::new(TypeKey::new(TypeId::new(601), "Player"))
                .host_type(HostTypeId::new(6))
                .field(FieldDesc::new(FieldId::new(1), "level"))
                .field(
                    FieldDesc::new(FieldId::new(2), "title").access(
                        crate::FieldAccess::new().require_permission("player.title.inspect"),
                    ),
                ),
        );
        let target =
            ReflectValue::HostRef(HostRef::new(HostTypeId::new(6), HostObjectId::new(1), 1));

        let ReflectValue::Host(HostValue::Array(fields)) =
            field_names_with_policy(&registry, &target, &ReflectPolicy::read_only())
                .expect("field names")
        else {
            panic!("fields should be an array");
        };
        assert_eq!(fields, vec![HostValue::String("level".to_owned())]);
        assert!(
            !has_field_with_policy(&registry, &target, "title", &ReflectPolicy::read_only())
                .expect("has title")
        );

        let error = field_with_policy(&registry, &target, "title", &ReflectPolicy::read_only())
            .expect_err("field permission");
        assert_eq!(
            error.kind,
            ReflectErrorKind::FieldPermissionDenied {
                type_name: "Player".to_owned(),
                field: "title".to_owned(),
                permission: "player.title.inspect".to_owned(),
            }
        );

        let policy = ReflectPolicy::read_only().with_field_permission("player.title.inspect");
        let ReflectValue::Host(HostValue::Record { fields, .. }) =
            field_with_policy(&registry, &target, "title", &policy).expect("allowed field")
        else {
            panic!("field metadata should be a record");
        };
        assert_eq!(
            fields.get("name"),
            Some(&HostValue::String("title".to_owned()))
        );
        let Some(HostValue::Record { fields: access, .. }) = fields.get("access") else {
            panic!("field access should be a record");
        };
        assert_eq!(
            access.get("required_permissions"),
            Some(&HostValue::Array(vec![HostValue::String(
                "player.title.inspect".to_owned()
            )]))
        );
    }

    #[test]
    fn variants_with_policy_hide_non_reflect_readable_fields() {
        let mut registry = TypeRegistry::new();
        registry.register(
            TypeDesc::new(TypeKey::new(TypeId::new(700), "QuestProgress"))
                .kind(TypeKind::ScriptEnum)
                .variant(
                    VariantDesc::new(VariantId::new(1), "Active")
                        .field(FieldDesc::new(FieldId::new(1), "count"))
                        .field(
                            FieldDesc::new(FieldId::new(2), "secret")
                                .access(crate::FieldAccess::new().reflect_readable(false)),
                        ),
                ),
        );
        let target = ReflectValue::ScriptEnum {
            enum_name: "QuestProgress".to_owned(),
            variant: "Active".to_owned(),
            fields: BTreeMap::new(),
        };

        let ReflectValue::Host(HostValue::Array(raw_variants)) =
            variants(&registry, &target).expect("raw variants")
        else {
            panic!("variants should be an array");
        };
        let HostValue::Record { fields, .. } = &raw_variants[0] else {
            panic!("variant metadata should be a record");
        };
        assert!(matches!(
            fields.get("fields"),
            Some(HostValue::Array(raw_fields)) if raw_fields.len() == 2
        ));

        let ReflectValue::Host(HostValue::Array(policy_variants)) =
            variants_with_policy(&registry, &target, &ReflectPolicy::read_only())
                .expect("policy variants")
        else {
            panic!("variants should be an array");
        };
        let ReflectValue::Host(HostValue::Record {
            fields: policy_variant,
            ..
        }) = variant_info_with_policy(&registry, &target, "Active", &ReflectPolicy::read_only())
            .expect("policy variant info")
        else {
            panic!("variant info should be a record");
        };
        let ReflectValue::Host(HostValue::Array(policy_all_variants)) =
            all_variants_with_policy(&registry, &ReflectPolicy::read_only())
        else {
            panic!("all variants should be an array");
        };
        let HostValue::Record { fields, .. } = &policy_variants[0] else {
            panic!("variant metadata should be a record");
        };
        let Some(HostValue::Array(policy_fields)) = fields.get("fields") else {
            panic!("variant fields should be an array");
        };
        assert_eq!(policy_fields.len(), 1);
        let HostValue::Record { fields, .. } = &policy_fields[0] else {
            panic!("field metadata should be a record");
        };
        assert_eq!(
            fields.get("name"),
            Some(&HostValue::String("count".to_owned()))
        );
        let HostValue::Record {
            fields: all_variant_fields,
            ..
        } = &policy_all_variants[0]
        else {
            panic!("variant metadata should be a record");
        };
        assert_eq!(
            all_variant_fields.get("owner"),
            Some(&HostValue::String("QuestProgress".to_owned()))
        );
        let Some(HostValue::Array(all_policy_fields)) = all_variant_fields.get("fields") else {
            panic!("variant fields should be an array");
        };
        assert_eq!(all_policy_fields.len(), 1);
        let Some(HostValue::Array(policy_variant_fields)) = policy_variant.get("fields") else {
            panic!("variant info fields should be an array");
        };
        assert_eq!(policy_variant_fields.len(), 1);
    }
}
