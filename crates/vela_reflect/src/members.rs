use std::collections::BTreeMap;

use vela_host::HostValue;

use crate::{
    FieldDesc, MethodDesc, ReflectError, ReflectErrorKind, ReflectResult, ReflectValue, TraitDesc,
    TraitMethodDesc, TypeDesc, TypeKind, TypeRegistry, VariantDesc, name_candidates, type_of,
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

pub fn variant_is(target: &ReflectValue, name: &str) -> ReflectResult<bool> {
    Ok(variant_name(target)? == name)
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
    HostValue::Record {
        type_name: "ReflectMethod".to_owned(),
        fields,
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
    HostValue::Record {
        type_name: "ReflectField".to_owned(),
        fields,
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
                .field(FieldDesc::new(FieldId::new(1), "id"))
                .field(FieldDesc::new(FieldId::new(2), "level").writable(true))
                .method(MethodDesc::new(HostMethodId::new(5), "grant_exp"))
                .trait_impl(
                    TraitDesc::new("Damageable")
                        .method(TraitMethodDesc::new(MethodId::new(9), "damage").defaulted(true)),
                ),
        );
        registry.register(
            TypeDesc::new(TypeKey::new(TypeId::new(200), "QuestProgress"))
                .kind(TypeKind::ScriptEnum)
                .variant(
                    VariantDesc::new(VariantId::new(11), "Active")
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
        assert!(has_field(&registry, &target, "level").expect("has field"));

        let ReflectValue::Host(HostValue::Record { fields, .. }) =
            field(&registry, &target, "level").expect("field")
        else {
            panic!("field metadata should be a record");
        };
        assert_eq!(fields.get("writable"), Some(&HostValue::Bool(true)));

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
        assert!(variant_is(&target, "Active").expect("variant is"));
        let ReflectValue::Host(HostValue::Array(variants)) =
            variants(&registry, &target).expect("variants")
        else {
            panic!("variants should be an array");
        };
        assert_eq!(variants.len(), 2);
    }
}
