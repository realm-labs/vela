use std::collections::BTreeMap;

use vela_host::value::HostValue;

use crate::{
    candidates::{candidate_names, ranked_candidates},
    error::{ReflectError, ReflectErrorKind, ReflectResult},
    metadata::{attrs_value, docs_value, span_value},
    modules::DeclOrigin,
    registry::{TypeDesc, TypeKind, TypeRegistry},
    value::ReflectValue,
};

pub fn type_list(registry: &TypeRegistry) -> ReflectValue {
    ReflectValue::Host(HostValue::Array(
        registry.types().map(type_record).collect(),
    ))
}

pub fn type_by_name(registry: &TypeRegistry, name: &str) -> ReflectResult<ReflectValue> {
    let desc = registry.type_by_name(name).ok_or_else(|| {
        let related = ranked_candidates(
            name,
            registry
                .types()
                .map(|desc| (desc.key.name.as_str(), desc.source_span)),
        );
        ReflectError::new(ReflectErrorKind::UnknownTypeName {
            type_name: name.to_owned(),
            candidates: candidate_names(&related),
            related,
        })
    })?;
    Ok(ReflectValue::Host(type_record(desc)))
}

pub fn type_of_value(registry: &TypeRegistry, target: &ReflectValue) -> ReflectValue {
    crate::value::type_of(registry, target)
        .map(type_record)
        .map_or(ReflectValue::Host(HostValue::Null), ReflectValue::Host)
}

pub fn has_type(registry: &TypeRegistry, name: &str) -> bool {
    registry.type_by_name(name).is_some()
}

fn type_record(desc: &TypeDesc) -> HostValue {
    let mut fields = BTreeMap::new();
    fields.insert(
        "id".to_owned(),
        // TODO(reflect): stable IDs are u64, but reflection currently exposes IDs
        // through signed script ints. Replace this lossy saturation with a deliberate
        // unsigned/ID value surface before treating reflect.id() as a stable public
        // identity API.
        HostValue::Int(i64::try_from(desc.key.id.get()).unwrap_or(i64::MAX)),
    );
    fields.insert("name".to_owned(), HostValue::String(desc.key.name.clone()));
    fields.insert("kind".to_owned(), HostValue::String(kind_name(desc.kind)));
    fields.insert("origin".to_owned(), origin_value(desc.origin));
    fields.insert(
        "schema_hash".to_owned(),
        desc.schema_hash.map_or(HostValue::Null, |hash| {
            // TODO(reflect): stable IDs are u64, but reflection currently exposes IDs
            // through signed script ints. Replace this lossy saturation with a deliberate
            // unsigned/ID value surface before treating reflect.id() as a stable public
            // identity API.
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
    fields.insert("source_span".to_owned(), span_value(desc.source_span));
    HostValue::Record {
        type_name: "ReflectType".to_owned(),
        fields,
    }
}

fn origin_value(origin: DeclOrigin) -> HostValue {
    HostValue::String(origin.as_str().to_owned())
}

fn kind_name(kind: TypeKind) -> String {
    match kind {
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
    .to_owned()
}

#[cfg(test)]
mod tests {
    use vela_common::{FieldId, HostObjectId, HostTypeId, SourceId, Span, TypeId, VariantId};
    use vela_host::path::HostRef;

    use super::*;
    use crate::members::{kind, origin};
    use crate::registry::{FieldDesc, TypeDesc, TypeKey, TypeRegistry, VariantDesc};

    #[test]
    fn type_query_returns_metadata_and_unknown_type_candidates() {
        let mut registry = TypeRegistry::new();
        registry.register(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
                .host_type(HostTypeId::new(1))
                .docs("A host player.")
                .attr("domain", "gameplay")
                .source_span(Span::new(SourceId::new(7), 10, 20))
                .field(FieldDesc::new(FieldId::new(1), "level")),
        );
        registry.register(
            TypeDesc::new(TypeKey::new(TypeId::new(2), "QuestProgress"))
                .variant(VariantDesc::new(VariantId::new(1), "Active")),
        );

        assert!(has_type(&registry, "Player"));
        assert!(has_type(&registry, "QuestProgress"));
        assert!(!has_type(&registry, "Monster"));

        let ReflectValue::Host(HostValue::Array(types)) = type_list(&registry) else {
            panic!("type list should be an array");
        };
        assert_eq!(types.len(), 2);
        let HostValue::Record { fields, .. } = &types[0] else {
            panic!("type list item should be a record");
        };
        assert_eq!(
            fields.get("name"),
            Some(&HostValue::String("Player".to_owned()))
        );
        let HostValue::Record { fields, .. } = &types[1] else {
            panic!("type list item should be a record");
        };
        assert_eq!(
            fields.get("name"),
            Some(&HostValue::String("QuestProgress".to_owned()))
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
        assert_eq!(
            fields.get("origin"),
            Some(&HostValue::String("host".to_owned()))
        );
        let metadata = type_by_name(&registry, "Player").expect("type metadata");
        let type_of_metadata = type_of_value(
            &registry,
            &ReflectValue::HostRef(HostRef::new(HostTypeId::new(1), HostObjectId::new(7), 0)),
        );
        assert_eq!(type_of_metadata, metadata);
        assert_eq!(
            kind(&registry, &metadata).expect("metadata kind"),
            ReflectValue::Host(HostValue::String("host".to_owned()))
        );
        assert_eq!(
            origin(&registry, &metadata).expect("metadata origin"),
            ReflectValue::Host(HostValue::String("host".to_owned()))
        );
        assert_eq!(fields.get("field_count"), Some(&HostValue::Int(1)));
        assert_eq!(
            fields.get("docs"),
            Some(&HostValue::String("A host player.".to_owned()))
        );
        assert_eq!(
            fields.get("source_span"),
            Some(&HostValue::Record {
                type_name: "ReflectSourceSpan".to_owned(),
                fields: BTreeMap::from([
                    ("source".to_owned(), HostValue::Int(7)),
                    ("start".to_owned(), HostValue::Int(10)),
                    ("end".to_owned(), HostValue::Int(20)),
                ])
            })
        );

        let error = type_by_name(&registry, "Plyer").expect_err("unknown type");
        assert_eq!(
            error.kind,
            ReflectErrorKind::UnknownTypeName {
                type_name: "Plyer".to_owned(),
                candidates: vec!["Player".to_owned(), "QuestProgress".to_owned()],
                related: vec![
                    crate::candidates::ReflectCandidate::new(
                        "Player",
                        Some(Span::new(SourceId::new(7), 10, 20))
                    ),
                    crate::candidates::ReflectCandidate::new("QuestProgress", None),
                ],
            }
        );
    }
}
