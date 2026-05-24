use std::collections::BTreeMap;

use vela_host::HostValue;

use crate::{
    ReflectError, ReflectErrorKind, ReflectResult, ReflectValue, TypeDesc, TypeKind, TypeRegistry,
    metadata::{attrs_value, docs_value},
    name_candidates,
};

pub fn type_names(registry: &TypeRegistry) -> ReflectValue {
    ReflectValue::Host(HostValue::Array(
        registry
            .types()
            .map(|desc| HostValue::String(desc.key.name.clone()))
            .collect(),
    ))
}

pub fn type_by_name(registry: &TypeRegistry, name: &str) -> ReflectResult<ReflectValue> {
    let desc = registry.type_by_name(name).ok_or_else(|| {
        ReflectError::new(ReflectErrorKind::UnknownTypeName {
            type_name: name.to_owned(),
            candidates: name_candidates(name, registry.types().map(|desc| desc.key.name.as_str())),
        })
    })?;
    Ok(ReflectValue::Host(type_record(desc)))
}

fn type_record(desc: &TypeDesc) -> HostValue {
    let mut fields = BTreeMap::new();
    fields.insert(
        "id".to_owned(),
        HostValue::Int(i64::from(desc.key.id.get())),
    );
    fields.insert("name".to_owned(), HostValue::String(desc.key.name.clone()));
    fields.insert("kind".to_owned(), HostValue::String(kind_name(desc.kind)));
    fields.insert(
        "schema_hash".to_owned(),
        desc.schema_hash.map_or(HostValue::Null, |hash| {
            HostValue::Int(i64::try_from(hash.get()).unwrap_or(i64::MAX))
        }),
    );
    fields.insert(
        "field_count".to_owned(),
        HostValue::Int(i64::try_from(desc.fields.len()).unwrap_or(i64::MAX)),
    );
    fields.insert(
        "method_count".to_owned(),
        HostValue::Int(i64::try_from(desc.methods.len()).unwrap_or(i64::MAX)),
    );
    fields.insert(
        "trait_count".to_owned(),
        HostValue::Int(i64::try_from(desc.traits.len()).unwrap_or(i64::MAX)),
    );
    fields.insert(
        "variant_count".to_owned(),
        HostValue::Int(i64::try_from(desc.variants.len()).unwrap_or(i64::MAX)),
    );
    fields.insert("docs".to_owned(), docs_value(desc.docs.as_deref()));
    fields.insert("attrs".to_owned(), attrs_value(&desc.attrs));
    HostValue::Record {
        type_name: "ReflectType".to_owned(),
        fields,
    }
}

fn kind_name(kind: TypeKind) -> String {
    match kind {
        TypeKind::Host => "host",
        TypeKind::ScriptStruct => "script_struct",
        TypeKind::ScriptEnum => "script_enum",
    }
    .to_owned()
}

#[cfg(test)]
mod tests {
    use vela_common::{FieldId, HostTypeId, TypeId, VariantId};

    use super::*;
    use crate::{FieldDesc, TypeDesc, TypeKey, VariantDesc};

    #[test]
    fn type_query_returns_metadata_and_unknown_type_candidates() {
        let mut registry = TypeRegistry::new();
        registry.register(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
                .host_type(HostTypeId::new(1))
                .docs("A host player.")
                .attr("domain", "gameplay")
                .field(FieldDesc::new(FieldId::new(1), "level")),
        );
        registry.register(
            TypeDesc::new(TypeKey::new(TypeId::new(2), "QuestProgress"))
                .variant(VariantDesc::new(VariantId::new(1), "Active")),
        );

        let ReflectValue::Host(HostValue::Array(names)) = type_names(&registry) else {
            panic!("type names should be an array");
        };
        assert_eq!(
            names,
            vec![
                HostValue::String("Player".to_owned()),
                HostValue::String("QuestProgress".to_owned())
            ]
        );

        let ReflectValue::Host(HostValue::Record { fields, .. }) =
            type_by_name(&registry, "Player").expect("type metadata")
        else {
            panic!("type metadata should be a record");
        };
        assert_eq!(
            fields.get("name"),
            Some(&HostValue::String("Player".to_owned()))
        );
        assert_eq!(
            fields.get("kind"),
            Some(&HostValue::String("host".to_owned()))
        );
        assert_eq!(fields.get("field_count"), Some(&HostValue::Int(1)));
        assert_eq!(
            fields.get("docs"),
            Some(&HostValue::String("A host player.".to_owned()))
        );

        let error = type_by_name(&registry, "Plyer").expect_err("unknown type");
        assert_eq!(
            error.kind,
            ReflectErrorKind::UnknownTypeName {
                type_name: "Plyer".to_owned(),
                candidates: vec!["Player".to_owned(), "QuestProgress".to_owned()]
            }
        );
    }
}
