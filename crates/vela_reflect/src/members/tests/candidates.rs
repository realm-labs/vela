use super::*;

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
                crate::candidates::ReflectCandidate::new(
                    "Active",
                    Some(Span::new(SourceId::new(8), 90, 100))
                ),
                crate::candidates::ReflectCandidate::new("Finished", None),
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
                crate::candidates::ReflectCandidate::new(
                    "Active",
                    Some(Span::new(SourceId::new(8), 90, 100))
                ),
                crate::candidates::ReflectCandidate::new("Finished", None),
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

    let ReflectValue::Array(traits) = all_traits(&registry) else {
        panic!("trait list should be an array");
    };
    assert_eq!(traits.len(), 2);
    let ReflectValue::ScriptRecord {
        fields: first_trait,
        ..
    } = &traits[0]
    else {
        panic!("trait list item should be a record");
    };
    assert_eq!(
        first_trait.get("name"),
        Some(&ReflectValue::Host(HostValue::String(
            "Damageable".to_owned()
        )))
    );
    let first_trait_value = traits[0].clone();
    assert_eq!(
        origin(&registry, &first_trait_value).expect("trait origin metadata"),
        ReflectValue::Host(HostValue::String("host".to_owned()))
    );

    let ReflectValue::ScriptRecord { fields, .. } =
        trait_by_name(&registry, "Damageable").expect("trait metadata")
    else {
        panic!("trait metadata should be a record");
    };
    assert_eq!(
        fields.get("name"),
        Some(&ReflectValue::Host(HostValue::String(
            "Damageable".to_owned()
        )))
    );
    assert_eq!(
        fields.get("origin"),
        Some(&ReflectValue::Host(HostValue::String("host".to_owned())))
    );
    assert_eq!(
        fields.get("source_span"),
        Some(&span_value(Some(Span::new(SourceId::new(8), 20, 40))))
    );
    assert!(matches!(
        fields.get("methods"),
        Some(ReflectValue::Array(methods)) if methods.len() == 1
    ));
    let Some(ReflectValue::Array(methods)) = fields.get("methods") else {
        panic!("trait methods should be an array");
    };
    let ReflectValue::ScriptRecord {
        fields: method_fields,
        ..
    } = &methods[0]
    else {
        panic!("trait method should be a record");
    };
    let trait_method_value = methods[0].clone();
    assert_eq!(
        method_fields.get("owner"),
        Some(&ReflectValue::Host(HostValue::String(
            "Damageable".to_owned()
        )))
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
        Some(&ReflectValue::Host(HostValue::String("int".to_owned())))
    );
    assert_eq!(
        method_fields.get("returns"),
        Some(&ReflectValue::Host(HostValue::String("int".to_owned())))
    );
    let Some(ReflectValue::Array(params)) = method_fields.get("params") else {
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
                crate::candidates::ReflectCandidate::new(
                    "Damageable",
                    Some(Span::new(SourceId::new(8), 20, 40))
                ),
                crate::candidates::ReflectCandidate::new(
                    "Trackable",
                    Some(Span::new(SourceId::new(9), 10, 30))
                ),
            ],
        }
    );
}
