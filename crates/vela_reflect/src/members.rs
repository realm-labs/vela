use std::collections::BTreeMap;

use vela_host::HostValue;

use crate::{
    FieldDesc, MethodDesc, ReflectError, ReflectErrorKind, ReflectResult, ReflectValue, TraitDesc,
    TraitMethodDesc, TypeDesc, TypeKind, TypeRegistry, VariantDesc,
    metadata::{attrs_value, docs_value},
    name_candidates, type_of,
};

pub fn name(registry: &TypeRegistry, target: &ReflectValue) -> ReflectResult<ReflectValue> {
    let desc = target_type(registry, target)?;
    Ok(ReflectValue::Host(HostValue::String(desc.key.name.clone())))
}

pub fn kind(registry: &TypeRegistry, target: &ReflectValue) -> ReflectResult<ReflectValue> {
    let desc = target_type(registry, target)?;
    Ok(ReflectValue::Host(HostValue::String(
        match desc.kind {
            TypeKind::Host => "host",
            TypeKind::ScriptStruct => "script_struct",
            TypeKind::ScriptEnum => "script_enum",
        }
        .to_owned(),
    )))
}

pub fn attrs(registry: &TypeRegistry, target: &ReflectValue) -> ReflectResult<ReflectValue> {
    let desc = target_type(registry, target)?;
    Ok(ReflectValue::Host(attrs_value(&desc.attrs)))
}

pub fn docs(registry: &TypeRegistry, target: &ReflectValue) -> ReflectResult<ReflectValue> {
    let desc = target_type(registry, target)?;
    Ok(ReflectValue::Host(docs_value(desc.docs.as_deref())))
}

pub fn field(
    registry: &TypeRegistry,
    target: &ReflectValue,
    name: &str,
) -> ReflectResult<ReflectValue> {
    let desc = target_type(registry, target)?;
    let field = desc
        .fields
        .iter()
        .find(|field| field.name == name)
        .ok_or_else(|| {
            ReflectError::new(ReflectErrorKind::UnknownField {
                type_name: desc.key.name.clone(),
                field: name.to_owned(),
                candidates: name_candidates(
                    name,
                    desc.fields.iter().map(|field| field.name.as_str()),
                ),
            })
        })?;
    Ok(ReflectValue::Host(field_record(field)))
}

pub fn has_field(
    registry: &TypeRegistry,
    target: &ReflectValue,
    name: &str,
) -> ReflectResult<bool> {
    let desc = target_type(registry, target)?;
    Ok(desc.fields.iter().any(|field| field.name == name))
}

pub fn methods(registry: &TypeRegistry, target: &ReflectValue) -> ReflectResult<ReflectValue> {
    let desc = target_type(registry, target)?;
    Ok(ReflectValue::Host(HostValue::Array(
        desc.methods.iter().map(method_record).collect(),
    )))
}

pub fn has_method(
    registry: &TypeRegistry,
    target: &ReflectValue,
    name: &str,
) -> ReflectResult<bool> {
    let desc = target_type(registry, target)?;
    Ok(desc.methods.iter().any(|method| method.name == name))
}

pub fn traits(registry: &TypeRegistry, target: &ReflectValue) -> ReflectResult<ReflectValue> {
    let desc = target_type(registry, target)?;
    Ok(ReflectValue::Host(HostValue::Array(
        desc.traits.iter().map(trait_record).collect(),
    )))
}

pub fn variants(registry: &TypeRegistry, target: &ReflectValue) -> ReflectResult<ReflectValue> {
    let desc = target_type(registry, target)?;
    Ok(ReflectValue::Host(HostValue::Array(
        desc.variants.iter().map(variant_record).collect(),
    )))
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
    Err(ReflectError::new(ReflectErrorKind::UnknownVariant {
        type_name: desc.key.name.clone(),
        variant: name.to_owned(),
        candidates: name_candidates(
            name,
            desc.variants.iter().map(|variant| variant.name.as_str()),
        ),
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

fn method_record(method: &MethodDesc) -> HostValue {
    let mut fields = BTreeMap::new();
    fields.insert("id".to_owned(), HostValue::Int(i64::from(method.id.get())));
    fields.insert("name".to_owned(), HostValue::String(method.name.clone()));
    fields.insert("effects".to_owned(), method_effects_record(method));
    fields.insert("access".to_owned(), method_access_record(method));
    fields.insert("docs".to_owned(), docs_value(method.docs.as_deref()));
    fields.insert("attrs".to_owned(), attrs_value(&method.attrs));
    HostValue::Record {
        type_name: "ReflectMethod".to_owned(),
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
    HostValue::Record {
        type_name: "ReflectTrait".to_owned(),
        fields,
    }
}

fn trait_method_record(method: &TraitMethodDesc) -> HostValue {
    let mut fields = BTreeMap::new();
    fields.insert("id".to_owned(), HostValue::Int(i64::from(method.id.get())));
    fields.insert("name".to_owned(), HostValue::String(method.name.clone()));
    fields.insert("defaulted".to_owned(), HostValue::Bool(method.has_default));
    fields.insert("docs".to_owned(), docs_value(method.docs.as_deref()));
    fields.insert("attrs".to_owned(), attrs_value(&method.attrs));
    HostValue::Record {
        type_name: "ReflectTraitMethod".to_owned(),
        fields,
    }
}

fn variant_record(variant: &VariantDesc) -> HostValue {
    let mut fields = BTreeMap::new();
    fields.insert("id".to_owned(), HostValue::Int(i64::from(variant.id.get())));
    fields.insert("name".to_owned(), HostValue::String(variant.name.clone()));
    fields.insert(
        "fields".to_owned(),
        HostValue::Array(variant.fields.iter().map(field_record).collect()),
    );
    fields.insert("docs".to_owned(), docs_value(variant.docs.as_deref()));
    fields.insert("attrs".to_owned(), attrs_value(&variant.attrs));
    HostValue::Record {
        type_name: "ReflectVariant".to_owned(),
        fields,
    }
}

fn field_record(field: &FieldDesc) -> HostValue {
    let mut fields = BTreeMap::new();
    fields.insert("id".to_owned(), HostValue::Int(i64::from(field.id.get())));
    fields.insert("name".to_owned(), HostValue::String(field.name.clone()));
    fields.insert("writable".to_owned(), HostValue::Bool(field.writable));
    fields.insert("access".to_owned(), field_access_record(field));
    fields.insert("docs".to_owned(), docs_value(field.docs.as_deref()));
    fields.insert("attrs".to_owned(), attrs_value(&field.attrs));
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
        ]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_common::{
        FieldId, HostMethodId, HostObjectId, HostTypeId, MethodId, TypeId, VariantId,
    };
    use vela_host::HostRef;

    use crate::{TraitMethodDesc, TypeKey, TypeKind};

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
                        .docs("Current level.")
                        .attr("unit", "level"),
                )
                .method(
                    MethodDesc::new(HostMethodId::new(5), "grant_exp")
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
                        .docs("Can take damage.")
                        .attr("combat", "true")
                        .method(
                            TraitMethodDesc::new(MethodId::new(9), "damage")
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

        let ReflectValue::Host(HostValue::Record { fields, .. }) =
            field(&registry, &target, "level").expect("field")
        else {
            panic!("field metadata should be a record");
        };
        assert_eq!(fields.get("writable"), Some(&HostValue::Bool(true)));
        assert_eq!(
            fields.get("access"),
            Some(&HostValue::Record {
                type_name: "ReflectFieldAccess".to_owned(),
                fields: BTreeMap::from([
                    ("readable".to_owned(), HostValue::Bool(true)),
                    ("writable".to_owned(), HostValue::Bool(true)),
                    ("reflect_readable".to_owned(), HostValue::Bool(true)),
                    ("reflect_writable".to_owned(), HostValue::Bool(true)),
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

        let error = field(&registry, &target, "levle").expect_err("unknown field");
        assert_eq!(
            error.kind,
            ReflectErrorKind::UnknownField {
                type_name: "Player".to_owned(),
                field: "levle".to_owned(),
                candidates: vec!["level".to_owned(), "id".to_owned()]
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

        let ReflectValue::Host(HostValue::Array(traits)) =
            traits(&registry, &ReflectValue::HostRef(player_ref())).expect("traits")
        else {
            panic!("traits should be an array");
        };
        assert_eq!(traits.len(), 1);

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
                candidates: vec!["Active".to_owned(), "Finished".to_owned()]
            }
        );
    }
}
