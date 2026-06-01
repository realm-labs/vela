use super::*;

#[test]
fn reflect_implements_uses_registry_metadata() {
    let registry = registry();

    assert!(
        implements(
            &registry,
            &ReflectValue::HostRef(player_ref()),
            &trait_name("Damageable"),
        )
        .expect("implements check")
    );
    assert!(
        !implements(
            &registry,
            &ReflectValue::HostRef(player_ref()),
            &trait_name("Trackable")
        )
        .expect("known unimplemented trait check")
    );
    let player_type = type_metadata_by_name(&registry, "Player").expect("type metadata");
    assert!(
        implements(&registry, &player_type, &trait_name("Damageable"))
            .expect("copied type descriptor implements check")
    );
    assert!(
        !implements(&registry, &player_type, &trait_name("Trackable"))
            .expect("copied type descriptor negative implements check")
    );
    let damageable_trait = trait_metadata_by_name(&registry, "Damageable").expect("trait metadata");
    assert!(
        implements(&registry, &player_type, &damageable_trait)
            .expect("copied trait descriptor implements check")
    );

    let error = implements(
        &registry,
        &ReflectValue::HostRef(player_ref()),
        &trait_name("Damagable"),
    )
    .expect_err("unknown trait should diagnose");
    assert_eq!(
        error.kind,
        ReflectErrorKind::UnknownTrait {
            trait_name: "Damagable".to_owned(),
            candidates: vec!["Damageable".to_owned(), "Trackable".to_owned()],
            related: vec![
                ReflectCandidate::new("Damageable", None),
                ReflectCandidate::new("Trackable", None),
            ],
        }
    );
}

#[test]
fn reflect_implements_uses_script_type_metadata() {
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(200), "game.Player"))
            .kind(TypeKind::ScriptStruct)
            .trait_impl(TraitDesc::new("game.Damageable")),
    );
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(201), "game.Progress"))
            .kind(TypeKind::ScriptEnum)
            .trait_impl(TraitDesc::new("game.Trackable")),
    );

    assert!(
        implements(
            &registry,
            &ReflectValue::ScriptRecord {
                type_name: "game.Player".to_owned(),
                fields: BTreeMap::new(),
            },
            &trait_name("game.Damageable"),
        )
        .expect("script record implements check")
    );
    assert!(
        implements(
            &registry,
            &ReflectValue::ScriptEnum {
                enum_name: "game.Progress".to_owned(),
                variant: "Active".to_owned(),
                fields: BTreeMap::new(),
            },
            &trait_name("game.Trackable"),
        )
        .expect("script enum implements check")
    );
    assert!(
        !implements(
            &registry,
            &ReflectValue::ScriptRecord {
                type_name: "game.Player".to_owned(),
                fields: BTreeMap::new(),
            },
            &trait_name("game.Trackable"),
        )
        .expect("script record negative implements check")
    );
}
