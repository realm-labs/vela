use super::*;

#[test]
fn engine_rejects_type_names_that_shadow_standard_types() {
    let result = Engine::builder()
        .with_standard_natives()
        .register_type(TypeDesc::new(TypeKey::new(TypeId::new(0x1234), "Option")))
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::DuplicateTypeName {
            name: "Option".to_owned()
        }
    ));
}

#[test]
fn engine_rejects_type_ids_that_collide_with_standard_types() {
    let result = Engine::builder()
        .with_standard_natives()
        .register_type(TypeDesc::new(TypeKey::new(INT_TYPE_ID, "GameInt")))
        .build();

    match result {
        Err(error) => assert_eq!(
            error.kind,
            EngineErrorKind::DuplicateTypeId {
                id: INT_TYPE_ID.get()
            }
        ),
        Ok(_) => panic!("standard type ID collision should fail"),
    }
}

#[test]
fn engine_rejects_duplicate_module_names() {
    let result = Engine::builder()
        .register_module(ModuleDesc::new("game::reward"))
        .register_module(ModuleDesc::new("game::reward"))
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::DuplicateModuleName {
            name: "game::reward".to_owned()
        }
    ));
}

#[test]
fn engine_rejects_malformed_module_names() {
    let result = Engine::builder()
        .register_module(ModuleDesc::new("game..reward"))
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::InvalidModuleName {
            name: "game..reward".to_owned()
        }
    ));
}

#[test]
fn engine_rejects_module_names_that_shadow_standard_modules() {
    let result = Engine::builder()
        .with_standard_natives()
        .register_module(ModuleDesc::new("math"))
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::DuplicateModuleName {
            name: "math".to_owned()
        }
    ));
}

#[test]
fn engine_rejects_module_names_that_shadow_context_clock_modules() {
    let result = Engine::builder()
        .with_context_clock(1, 2)
        .register_module(ModuleDesc::new("ctx"))
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::DuplicateModuleName {
            name: "ctx".to_owned()
        }
    ));
}

#[test]
fn engine_rejects_module_names_that_shadow_controlled_random_modules() {
    let result = Engine::builder()
        .with_controlled_random(7)
        .register_module(ModuleDesc::new("math"))
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::DuplicateModuleName {
            name: "math".to_owned()
        }
    ));
}

#[test]
fn engine_rejects_duplicate_type_names() {
    let result = Engine::builder()
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .register_type(player_type(TypeId::new(2), HostTypeId::new(2)))
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::DuplicateTypeName {
            name: "Player".to_owned()
        }
    ));
}

#[test]
fn engine_rejects_malformed_type_names() {
    let result = Engine::builder()
        .register_type(TypeDesc::new(TypeKey::new(TypeId::new(1), "")))
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::InvalidTypeName {
            name: "".to_owned(),
        }
    ));
}

#[test]
fn engine_rejects_empty_type_attribute_names() {
    let result = Engine::builder()
        .register_type(TypeDesc::new(TypeKey::new(TypeId::new(1), "Player")).attr("", "bad"))
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::InvalidAttributeName {
            descriptor: "type Player".to_owned(),
            name: "".to_owned(),
        }
    ));
}

#[test]
fn engine_rejects_empty_field_type_hints() {
    let result = Engine::builder()
        .register_type(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
                .field(FieldDesc::new(FieldId::new(1), "level").type_hint("")),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::InvalidTypeHintName {
            descriptor: "field Player.level".to_owned(),
            type_name: "".to_owned(),
        }
    ));
}

#[test]
fn engine_rejects_generic_field_type_hints() {
    let result = Engine::builder()
        .register_type(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
                .field(FieldDesc::new(FieldId::new(1), "inventory").type_hint("Array<Item>")),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::InvalidTypeHintName {
            descriptor: "field Player.inventory".to_owned(),
            type_name: "Array<Item>".to_owned(),
        }
    ));
}

#[test]
fn engine_rejects_duplicate_type_ids() {
    let result = Engine::builder()
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .register_type(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Monster")).host_type(HostTypeId::new(2)),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::DuplicateTypeId { id: 1 }
    ));
}

#[test]
fn engine_rejects_malformed_field_names() {
    let result = Engine::builder()
        .register_type(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
                .host_type(HostTypeId::new(1))
                .field(FieldDesc::new(FieldId::new(1), "")),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::InvalidSchemaMemberName {
            type_name: "Player".to_owned(),
            member_kind: "field".to_owned(),
            name: "".to_owned(),
        }
    ));
}

#[test]
fn engine_rejects_empty_field_required_permissions() {
    let result = Engine::builder()
        .register_type(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
                .host_type(HostTypeId::new(1))
                .field(
                    FieldDesc::new(FieldId::new(1), "level")
                        .access(FieldAccess::new().require_permission("")),
                ),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::InvalidPermissionName {
            descriptor: "field Player.level".to_owned(),
            name: "".to_owned(),
        }
    ));
}

#[test]
fn engine_rejects_empty_field_attribute_names() {
    let result = Engine::builder()
        .register_type(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
                .field(FieldDesc::new(FieldId::new(1), "level").attr("", "bad")),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::InvalidAttributeName {
            descriptor: "field Player.level".to_owned(),
            name: "".to_owned(),
        }
    ));
}

#[test]
fn engine_rejects_empty_variant_attribute_names() {
    let result = Engine::builder()
        .register_type(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Reward"))
                .variant(VariantDesc::new(VariantId::new(1), "Gold").attr("", "bad")),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::InvalidAttributeName {
            descriptor: "variant Reward::Gold".to_owned(),
            name: "".to_owned(),
        }
    ));
}

#[test]
fn engine_rejects_empty_variant_field_attribute_names() {
    let result = Engine::builder()
        .register_type(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Reward")).variant(
                VariantDesc::new(VariantId::new(1), "Gold")
                    .field(FieldDesc::new(FieldId::new(1), "count").attr("", "bad")),
            ),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::InvalidAttributeName {
            descriptor: "variant field Reward::Gold::count".to_owned(),
            name: "".to_owned(),
        }
    ));
}

#[test]
fn engine_rejects_generic_variant_field_type_hints() {
    let result = Engine::builder()
        .register_type(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Reward")).variant(
                VariantDesc::new(VariantId::new(1), "Gold")
                    .field(FieldDesc::new(FieldId::new(1), "count").type_hint("Option<int>")),
            ),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::InvalidTypeHintName {
            descriptor: "variant field Reward::Gold::count".to_owned(),
            type_name: "Option<int>".to_owned(),
        }
    ));
}

#[test]
fn engine_rejects_empty_trait_attribute_names() {
    let result = Engine::builder()
        .register_type(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
                .trait_impl(trait_desc_with_id(TraitId::new(1), "Damageable").attr("", "bad")),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::InvalidAttributeName {
            descriptor: "trait Player::Damageable".to_owned(),
            name: "".to_owned(),
        }
    ));
}

#[test]
fn engine_rejects_empty_trait_method_attribute_names() {
    let result = Engine::builder()
        .register_type(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Player")).trait_impl(
                trait_desc_with_id(TraitId::new(1), "Damageable")
                    .method(TraitMethodDesc::new(MethodId::new(1), "damage").attr("", "bad")),
            ),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::InvalidAttributeName {
            descriptor: "trait method Player::Damageable::damage".to_owned(),
            name: "".to_owned(),
        }
    ));
}

#[test]
fn engine_rejects_generic_trait_method_type_hints() {
    let result = Engine::builder()
        .register_type(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Player")).trait_impl(
                trait_desc_with_id(TraitId::new(1), "Rewardable").method(
                    TraitMethodDesc::new(MethodId::new(1), "reward")
                        .param(MethodParamDesc::new("items").type_hint("Array<Item>"))
                        .return_type("Result<int>"),
                ),
            ),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::InvalidTypeHintName {
            descriptor: "trait method Player::Rewardable::reward return".to_owned(),
            type_name: "Result<int>".to_owned(),
        }
    ));
}

#[test]
fn engine_rejects_generic_trait_method_param_type_hints() {
    let result = Engine::builder()
        .register_type(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Player")).trait_impl(
                trait_desc_with_id(TraitId::new(1), "Rewardable").method(
                    TraitMethodDesc::new(MethodId::new(1), "reward")
                        .param(MethodParamDesc::new("items").type_hint("Array<Item>"))
                        .return_type("Result"),
                ),
            ),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::InvalidTypeHintName {
            descriptor: "trait method Player::Rewardable::reward parameter items".to_owned(),
            type_name: "Array<Item>".to_owned(),
        }
    ));
}
