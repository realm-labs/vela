use vela_host::HostValue;

mod fields;
mod methods;
mod traits;
mod variants;

use crate::{
    MethodDesc, ReflectError, ReflectErrorKind, ReflectResult, ReflectValue, TypeDesc, TypeKind,
    TypeRegistry,
    candidates::{candidate_names, ranked_candidates},
    descriptor_targets,
    metadata::{attrs_value, docs_value, span_value},
    metadata_records, type_of,
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

fn method_candidates(desc: &TypeDesc, method: &str) -> Vec<crate::ReflectCandidate> {
    ranked_candidates(
        method,
        desc.methods
            .iter()
            .map(|method| (method.name.as_str(), method.source_span)),
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use vela_common::{
        FieldId, HostMethodId, HostObjectId, HostTypeId, MethodId, SourceId, Span, TypeId,
        VariantId,
    };
    use vela_host::HostRef;

    use crate::{
        FieldDesc, MethodDesc, MethodParamDesc, ReflectPolicy, TraitDesc, TraitMethodDesc, TypeKey,
        TypeKind, VariantDesc,
    };

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
        let player_type = crate::type_metadata_by_name(&registry, "Player").expect("type info");
        let field_from_type = field(&registry, &player_type, "level").expect("type field");
        assert!(has_field(&registry, &player_type, "level").expect("type has field"));
        let ReflectValue::Host(HostValue::Record { fields, .. }) = &field_metadata else {
            panic!("field metadata should be a record");
        };
        assert_eq!(field_metadata, field_from_type);
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
            fields.get("origin"),
            Some(&HostValue::String("host".to_owned()))
        );
        assert_eq!(
            origin(&registry, &field_metadata).expect("field origin metadata"),
            ReflectValue::Host(HostValue::String("host".to_owned()))
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
        let ReflectValue::Host(HostValue::Array(method_records)) =
            methods(&registry, &ReflectValue::HostRef(player_ref())).expect("methods")
        else {
            panic!("methods should be an array");
        };
        let player_type = crate::type_metadata_by_name(&registry, "Player").expect("type info");
        let ReflectValue::Host(HostValue::Array(type_methods)) =
            methods(&registry, &player_type).expect("type methods")
        else {
            panic!("type methods should be an array");
        };
        assert_eq!(method_records.len(), 1);
        assert_eq!(type_methods, method_records);
        assert!(has_method(&registry, &player_type, "grant_exp").expect("type has method"));
        let HostValue::Record { fields, .. } = &method_records[0] else {
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
        let Some(HostValue::Array(raw_params)) = fields.get("params") else {
            panic!("method params should be an array");
        };
        assert_eq!(raw_params.len(), 1);
        let HostValue::Record {
            fields: param_fields,
            ..
        } = &raw_params[0]
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
        let single_method_value =
            method(&registry, &ReflectValue::HostRef(player_ref()), "grant_exp")
                .expect("method metadata");
        assert_eq!(
            method(&registry, &player_type, "grant_exp").expect("type method"),
            single_method_value
        );
        let ReflectValue::Host(HostValue::Record {
            fields: single_method,
            ..
        }) = &single_method_value
        else {
            panic!("single method metadata should be a record");
        };
        assert_eq!(
            single_method.get("name"),
            Some(&HostValue::String("grant_exp".to_owned()))
        );
        assert_eq!(
            single_method.get("origin"),
            Some(&HostValue::String("host".to_owned()))
        );
        assert_eq!(
            origin(&registry, &single_method_value).expect("method origin metadata"),
            ReflectValue::Host(HostValue::String("host".to_owned()))
        );
        assert_eq!(
            owner(&registry, &single_method_value).expect("method owner metadata"),
            ReflectValue::Host(HostValue::String("Player".to_owned()))
        );
        assert_eq!(
            single_method.get("attrs"),
            Some(&HostValue::Map(BTreeMap::from([(
                "effect".to_owned(),
                HostValue::String("write".to_owned())
            )])))
        );
        let ReflectValue::Host(HostValue::Record {
            fields: helper_effects,
            ..
        }) = effects(&registry, &single_method_value).expect("method effects metadata")
        else {
            panic!("method effects metadata should be a record");
        };
        assert_eq!(
            helper_effects.get("reads_host"),
            Some(&HostValue::Bool(true))
        );
        assert_eq!(
            helper_effects.get("writes_host"),
            Some(&HostValue::Bool(true))
        );
        let nested_effects = single_method
            .get("effects")
            .expect("method effects record")
            .clone();
        assert_eq!(
            effects(&registry, &ReflectValue::Host(nested_effects))
                .expect("nested effects metadata"),
            ReflectValue::Host(HostValue::Record {
                type_name: "ReflectEffectSet".to_owned(),
                fields: helper_effects,
            })
        );
        let ReflectValue::Host(HostValue::Array(helper_params)) =
            params(&registry, &single_method_value).expect("method params metadata")
        else {
            panic!("method params metadata should be an array");
        };
        assert_eq!(helper_params.len(), 1);
        let HostValue::Record {
            fields: param_fields,
            ..
        } = &helper_params[0]
        else {
            panic!("method param should be a record");
        };
        assert_eq!(
            param_fields.get("name"),
            Some(&HostValue::String("amount".to_owned()))
        );
        assert_eq!(
            params(
                &registry,
                &ReflectValue::Host(
                    single_method
                        .get("params")
                        .expect("method params record")
                        .clone()
                )
            )
            .expect("nested params metadata"),
            ReflectValue::Host(HostValue::Array(helper_params))
        );
        assert_eq!(
            returns(&registry, &single_method_value).expect("method returns metadata"),
            ReflectValue::Host(HostValue::String("bool".to_owned()))
        );
        let ReflectValue::Host(HostValue::Record {
            fields: helper_access,
            ..
        }) = access(&registry, &single_method_value).expect("method access metadata")
        else {
            panic!("method access metadata should be a record");
        };
        assert_eq!(
            helper_access.get("reflect_callable"),
            Some(&HostValue::Bool(true))
        );
        assert_eq!(
            access(
                &registry,
                &ReflectValue::Host(
                    single_method
                        .get("access")
                        .expect("method access record")
                        .clone()
                )
            )
            .expect("nested access metadata"),
            ReflectValue::Host(HostValue::Record {
                type_name: "ReflectMethodAccess".to_owned(),
                fields: helper_access,
            })
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

        let ReflectValue::Host(HostValue::Array(trait_records)) =
            traits(&registry, &ReflectValue::HostRef(player_ref())).expect("traits")
        else {
            panic!("traits should be an array");
        };
        assert_eq!(
            traits(&registry, &player_type).expect("type traits"),
            ReflectValue::Host(HostValue::Array(trait_records.clone()))
        );
        assert_eq!(trait_records.len(), 1);
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
        let ReflectValue::Host(HostValue::Array(variant_records)) =
            variants(&registry, &target).expect("variants")
        else {
            panic!("variants should be an array");
        };
        let quest_type =
            crate::type_metadata_by_name(&registry, "QuestProgress").expect("type info");
        assert_eq!(
            variants(&registry, &quest_type).expect("type variants"),
            ReflectValue::Host(HostValue::Array(variant_records.clone()))
        );
        assert_eq!(variant_records.len(), 2);
        assert!(has_variant(&registry, &target, "Active").expect("has active"));
        assert!(has_variant(&registry, &quest_type, "Active").expect("type has active"));
        assert!(!has_variant(&registry, &target, "Paused").expect("has paused"));
        assert!(has_field(&registry, &target, "count").expect("has active field"));
        assert!(!has_field(&registry, &target, "missing").expect("missing active field"));
        let ReflectValue::Host(HostValue::Array(active_fields)) =
            fields_with_policy(&registry, &target, &ReflectPolicy::read_only())
                .expect("active variant fields")
        else {
            panic!("active variant fields should be an array");
        };
        assert_eq!(active_fields.len(), 1);
        let active_field = field(&registry, &target, "count").expect("active variant field");
        assert_eq!(
            active_fields[0],
            match active_field {
                ReflectValue::Host(value) => value,
                _ => panic!("active variant field should be host metadata"),
            }
        );
        let error = field(&registry, &target, "cout").expect_err("unknown active variant field");
        assert_eq!(
            error.kind,
            ReflectErrorKind::UnknownField {
                type_name: "QuestProgress.Active".to_owned(),
                field: "cout".to_owned(),
                candidates: vec!["count".to_owned()],
                related: vec![crate::ReflectCandidate::new("count", None)],
            }
        );
        let HostValue::Record {
            fields: variant_fields,
            ..
        } = &variant_records[0]
        else {
            panic!("variant metadata should be a record");
        };
        assert_eq!(
            variant_fields.get("source_span"),
            Some(&span_value(Some(Span::new(SourceId::new(8), 90, 100))))
        );
        let single_variant_value =
            variant_info(&registry, &target, "Active").expect("variant info");
        assert_eq!(
            variant_info(&registry, &quest_type, "Active").expect("type variant info"),
            single_variant_value
        );
        let ReflectValue::Host(HostValue::Record {
            fields: single_variant,
            ..
        }) = &single_variant_value
        else {
            panic!("single variant metadata should be a record");
        };
        assert_eq!(
            single_variant.get("name"),
            Some(&HostValue::String("Active".to_owned()))
        );
        assert_eq!(
            single_variant.get("origin"),
            Some(&HostValue::String("host".to_owned()))
        );
        assert_eq!(
            origin(&registry, &single_variant_value).expect("variant origin metadata"),
            ReflectValue::Host(HostValue::String("host".to_owned()))
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
        let first_trait_value = ReflectValue::Host(traits[0].clone());
        assert_eq!(
            origin(&registry, &first_trait_value).expect("trait origin metadata"),
            ReflectValue::Host(HostValue::String("host".to_owned()))
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
            fields.get("origin"),
            Some(&HostValue::String("host".to_owned()))
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
        let trait_method_value = ReflectValue::Host(methods[0].clone());
        assert_eq!(
            method_fields.get("owner"),
            Some(&HostValue::String("Damageable".to_owned()))
        );
        assert_eq!(
            owner(&registry, &trait_method_value).expect("trait method owner metadata"),
            ReflectValue::Host(HostValue::String("Damageable".to_owned()))
        );
        assert_eq!(
            kind(&registry, &trait_method_value).expect("trait method kind metadata"),
            ReflectValue::Host(HostValue::String("trait_method".to_owned()))
        );
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
            fields_with_policy(&registry, &target, &ReflectPolicy::read_only())
                .expect("field metadata")
        else {
            panic!("fields should be an array");
        };
        assert_eq!(fields.len(), 1);
        let HostValue::Record {
            fields: field_fields,
            ..
        } = &fields[0]
        else {
            panic!("field metadata should be a record");
        };
        assert_eq!(
            field_fields.get("owner"),
            Some(&HostValue::String("Player".to_owned()))
        );
        assert_eq!(
            field_fields.get("name"),
            Some(&HostValue::String("level".to_owned()))
        );
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
            fields_with_policy(&registry, &target, &ReflectPolicy::read_only())
                .expect("field metadata")
        else {
            panic!("fields should be an array");
        };
        assert_eq!(fields.len(), 1);
        let HostValue::Record {
            fields: field_fields,
            ..
        } = &fields[0]
        else {
            panic!("field metadata should be a record");
        };
        assert_eq!(
            field_fields.get("name"),
            Some(&HostValue::String("level".to_owned()))
        );
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
        assert!(
            has_field_with_policy(&registry, &target, "count", &ReflectPolicy::read_only())
                .expect("has visible active variant field")
        );
        assert!(
            !has_field_with_policy(&registry, &target, "secret", &ReflectPolicy::read_only())
                .expect("has hidden active variant field")
        );

        let error = field_with_policy(&registry, &target, "secret", &ReflectPolicy::read_only())
            .expect_err("hidden active variant field");
        assert_eq!(
            error.kind,
            ReflectErrorKind::FieldNotReflectReadable {
                type_name: "QuestProgress.Active".to_owned(),
                field: "secret".to_owned(),
            }
        );
    }
}
