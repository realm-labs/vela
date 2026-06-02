use vela_common::{FieldId, HostMethodId, HostTypeId, MethodId, TraitId, TypeId, VariantId};
use vela_reflect::modules::ModuleDesc;
use vela_reflect::registry::{
    FieldDesc, MethodDesc, MethodParamDesc, TraitMethodDesc, TypeDesc, TypeKey, VariantDesc,
};

use vela_vm::value::Value;

use crate::engine::Engine;
use crate::error::EngineErrorKind;
use crate::method::NativeMethodDesc;
use crate::native::{NativeFunctionDesc, NativeFunctionId, TypeHint};
use crate::standard::MATH_CLAMP_FUNCTION_ID;

use super::{player_type, trait_desc_with_id};

#[test]
fn engine_rejects_native_methods_for_unknown_owner_types() {
    let result = Engine::builder()
        .register_native_method_fn(
            NativeMethodDesc::new(
                TypeKey::new(TypeId::new(99), "Missing"),
                HostMethodId::new(1),
                "grant_exp",
            ),
            |_, _, _| Ok(Value::Null),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::UnknownNativeMethodOwner {
            name: "Missing".to_owned()
        }
    ));
}

#[test]
fn engine_rejects_duplicate_native_function_ids() {
    let result = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game.first", NativeFunctionId::new(10)),
            |_| Ok(Value::Null),
        )
        .register_native_fn(
            NativeFunctionDesc::new("game.second", NativeFunctionId::new(10)),
            |_| Ok(Value::Null),
        )
        .build();

    assert!(matches!(
        result.map(|_| ()),
        Err(error) if error.kind == EngineErrorKind::DuplicateNativeFunctionId { id: 10 }
    ));
}

#[test]
fn engine_rejects_duplicate_names_across_host_and_pure_natives() {
    let result = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game.same", NativeFunctionId::new(10)),
            |_| Ok(Value::Null),
        )
        .register_host_native_fn(
            NativeFunctionDesc::new("game.same", NativeFunctionId::new(11)),
            |_, _| Ok(Value::Null),
        )
        .build();

    assert!(matches!(
        result.map(|_| ()),
        Err(error) if error.kind == EngineErrorKind::DuplicateNativeFunctionName {
            name: "game.same".to_owned()
        }
    ));
}

#[test]
fn engine_rejects_duplicate_context_host_native_ids() {
    let result = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game.first", NativeFunctionId::new(30)),
            |_| Ok(Value::Null),
        )
        .register_context_host_native_fn(
            NativeFunctionDesc::new("game.second", NativeFunctionId::new(30)),
            |_, _| Ok(Value::Null),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::DuplicateNativeFunctionId { id: 30 }
    ));
}

#[test]
fn engine_rejects_duplicate_native_function_param_names() {
    let result = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game.grant_reward", NativeFunctionId::new(31))
                .param("amount", TypeHint::Int)
                .param("amount", TypeHint::String),
            |_| Ok(Value::Null),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::DuplicateNativeFunctionParamName {
            function: "game.grant_reward".to_owned(),
            name: "amount".to_owned(),
        }
    ));
}

#[test]
fn engine_rejects_native_function_names_that_shadow_standard_natives() {
    let result = Engine::builder()
        .with_standard_natives()
        .register_native_fn(
            NativeFunctionDesc::new("math.clamp", NativeFunctionId::new(0x1234)),
            |_| Ok(Value::Null),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::DuplicateNativeFunctionName {
            name: "math.clamp".to_owned()
        }
    ));
}

#[test]
fn engine_rejects_native_function_ids_that_collide_with_standard_natives() {
    let result = Engine::builder()
        .with_standard_natives()
        .register_native_fn(
            NativeFunctionDesc::new("game.custom_clamp", MATH_CLAMP_FUNCTION_ID),
            |_| Ok(Value::Null),
        )
        .build();

    match result {
        Err(error) => assert_eq!(
            error.kind,
            EngineErrorKind::DuplicateNativeFunctionId {
                id: MATH_CLAMP_FUNCTION_ID.get()
            }
        ),
        Ok(_) => panic!("standard native ID collision should fail"),
    }
}

#[test]
fn engine_rejects_duplicate_module_names() {
    let result = Engine::builder()
        .register_module(ModuleDesc::new("game.reward"))
        .register_module(ModuleDesc::new("game.reward"))
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::DuplicateModuleName {
            name: "game.reward".to_owned()
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
fn engine_rejects_duplicate_host_type_ids() {
    let result = Engine::builder()
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .register_type(
            TypeDesc::new(TypeKey::new(TypeId::new(2), "Monster")).host_type(HostTypeId::new(1)),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::DuplicateHostTypeId { id: 1 }
    ));
}

#[test]
fn engine_rejects_duplicate_field_ids() {
    let result = Engine::builder()
        .register_type(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
                .host_type(HostTypeId::new(1))
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
        .register_type(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
                .host_type(HostTypeId::new(1))
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
        .register_type(
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
        .register_type(
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
        .register_type(
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
        .register_type(
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
        .register_type(
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
        .register_type(
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
        .register_type(
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
        .register_type(
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
        .register_type(
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
        .register_type(
            player_type(TypeId::new(1), HostTypeId::new(1))
                .method(MethodDesc::new(HostMethodId::new(1), "grant_exp"))
                .method(MethodDesc::new(HostMethodId::new(1), "heal")),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::DuplicateHostMethodId { id: 1 }
    ));
}

#[test]
fn engine_rejects_duplicate_host_method_names() {
    let result = Engine::builder()
        .register_type(
            player_type(TypeId::new(1), HostTypeId::new(1))
                .method(MethodDesc::new(HostMethodId::new(1), "grant_exp"))
                .method(MethodDesc::new(HostMethodId::new(2), "grant_exp")),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::DuplicateHostMethodName {
            name: "grant_exp".to_owned()
        }
    ));
}

#[test]
fn engine_rejects_duplicate_host_method_param_names() {
    let result = Engine::builder()
        .register_type(
            player_type(TypeId::new(1), HostTypeId::new(1)).method(
                MethodDesc::new(HostMethodId::new(1), "grant_exp")
                    .param(MethodParamDesc::new("amount"))
                    .param(MethodParamDesc::new("amount")),
            ),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::DuplicateHostMethodParamName {
            type_name: "Player".to_owned(),
            method: "grant_exp".to_owned(),
            name: "amount".to_owned(),
        }
    ));
}

#[test]
fn engine_rejects_duplicate_native_method_ids() {
    let player_key = TypeKey::new(TypeId::new(1), "Player");
    let result = Engine::builder()
        .register_type(player_type(player_key.id, HostTypeId::new(1)))
        .register_native_method_fn(
            NativeMethodDesc::new(player_key.clone(), HostMethodId::new(44), "grant_exp"),
            |_, _, _| Ok(Value::Null),
        )
        .register_native_method_fn(
            NativeMethodDesc::new(player_key, HostMethodId::new(44), "heal"),
            |_, _, _| Ok(Value::Null),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::DuplicateHostMethodId { id: 44 }
    ));
}

#[test]
fn engine_rejects_duplicate_native_method_param_names() {
    let player_key = TypeKey::new(TypeId::new(1), "Player");
    let result = Engine::builder()
        .register_type(player_type(player_key.id, HostTypeId::new(1)))
        .register_host_method_desc(
            NativeMethodDesc::new(player_key, HostMethodId::new(44), "grant_exp")
                .param("amount", TypeHint::Int)
                .param("amount", TypeHint::String),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::DuplicateHostMethodParamName {
            type_name: "Player".to_owned(),
            method: "grant_exp".to_owned(),
            name: "amount".to_owned(),
        }
    ));
}
