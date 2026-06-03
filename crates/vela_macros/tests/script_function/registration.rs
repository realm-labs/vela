use super::*;

#[test]
fn script_function_registers_typed_native_with_engine() {
    let engine =
        vela_register_native_function_grant_bonus(Engine::builder().grant_permission("bonus.read"))
            .build()
            .expect("engine should build from macro native function");
    let program = compile_source!(
        engine,
        r#"
fn main() {
    return game::grant_bonus(6, 7);
}
"#,
        "source should compile with macro registered native"
    );

    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(Value::Int(42)),
    );
}

#[test]
fn script_function_alias_registers_renamed_native_with_stable_id() {
    let engine = vela_register_native_function_grant_bonus_v2(Engine::builder())
        .build()
        .expect("engine should build from macro renamed native function");
    let program = compile_source!(
        engine,
        r#"
fn main() {
    return game::grant_bonus_v2(5);
}
"#,
        "source should compile with macro registered renamed native"
    );

    let registry = engine.registry();
    let registered = registry
        .functions()
        .find(|function| function.name == "game::grant_bonus_v2")
        .expect("renamed function should be reflected");
    assert_eq!(registered.id, function_id("game::grant_bonus"));
    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(Value::Int(7)),
    );
}

#[test]
fn script_function_registers_typed_set_native_with_engine() {
    let engine = vela_register_native_function_count_labels(Engine::builder())
        .build()
        .expect("engine should build from macro set native function");
    let program = compile_source!(
        engine,
        r#"
fn main(labels) {
    return game::count_labels(labels);
}
"#,
        "source should compile with macro registered set native"
    );

    assert_eq!(
        engine.into_vm().run_program(
            &program,
            "main",
            &[Value::Set(vec![
                Value::String("raid".to_owned()),
                Value::String("pvp".to_owned()),
                Value::String("raid".to_owned()),
            ])],
        ),
        Ok(Value::Int(2)),
    );
}

#[test]
fn script_function_registers_typed_hash_set_native_with_engine() {
    let engine = vela_register_native_function_count_unordered_labels(Engine::builder())
        .build()
        .expect("engine should build from macro unordered set native function");
    let program = compile_source!(
        engine,
        r#"
fn main(labels) {
    return game::count_unordered_labels(labels);
}
"#,
        "source should compile with macro registered unordered set native"
    );

    assert_eq!(
        engine.into_vm().run_program(
            &program,
            "main",
            &[Value::Set(vec![
                Value::String("raid".to_owned()),
                Value::String("pvp".to_owned()),
                Value::String("raid".to_owned()),
            ])],
        ),
        Ok(Value::Int(2)),
    );
}

#[test]
fn script_function_registers_typed_fixed_array_native_with_engine() {
    let engine = vela_register_native_function_default_weights(
        vela_register_native_function_sum_weights(Engine::builder()),
    )
    .build()
    .expect("engine should build from macro fixed-array native functions");
    let program = compile_source!(
        engine,
        r#"
fn main(weights) {
    return game::sum_weights(weights) + game::default_weights().sum();
}
"#,
        "source should compile with macro registered fixed-array natives"
    );

    assert_eq!(
        engine.into_vm().run_program(
            &program,
            "main",
            &[Value::Array(vec![
                Value::Int(3),
                Value::Int(5),
                Value::Int(7),
            ])],
        ),
        Ok(Value::Int(27)),
    );
}

#[test]
fn script_function_registers_typed_hash_map_native_with_engine() {
    let engine = vela_register_native_function_score_total(Engine::builder())
        .build()
        .expect("engine should build from macro map native function");
    let program = compile_source!(
        engine,
        r#"
fn main(scores) {
    return game::score_total(scores);
}
"#,
        "source should compile with macro registered map native"
    );

    assert_eq!(
        engine.into_vm().run_program(
            &program,
            "main",
            &[Value::Map(
                [
                    ("daily".to_owned(), Value::Int(3)),
                    ("weekly".to_owned(), Value::Int(7)),
                ]
                .into(),
            )],
        ),
        Ok(Value::Int(10)),
    );
}

#[test]
fn script_function_registers_typed_btree_map_native_with_engine() {
    let engine = vela_register_native_function_ordered_score_summary(Engine::builder())
        .build()
        .expect("engine should build from macro ordered map native function");
    let program = compile_source!(
        engine,
        r#"
fn main(scores) {
    let summary = game::ordered_score_summary(scores);
    return summary.get_or("total", 0) + summary.get_or("daily", 0);
}
"#,
        "source should compile with macro registered ordered map native"
    );

    assert_eq!(
        engine.into_vm().run_program(
            &program,
            "main",
            &[Value::Map(
                [
                    ("daily".to_owned(), Value::Int(3)),
                    ("weekly".to_owned(), Value::Int(7)),
                ]
                .into(),
            )],
        ),
        Ok(Value::Int(13)),
    );
}

#[test]
fn script_function_registers_typed_f32_native_with_engine() {
    let engine = vela_register_native_function_scale_weight(Engine::builder())
        .build()
        .expect("engine should build from macro f32 native function");
    let program = compile_source!(
        engine,
        r#"
fn main() {
    return game::scale_weight(2.0);
}
"#,
        "source should compile with macro registered f32 native"
    );

    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(Value::Float(3.0)),
    );
}

#[test]
fn script_function_registers_typed_option_native_with_engine() {
    let engine =
        vela_register_native_function_optional_bonus(Engine::builder().with_standard_natives())
            .build()
            .expect("engine should build from macro option native function");
    let program = compile_source!(
        engine,
        r#"
fn main() {
    return game::optional_bonus(null) == null
        && game::optional_bonus(4) == 5
        && game::optional_bonus(option::none()) == null
        && game::optional_bonus(option::some(8)) == 9;
}
"#,
        "source should compile with macro registered option native"
    );

    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(Value::Bool(true)),
    );
}

#[test]
fn script_function_registers_typed_five_arg_native_with_engine() {
    let engine = vela_register_native_function_sum5(Engine::builder())
        .build()
        .expect("engine should build from macro five-arg native function");
    let program = compile_source!(
        engine,
        r#"
fn main() {
    return game::sum5(1, 2, 3, 4, 5);
}
"#,
        "source should compile with macro registered five-arg native"
    );

    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(Value::Int(15)),
    );
}

#[test]
fn script_function_registers_typed_six_arg_native_with_engine() {
    let engine = vela_register_native_function_sum6(Engine::builder())
        .build()
        .expect("engine should build from macro six-arg native function");
    let program = compile_source!(
        engine,
        r#"
fn main() {
    return game::sum6(1, 2, 3, 4, 5, 6);
}
"#,
        "source should compile with macro registered six-arg native"
    );

    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(Value::Int(21)),
    );
}

#[test]
fn script_function_registers_typed_result_native_with_engine() {
    let engine =
        vela_register_native_function_checked_bonus(Engine::builder().with_standard_natives())
            .build()
            .expect("engine should build from macro result native function");
    let program = compile_source!(
        engine,
        r#"
fn main() {
    let ok = game::checked_bonus(true);
    let err = game::checked_bonus(false);
    return result::unwrap_or(ok, 0) + result::unwrap_or(err, 4);
}
"#,
        "source should compile with macro registered result native"
    );

    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(Value::Int(13)),
    );
}

#[test]
fn script_function_registers_private_reflect_visible_metadata() {
    let engine = vela_register_native_function_debug_probe(
        Engine::builder().reflection_permissions(ReflectPermissionSet::all()),
    )
    .build()
    .expect("engine should build from macro private reflection metadata");
    let program = compile_source!(
        engine,
        r#"
fn main() {
    let probe = reflect::function("game::debug_probe");
    return reflect::has_function("game::debug_probe")
        && !probe.public
        && probe.access.reflect_visible
        && !probe.access.reflect_callable;
}
"#,
        "source should compile with macro registered private metadata"
    );
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine
            .into_vm()
            .run_program_with_host(&program, "main", &[], &mut host),
        Ok(Value::Bool(true)),
    );
    assert!(tx.patches().is_empty());
}

#[test]
fn script_context_function_registers_typed_native_with_engine() {
    let engine = vela_register_context_native_function_set_level(
        Engine::builder().grant_permission("player.write"),
    )
    .build()
    .expect("engine should build from macro context native function");
    let program = compile_source!(
        engine,
        r#"
fn main(player) {
    return game::set_level(player, 9);
}
"#,
        "source should compile with macro registered context native"
    );
    let player = HostRef::new(HostTypeId::new(1001), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine.into_vm().run_program_with_host(
            &program,
            "main",
            &[Value::HostRef(player)],
            &mut host,
        ),
        Ok(Value::Bool(true)),
    );
    assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Int(9)));
}

#[test]
fn script_context_function_alias_registers_renamed_native_with_stable_id() {
    let engine = vela_register_context_native_function_set_level_v2(
        Engine::builder().grant_permission("player.write"),
    )
    .build()
    .expect("engine should build from macro renamed context native function");
    let program = compile_source!(
        engine,
        r#"
fn main(player) {
    return game::set_level_v2(player, 11);
}
"#,
        "source should compile with macro registered renamed context native"
    );
    let registry = engine.registry();
    let registered = registry
        .functions()
        .find(|function| function.name == "game::set_level_v2")
        .expect("renamed context function should be reflected");
    assert_eq!(registered.id, function_id("game::set_level"));
    let player = HostRef::new(HostTypeId::new(1001), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine.into_vm().run_program_with_host(
            &program,
            "main",
            &[Value::HostRef(player)],
            &mut host,
        ),
        Ok(Value::Int(11)),
    );
    assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Int(11)));
}

#[test]
fn script_context_function_enforces_engine_permissions_before_patching() {
    let engine = vela_register_context_native_function_set_level(Engine::builder())
        .build()
        .expect("engine should build from macro context native function");
    let program = compile_source!(
        engine,
        r#"
fn main(player) {
    return game::set_level(player, 9);
}
"#,
        "source should compile with macro registered context native"
    );
    let player = HostRef::new(HostTypeId::new(1001), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut runtime = Runtime::new(engine, program);

    let error = runtime
        .call(
            "main",
            &[Value::HostRef(player)],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx,
        )
        .expect_err("missing macro-native permission should fail");

    assert_eq!(
        error.kind,
        VmErrorKind::PermissionDenied {
            native: "game::set_level".to_owned(),
            permission: "player.write".to_owned(),
        },
    );
    assert!(tx.patches().is_empty());
}

#[test]
fn script_context_function_charges_runtime_instruction_budget_before_patching() {
    let engine = vela_register_context_native_function_set_level(
        Engine::builder().grant_permission("player.write"),
    )
    .build()
    .expect("engine should build from macro context native function");
    let program = compile_source!(
        engine,
        r#"
fn main(player) {
    return game::set_level(player, 9);
}
"#,
        "source should compile with macro registered context native"
    );
    let player = HostRef::new(HostTypeId::new(1001), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut runtime = Runtime::new(engine, program);

    let error = runtime
        .call(
            "main",
            &[Value::HostRef(player)],
            CallOptions::new(2, usize::MAX, usize::MAX, usize::MAX),
            &mut adapter,
            &mut tx,
        )
        .expect_err("macro-native budget charge should fail");

    assert_eq!(
        error.kind,
        VmErrorKind::BudgetExceeded {
            budget: ExecutionBudgetKind::Instructions,
            limit: 2,
        },
    );
    assert!(tx.patches().is_empty());
}

#[test]
fn script_host_function_registers_typed_native_with_engine() {
    let engine = vela_register_host_native_function_set_score(
        Engine::builder().grant_permission("player.write"),
    )
    .build()
    .expect("engine should build from macro host native function");
    let program = compile_source!(
        engine,
        r#"
fn main(player) {
    return game::set_score(player, 12);
}
"#,
        "source should compile with macro registered host native"
    );
    let player = HostRef::new(HostTypeId::new(1001), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine.into_vm().run_program_with_host(
            &program,
            "main",
            &[Value::HostRef(player)],
            &mut host,
        ),
        Ok(Value::Int(12)),
    );
    assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Int(12)));
}

#[test]
fn script_host_function_alias_registers_renamed_native_with_stable_id() {
    let engine = vela_register_host_native_function_set_score_v2(
        Engine::builder().grant_permission("player.write"),
    )
    .build()
    .expect("engine should build from macro renamed host native function");
    let program = compile_source!(
        engine,
        r#"
fn main(player) {
    return game::set_score_v2(player, 14);
}
"#,
        "source should compile with macro registered renamed host native"
    );
    let registry = engine.registry();
    let registered = registry
        .functions()
        .find(|function| function.name == "game::set_score_v2")
        .expect("renamed host function should be reflected");
    assert_eq!(registered.id, function_id("game::set_score"));
    let player = HostRef::new(HostTypeId::new(1001), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine.into_vm().run_program_with_host(
            &program,
            "main",
            &[Value::HostRef(player)],
            &mut host,
        ),
        Ok(Value::Int(14)),
    );
    assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Int(14)));
}
