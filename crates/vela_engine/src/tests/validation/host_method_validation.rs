use super::*;
use vela_reflect::registry::MethodDesc;

#[test]
fn engine_rejects_duplicate_field_ids() {
    let result = Engine::builder()
        .register_type_desc(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
                .field(FieldDesc::new(FieldId::new(1), "level"))
                .field(FieldDesc::new(FieldId::new(1), "exp")),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::DuplicateFieldId {
            type_name: "Player".to_owned(),
            id: 1,
        }
    ));
}

#[test]
fn engine_rejects_duplicate_field_names() {
    let result = Engine::builder()
        .register_type_desc(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
                .field(FieldDesc::new(FieldId::new(1), "level"))
                .field(FieldDesc::new(FieldId::new(2), "level")),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::DuplicateFieldName {
            type_name: "Player".to_owned(),
            name: "level".to_owned(),
        }
    ));
}

#[test]
fn engine_rejects_duplicate_variant_ids() {
    let result = Engine::builder()
        .register_type_desc(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Reward"))
                .variant(VariantDesc::new(VariantId::new(1), "Gold"))
                .variant(VariantDesc::new(VariantId::new(1), "Gem")),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::DuplicateVariantId {
            type_name: "Reward".to_owned(),
            id: 1,
        }
    ));
}

#[test]
fn engine_rejects_duplicate_variant_names() {
    let result = Engine::builder()
        .register_type_desc(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Reward"))
                .variant(VariantDesc::new(VariantId::new(1), "Gold"))
                .variant(VariantDesc::new(VariantId::new(2), "Gold")),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::DuplicateVariantName {
            type_name: "Reward".to_owned(),
            name: "Gold".to_owned(),
        }
    ));
}

#[test]
fn engine_rejects_duplicate_variant_field_ids() {
    let result = Engine::builder()
        .register_type_desc(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Reward")).variant(
                VariantDesc::new(VariantId::new(1), "Gold")
                    .field(FieldDesc::new(FieldId::new(1), "item_id"))
                    .field(FieldDesc::new(FieldId::new(1), "count")),
            ),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::DuplicateVariantFieldId {
            type_name: "Reward".to_owned(),
            variant: "Gold".to_owned(),
            id: 1,
        }
    ));
}

#[test]
fn engine_rejects_duplicate_variant_field_names() {
    let result = Engine::builder()
        .register_type_desc(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Reward")).variant(
                VariantDesc::new(VariantId::new(1), "Gold")
                    .field(FieldDesc::new(FieldId::new(1), "count"))
                    .field(FieldDesc::new(FieldId::new(2), "count")),
            ),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::DuplicateVariantFieldName {
            type_name: "Reward".to_owned(),
            variant: "Gold".to_owned(),
            name: "count".to_owned(),
        }
    ));
}

#[test]
fn engine_rejects_duplicate_trait_ids() {
    let result = Engine::builder()
        .register_type_desc(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
                .trait_impl(trait_desc_with_id(TraitId::new(1), "Damageable"))
                .trait_impl(trait_desc_with_id(TraitId::new(1), "Rewardable")),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::DuplicateTraitId {
            type_name: "Player".to_owned(),
            id: 1,
        }
    ));
}

#[test]
fn engine_rejects_duplicate_trait_names() {
    let result = Engine::builder()
        .register_type_desc(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
                .trait_impl(trait_desc_with_id(TraitId::new(1), "Damageable"))
                .trait_impl(trait_desc_with_id(TraitId::new(2), "Damageable")),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::DuplicateTraitName {
            type_name: "Player".to_owned(),
            name: "Damageable".to_owned(),
        }
    ));
}

#[test]
fn engine_rejects_duplicate_trait_method_ids() {
    let result = Engine::builder()
        .register_type_desc(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Player")).trait_impl(
                trait_desc_with_id(TraitId::new(1), "Damageable")
                    .method(TraitMethodDesc::new(MethodId::new(1), "damage"))
                    .method(TraitMethodDesc::new(MethodId::new(1), "heal")),
            ),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::DuplicateTraitMethodId {
            type_name: "Player".to_owned(),
            trait_name: "Damageable".to_owned(),
            id: 1,
        }
    ));
}

#[test]
fn engine_rejects_duplicate_trait_method_names() {
    let result = Engine::builder()
        .register_type_desc(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Player")).trait_impl(
                trait_desc_with_id(TraitId::new(1), "Damageable")
                    .method(TraitMethodDesc::new(MethodId::new(1), "damage"))
                    .method(TraitMethodDesc::new(MethodId::new(2), "damage")),
            ),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::DuplicateTraitMethodName {
            type_name: "Player".to_owned(),
            trait_name: "Damageable".to_owned(),
            name: "damage".to_owned(),
        }
    ));
}

#[test]
fn engine_rejects_duplicate_trait_method_param_names() {
    let result = Engine::builder()
        .register_type_desc(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Player")).trait_impl(
                trait_desc_with_id(TraitId::new(1), "Damageable").method(
                    TraitMethodDesc::new(MethodId::new(1), "damage")
                        .param(MethodParamDesc::new("amount"))
                        .param(MethodParamDesc::new("amount")),
                ),
            ),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::DuplicateTraitMethodParamName {
            type_name: "Player".to_owned(),
            trait_name: "Damageable".to_owned(),
            method: "damage".to_owned(),
            name: "amount".to_owned(),
        }
    ));
}

#[test]
fn engine_rejects_duplicate_host_method_ids() {
    let result = Engine::builder()
        .register_type_desc(
            player_type(TypeId::new(1))
                .method(MethodDesc::new(MethodId::new(1), "grant_exp"))
                .method(MethodDesc::new(MethodId::new(1), "heal")),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::DuplicateMethodId { id: 1 }
    ));
}

#[test]
fn engine_rejects_malformed_trait_method_param_names() {
    let result = Engine::builder()
        .register_type_desc(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Player")).trait_impl(
                trait_desc_with_id(TraitId::new(1), "Damageable").method(
                    TraitMethodDesc::new(MethodId::new(1), "damage")
                        .param(MethodParamDesc::new("")),
                ),
            ),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::InvalidSchemaMemberName {
            type_name: "Player".to_owned(),
            member_kind: "trait method parameter".to_owned(),
            name: "".to_owned(),
        }
    ));
}

#[test]
fn engine_rejects_duplicate_host_method_names() {
    let result = Engine::builder()
        .register_type_desc(
            player_type(TypeId::new(1))
                .method(MethodDesc::new(MethodId::new(1), "grant_exp"))
                .method(MethodDesc::new(MethodId::new(2), "grant_exp")),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::DuplicateMethodName {
            name: "grant_exp".to_owned()
        }
    ));
}

#[test]
fn engine_rejects_duplicate_host_method_param_names() {
    let result = Engine::builder()
        .register_type_desc(
            player_type(TypeId::new(1)).method(
                MethodDesc::new(MethodId::new(1), "grant_exp")
                    .param(MethodParamDesc::new("amount"))
                    .param(MethodParamDesc::new("amount")),
            ),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::DuplicateMethodParamName {
            type_name: "Player".to_owned(),
            method: "grant_exp".to_owned(),
            name: "amount".to_owned(),
        }
    ));
}

#[test]
fn engine_rejects_empty_host_method_required_permissions() {
    let result = Engine::builder()
        .register_type_desc(
            player_type(TypeId::new(1)).method(
                MethodDesc::new(MethodId::new(1), "grant_exp")
                    .access(vela_reflect::access::MethodAccess::new().require_permission("")),
            ),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::InvalidPermissionName {
            descriptor: "method Player.grant_exp".to_owned(),
            name: "".to_owned(),
        }
    ));
}

#[test]
fn engine_rejects_empty_host_method_attribute_names() {
    let result = Engine::builder()
        .register_type_desc(
            player_type(TypeId::new(1))
                .method(MethodDesc::new(MethodId::new(1), "grant_exp").attr("", "bad")),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::InvalidAttributeName {
            descriptor: "method Player.grant_exp".to_owned(),
            name: "".to_owned(),
        }
    ));
}

#[test]
fn engine_rejects_generic_host_method_type_hints() {
    let result = Engine::builder()
        .register_type_desc(
            player_type(TypeId::new(1)).method(
                MethodDesc::new(MethodId::new(1), "grant_rewards")
                    .param(MethodParamDesc::new("items").type_hint("Array<Item>"))
                    .return_type("Result<i64>"),
            ),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::InvalidTypeHintName {
            descriptor: "method Player.grant_rewards return".to_owned(),
            type_name: "Result<i64>".to_owned(),
        }
    ));
}

#[test]
fn engine_rejects_generic_host_method_param_type_hints() {
    let result = Engine::builder()
        .register_type_desc(
            player_type(TypeId::new(1)).method(
                MethodDesc::new(MethodId::new(1), "grant_rewards")
                    .param(MethodParamDesc::new("items").type_hint("Set<Function>"))
                    .return_type("Result"),
            ),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::InvalidTypeHintName {
            descriptor: "method Player.grant_rewards parameter items".to_owned(),
            type_name: "Set<Function>".to_owned(),
        }
    ));
}

#[test]
fn engine_rejects_malformed_host_method_names() {
    let result = Engine::builder()
        .register_type_desc(
            player_type(TypeId::new(1)).method(MethodDesc::new(MethodId::new(1), "")),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::InvalidSchemaMemberName {
            type_name: "Player".to_owned(),
            member_kind: "method".to_owned(),
            name: "".to_owned(),
        }
    ));
}
