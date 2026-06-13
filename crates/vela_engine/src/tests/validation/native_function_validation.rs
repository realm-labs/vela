use super::*;

#[test]
fn engine_rejects_native_methods_for_unknown_owner_types() {
    let result = Engine::builder()
        .register_native_method_fn(
            NativeMethodDesc::new(
                TypeKey::new(TypeId::new(99), "Missing"),
                HostMethodId::new(1),
                "grant_exp",
            ),
            |_, _, _| Ok(OwnedValue::Null),
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
            NativeFunctionDesc::new("game::first", NativeFunctionId::new(10)),
            |_| Ok(OwnedValue::Null),
        )
        .register_native_fn(
            NativeFunctionDesc::new("game::second", NativeFunctionId::new(10)),
            |_| Ok(OwnedValue::Null),
        )
        .build();

    assert!(matches!(
        result.map(|_| ()),
        Err(error) if error.kind == EngineErrorKind::DuplicateNativeFunctionId { id: 10 }
    ));
}

#[test]
fn engine_rejects_empty_module_attribute_names() {
    let result = Engine::builder()
        .register_module(ModuleDesc::new("game::reward").attr("", "bad"))
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::InvalidAttributeName {
            descriptor: "module game::reward".to_owned(),
            name: "".to_owned(),
        }
    ));
}

#[test]
fn engine_rejects_duplicate_names_across_host_and_pure_natives() {
    let result = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::same", NativeFunctionId::new(10)),
            |_| Ok(OwnedValue::Null),
        )
        .register_host_native_fn(
            NativeFunctionDesc::new("game::same", NativeFunctionId::new(11)),
            |_, _| Ok(OwnedValue::Null),
        )
        .build();

    assert!(matches!(
        result.map(|_| ()),
        Err(error) if error.kind == EngineErrorKind::DuplicateNativeFunctionName {
            name: "game::same".to_owned()
        }
    ));
}

#[test]
fn engine_rejects_duplicate_ids_across_host_and_pure_natives() {
    let result = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::first", NativeFunctionId::new(16)),
            |_| Ok(OwnedValue::Null),
        )
        .register_host_native_fn(
            NativeFunctionDesc::new("game::second", NativeFunctionId::new(16)),
            |_, _| Ok(OwnedValue::Null),
        )
        .build();

    assert!(matches!(
        result.map(|_| ()),
        Err(error) if error.kind == EngineErrorKind::DuplicateNativeFunctionId { id: 16 }
    ));
}

#[test]
fn engine_rejects_duplicate_names_across_context_host_and_pure_natives() {
    let result = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::same", NativeFunctionId::new(12)),
            |_| Ok(OwnedValue::Null),
        )
        .register_context_host_native_fn(
            NativeFunctionDesc::new("game::same", NativeFunctionId::new(13)),
            |_, _| Ok(OwnedValue::Null),
        )
        .build();

    assert!(matches!(
        result.map(|_| ()),
        Err(error) if error.kind == EngineErrorKind::DuplicateNativeFunctionName {
            name: "game::same".to_owned()
        }
    ));
}

#[test]
fn engine_rejects_duplicate_names_across_context_host_and_host_natives() {
    let result = Engine::builder()
        .register_host_native_fn(
            NativeFunctionDesc::new("game::same", NativeFunctionId::new(14)),
            |_, _| Ok(OwnedValue::Null),
        )
        .register_context_host_native_fn(
            NativeFunctionDesc::new("game::same", NativeFunctionId::new(15)),
            |_, _| Ok(OwnedValue::Null),
        )
        .build();

    assert!(matches!(
        result.map(|_| ()),
        Err(error) if error.kind == EngineErrorKind::DuplicateNativeFunctionName {
            name: "game::same".to_owned()
        }
    ));
}

#[test]
fn engine_rejects_duplicate_context_host_native_ids() {
    let result = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::first", NativeFunctionId::new(30)),
            |_| Ok(OwnedValue::Null),
        )
        .register_context_host_native_fn(
            NativeFunctionDesc::new("game::second", NativeFunctionId::new(30)),
            |_, _| Ok(OwnedValue::Null),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::DuplicateNativeFunctionId { id: 30 }
    ));
}

#[test]
fn engine_rejects_duplicate_ids_across_host_and_context_host_natives() {
    let result = Engine::builder()
        .register_host_native_fn(
            NativeFunctionDesc::new("game::first", NativeFunctionId::new(40)),
            |_, _| Ok(OwnedValue::Null),
        )
        .register_context_host_native_fn(
            NativeFunctionDesc::new("game::second", NativeFunctionId::new(40)),
            |_, _| Ok(OwnedValue::Null),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::DuplicateNativeFunctionId { id: 40 }
    ));
}

#[test]
fn engine_rejects_duplicate_native_function_param_names() {
    let result = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::grant_reward", NativeFunctionId::new(31))
                .param("amount", TypeHint::i64())
                .param("amount", TypeHint::string()),
            |_| Ok(OwnedValue::Null),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::DuplicateNativeFunctionParamName {
            function: "game::grant_reward".to_owned(),
            name: "amount".to_owned(),
        }
    ));
}

#[test]
fn engine_rejects_empty_native_function_attribute_names() {
    let result = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::grant_reward", NativeFunctionId::new(39))
                .attr("", "bad"),
            |_| Ok(OwnedValue::Null),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::InvalidAttributeName {
            descriptor: "native function game::grant_reward".to_owned(),
            name: "".to_owned(),
        }
    ));
}

#[test]
fn engine_rejects_malformed_native_function_names() {
    let result = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game..grant_reward", NativeFunctionId::new(32)),
            |_| Ok(OwnedValue::Null),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::InvalidNativeFunctionName {
            name: "game..grant_reward".to_owned()
        }
    ));
}

#[test]
fn engine_rejects_malformed_native_function_param_names() {
    let result = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::grant_reward", NativeFunctionId::new(33))
                .param("", TypeHint::i64()),
            |_| Ok(OwnedValue::Null),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::InvalidNativeFunctionParamName {
            function: "game::grant_reward".to_owned(),
            name: "".to_owned(),
        }
    ));
}

#[test]
fn engine_rejects_unknown_native_function_param_type_hints() {
    let result = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::grant_reward", NativeFunctionId::new(34)).param(
                "player",
                TypeHint::Record(TypeKey::new(TypeId::new(99), "Missing")),
            ),
            |_| Ok(OwnedValue::Null),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::UnknownTypeHint {
            descriptor: "native function game::grant_reward parameter player".to_owned(),
            type_name: "Missing".to_owned(),
        }
    ));
}

#[test]
fn engine_rejects_unknown_native_function_return_type_hints() {
    let result = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::find_player", NativeFunctionId::new(35))
                .returns(TypeHint::Enum(TypeKey::new(TypeId::new(99), "Missing"))),
            |_| Ok(OwnedValue::Null),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::UnknownTypeHint {
            descriptor: "native function game::find_player return".to_owned(),
            type_name: "Missing".to_owned(),
        }
    ));
}

#[test]
fn engine_accepts_registered_native_function_record_type_hints() {
    let reward_key = TypeKey::new(TypeId::new(77), "Reward");
    let result = Engine::builder()
        .register_type(TypeDesc::new(reward_key.clone()).kind(TypeKind::ScriptStruct))
        .register_native_fn(
            NativeFunctionDesc::new("game::inspect_reward", NativeFunctionId::new(36))
                .param("reward", TypeHint::Record(reward_key.clone()))
                .returns(TypeHint::Record(reward_key)),
            |_| Ok(OwnedValue::Null),
        )
        .build();

    assert!(result.is_ok());
}

#[test]
fn engine_accepts_native_function_iterator_type_hints() {
    let engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::scores", NativeFunctionId::new(37))
                .returns(TypeHint::iterator()),
            |_| Ok(OwnedValue::iterator([1_i64, 2, 3])),
        )
        .build()
        .expect("iterator return type hint should be accepted");

    let registry = engine.registry();
    let function = registry
        .function_by_name("game::scores")
        .expect("native function should be reflected");
    assert_eq!(function.return_type.as_deref(), Some("iterator"));
}

#[test]
fn engine_rejects_unknown_native_function_trait_type_hints() {
    let result = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::inspect_damageable", NativeFunctionId::new(38))
                .param("target", TypeHint::Trait("Damageable".to_owned())),
            |_| Ok(OwnedValue::Null),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::UnknownTypeHint {
            descriptor: "native function game::inspect_damageable parameter target".to_owned(),
            type_name: "Damageable".to_owned(),
        }
    ));
}

#[test]
fn engine_rejects_native_function_names_that_shadow_standard_natives() {
    let result = Engine::builder()
        .with_standard_natives()
        .register_native_fn(
            NativeFunctionDesc::new("math::clamp", NativeFunctionId::new(0x1234)),
            |_| Ok(OwnedValue::Null),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::DuplicateNativeFunctionName {
            name: "math::clamp".to_owned()
        }
    ));
}

#[test]
fn engine_rejects_native_function_ids_that_collide_with_standard_natives() {
    let math_clamp_id = vela_stdlib::STD_FUNCTIONS
        .iter()
        .find(|spec| spec.module == "math" && spec.name == "clamp")
        .expect("math::clamp should be declared in the stdlib manifest")
        .id();
    let result = Engine::builder()
        .with_standard_natives()
        .register_native_fn(
            NativeFunctionDesc::new("game::custom_clamp", math_clamp_id),
            |_| Ok(OwnedValue::Null),
        )
        .build();

    match result {
        Err(error) => assert_eq!(
            error.kind,
            EngineErrorKind::DuplicateNativeFunctionId {
                id: math_clamp_id.get()
            }
        ),
        Ok(_) => panic!("standard native ID collision should fail"),
    }
}
