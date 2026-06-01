use super::*;

#[test]
fn script_function_registers_typed_native_with_engine() {
    let engine =
        vela_register_native_function_grant_bonus(Engine::builder().grant_permission("bonus.read"))
            .build()
            .expect("engine should build from macro native function");
    let root = unique_test_dir("script_function_native");
    std::fs::create_dir_all(&root).expect("create temp source dir");
    let source = root.join("main.vela");
    std::fs::write(
        &source,
        r#"
fn main() {
    return game.grant_bonus(6, 7);
}
"#,
    )
    .expect("write source");
    let program = engine
        .compile_file(&source)
        .expect("source should compile with macro registered native");

    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(Value::Int(42)),
    );
    std::fs::remove_dir_all(root).expect("clean temp source dir");
}

#[test]
fn script_function_registers_typed_set_native_with_engine() {
    let engine = vela_register_native_function_count_labels(Engine::builder())
        .build()
        .expect("engine should build from macro set native function");
    let root = unique_test_dir("script_function_set_native");
    std::fs::create_dir_all(&root).expect("create temp source dir");
    let source = root.join("main.vela");
    std::fs::write(
        &source,
        r#"
fn main(labels) {
    return game.count_labels(labels);
}
"#,
    )
    .expect("write source");
    let program = engine
        .compile_file(&source)
        .expect("source should compile with macro registered set native");

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
    std::fs::remove_dir_all(root).expect("clean temp source dir");
}

#[test]
fn script_function_registers_typed_hash_set_native_with_engine() {
    let engine = vela_register_native_function_count_unordered_labels(Engine::builder())
        .build()
        .expect("engine should build from macro unordered set native function");
    let root = unique_test_dir("script_function_hash_set_native");
    std::fs::create_dir_all(&root).expect("create temp source dir");
    let source = root.join("main.vela");
    std::fs::write(
        &source,
        r#"
fn main(labels) {
    return game.count_unordered_labels(labels);
}
"#,
    )
    .expect("write source");
    let program = engine
        .compile_file(&source)
        .expect("source should compile with macro registered unordered set native");

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
    std::fs::remove_dir_all(root).expect("clean temp source dir");
}

#[test]
fn script_function_registers_typed_fixed_array_native_with_engine() {
    let engine = vela_register_native_function_default_weights(
        vela_register_native_function_sum_weights(Engine::builder()),
    )
    .build()
    .expect("engine should build from macro fixed-array native functions");
    let root = unique_test_dir("script_function_fixed_array_native");
    std::fs::create_dir_all(&root).expect("create temp source dir");
    let source = root.join("main.vela");
    std::fs::write(
        &source,
        r#"
fn main(weights) {
    return game.sum_weights(weights) + game.default_weights().sum();
}
"#,
    )
    .expect("write source");
    let program = engine
        .compile_file(&source)
        .expect("source should compile with macro registered fixed-array natives");

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
    std::fs::remove_dir_all(root).expect("clean temp source dir");
}

#[test]
fn script_function_registers_typed_hash_map_native_with_engine() {
    let engine = vela_register_native_function_score_total(Engine::builder())
        .build()
        .expect("engine should build from macro map native function");
    let root = unique_test_dir("script_function_hash_map_native");
    std::fs::create_dir_all(&root).expect("create temp source dir");
    let source = root.join("main.vela");
    std::fs::write(
        &source,
        r#"
fn main(scores) {
    return game.score_total(scores);
}
"#,
    )
    .expect("write source");
    let program = engine
        .compile_file(&source)
        .expect("source should compile with macro registered map native");

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
    std::fs::remove_dir_all(root).expect("clean temp source dir");
}

#[test]
fn script_function_registers_typed_btree_map_native_with_engine() {
    let engine = vela_register_native_function_ordered_score_summary(Engine::builder())
        .build()
        .expect("engine should build from macro ordered map native function");
    let root = unique_test_dir("script_function_btree_map_native");
    std::fs::create_dir_all(&root).expect("create temp source dir");
    let source = root.join("main.vela");
    std::fs::write(
        &source,
        r#"
fn main(scores) {
    let summary = game.ordered_score_summary(scores);
    return summary.get_or("total", 0) + summary.get_or("daily", 0);
}
"#,
    )
    .expect("write source");
    let program = engine
        .compile_file(&source)
        .expect("source should compile with macro registered ordered map native");

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
    std::fs::remove_dir_all(root).expect("clean temp source dir");
}

#[test]
fn script_function_registers_typed_f32_native_with_engine() {
    let engine = vela_register_native_function_scale_weight(Engine::builder())
        .build()
        .expect("engine should build from macro f32 native function");
    let root = unique_test_dir("script_function_f32_native");
    std::fs::create_dir_all(&root).expect("create temp source dir");
    let source = root.join("main.vela");
    std::fs::write(
        &source,
        r#"
fn main() {
    return game.scale_weight(2.0);
}
"#,
    )
    .expect("write source");
    let program = engine
        .compile_file(&source)
        .expect("source should compile with macro registered f32 native");

    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(Value::Float(3.0)),
    );
    std::fs::remove_dir_all(root).expect("clean temp source dir");
}

#[test]
fn script_function_registers_typed_option_native_with_engine() {
    let engine =
        vela_register_native_function_optional_bonus(Engine::builder().with_standard_natives())
            .build()
            .expect("engine should build from macro option native function");
    let root = unique_test_dir("script_function_option_native");
    std::fs::create_dir_all(&root).expect("create temp source dir");
    let source = root.join("main.vela");
    std::fs::write(
        &source,
        r#"
fn main() {
    return game.optional_bonus(null) == null
        && game.optional_bonus(4) == 5
        && game.optional_bonus(option.none()) == null
        && game.optional_bonus(option.some(8)) == 9;
}
"#,
    )
    .expect("write source");
    let program = engine
        .compile_file(&source)
        .expect("source should compile with macro registered option native");

    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(Value::Bool(true)),
    );
    std::fs::remove_dir_all(root).expect("clean temp source dir");
}

#[test]
fn script_function_registers_typed_five_arg_native_with_engine() {
    let engine = vela_register_native_function_sum5(Engine::builder())
        .build()
        .expect("engine should build from macro five-arg native function");
    let root = unique_test_dir("script_function_five_arg_native");
    std::fs::create_dir_all(&root).expect("create temp source dir");
    let source = root.join("main.vela");
    std::fs::write(
        &source,
        r#"
fn main() {
    return game.sum5(1, 2, 3, 4, 5);
}
"#,
    )
    .expect("write source");
    let program = engine
        .compile_file(&source)
        .expect("source should compile with macro registered five-arg native");

    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(Value::Int(15)),
    );
    std::fs::remove_dir_all(root).expect("clean temp source dir");
}

#[test]
fn script_function_registers_typed_six_arg_native_with_engine() {
    let engine = vela_register_native_function_sum6(Engine::builder())
        .build()
        .expect("engine should build from macro six-arg native function");
    let root = unique_test_dir("script_function_six_arg_native");
    std::fs::create_dir_all(&root).expect("create temp source dir");
    let source = root.join("main.vela");
    std::fs::write(
        &source,
        r#"
fn main() {
    return game.sum6(1, 2, 3, 4, 5, 6);
}
"#,
    )
    .expect("write source");
    let program = engine
        .compile_file(&source)
        .expect("source should compile with macro registered six-arg native");

    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(Value::Int(21)),
    );
    std::fs::remove_dir_all(root).expect("clean temp source dir");
}

#[test]
fn script_function_registers_typed_result_native_with_engine() {
    let engine =
        vela_register_native_function_checked_bonus(Engine::builder().with_standard_natives())
            .build()
            .expect("engine should build from macro result native function");
    let root = unique_test_dir("script_function_result_native");
    std::fs::create_dir_all(&root).expect("create temp source dir");
    let source = root.join("main.vela");
    std::fs::write(
        &source,
        r#"
fn main() {
    let ok = game.checked_bonus(true);
    let err = game.checked_bonus(false);
    return result.unwrap_or(ok, 0) + result.unwrap_or(err, 4);
}
"#,
    )
    .expect("write source");
    let program = engine
        .compile_file(&source)
        .expect("source should compile with macro registered result native");

    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(Value::Int(13)),
    );
    std::fs::remove_dir_all(root).expect("clean temp source dir");
}

#[test]
fn script_context_function_registers_typed_native_with_engine() {
    let engine = vela_register_context_native_function_set_level(
        Engine::builder().grant_permission("player.write"),
    )
    .build()
    .expect("engine should build from macro context native function");
    let root = unique_test_dir("script_context_function_native");
    std::fs::create_dir_all(&root).expect("create temp source dir");
    let source = root.join("main.vela");
    std::fs::write(
        &source,
        r#"
fn main(player) {
    return game.set_level(player, 9);
}
"#,
    )
    .expect("write source");
    let program = engine
        .compile_file(&source)
        .expect("source should compile with macro registered context native");
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
    std::fs::remove_dir_all(root).expect("clean temp source dir");
}

#[test]
fn script_host_function_registers_typed_native_with_engine() {
    let engine = vela_register_host_native_function_set_score(
        Engine::builder().grant_permission("player.write"),
    )
    .build()
    .expect("engine should build from macro host native function");
    let root = unique_test_dir("script_host_function_native");
    std::fs::create_dir_all(&root).expect("create temp source dir");
    let source = root.join("main.vela");
    std::fs::write(
        &source,
        r#"
fn main(player) {
    return game.set_score(player, 12);
}
"#,
    )
    .expect("write source");
    let program = engine
        .compile_file(&source)
        .expect("source should compile with macro registered host native");
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
    std::fs::remove_dir_all(root).expect("clean temp source dir");
}
