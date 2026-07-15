use super::*;

fn link_test_version(
    _engine: &Engine,
    version: vela_hot_reload::version::ProgramVersion,
) -> vela_hot_reload::version::ProgramVersion {
    assert!(
        version.linked_program().function_count() > 0,
        "test version should own linked bytecode"
    );
    version
}

#[test]
fn engine_compile_hot_reload_changed_file_reloads_module_root() {
    let root = unique_test_dir("hot_reload_changed_file");
    let reward_file = write_reward_modules(&root, "return grant() + 1;", 4);
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");

    write_reward_module(&reward_file, 9);
    let update = engine
        .compile_hot_reload_update_changed_file(&initial, &root, &reward_file)
        .expect("changed file update should compile");
    let mut runtime = HotReloadRuntime::new(initial);
    let report = runtime.apply_hot_update_report(update);

    assert!(report.accepted);
    assert_eq!(report.changed_functions, vec!["game::reward::grant"]);
    assert_eq!(report.changed_modules, vec!["game::reward"]);
    assert_eq!(
        report.impacted_modules,
        vec!["game::main".to_owned(), "game::reward".to_owned()]
    );
    let current = runtime.current();
    assert_eq!(
        engine
            .into_vm()
            .run_linked_program(current.linked_artifact(), "game::main::main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(10)))
    );
}

#[test]
fn engine_compile_hot_reload_changed_file_accepts_normalized_root_paths() {
    let root = unique_test_dir("hot_reload_changed_file_normalized_root");
    let reward_file = write_reward_modules(&root, "return grant();", 4);
    let root_with_current_segment = root.join(".");
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");

    write_reward_module(&reward_file, 8);
    let update = engine
        .compile_hot_reload_update_changed_file(&initial, &root_with_current_segment, &reward_file)
        .expect("changed file update should compile");
    let mut runtime = HotReloadRuntime::new(initial);
    let report = runtime.apply_hot_update_report(update);

    assert!(report.accepted);
    assert_eq!(report.changed_functions, vec!["game::reward::grant"]);
    let current = runtime.current();
    assert_eq!(
        engine
            .into_vm()
            .run_linked_program(current.linked_artifact(), "game::main::main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(8)))
    );
}

#[test]
fn engine_compile_hot_reload_changed_file_rejects_non_source_path() {
    let root = unique_test_dir("hot_reload_changed_file_invalid");
    let reward_file = write_reward_modules(&root, "return grant();", 4);
    let changed = root.join("ignored.txt");
    std::fs::write(&changed, "ignored").expect("write ignored file");
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");

    let error = engine
        .compile_hot_reload_update_changed_file(&initial, &root, &changed)
        .expect_err("non-source watcher path should be rejected");

    assert!(matches!(
        error.kind,
        EngineHotReloadSourceErrorKind::Source(crate::source::EngineSourceError {
            kind: EngineSourceErrorKind::InvalidSourcePath { .. }
        })
    ));
    assert!(reward_file.exists());
}

#[test]
fn engine_compile_hot_reload_changed_file_rejects_parent_dir_escape() {
    let root = unique_test_dir("hot_reload_changed_file_parent_escape");
    let reward_file = write_reward_modules(&root, "return grant();", 4);
    let changed = root.join("..").join("outside.vela");
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");

    let error = engine
        .compile_hot_reload_update_changed_file(&initial, &root, &changed)
        .expect_err("changed source path escaping the root should be rejected");

    assert!(matches!(
        error.kind,
        EngineHotReloadSourceErrorKind::Source(crate::source::EngineSourceError {
            kind: EngineSourceErrorKind::InvalidSourcePath { .. }
        })
    ));
    assert!(reward_file.exists());
}

#[test]
fn engine_compile_hot_reload_file_reports_source_errors() {
    let root = unique_test_dir("missing_hot_reload_file");
    let path = root.join("missing.vela");
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");

    let error = engine
        .compile_hot_reload_initial_file(&path)
        .expect_err("missing hot reload source file should fail");

    assert!(matches!(
        error.kind,
        EngineHotReloadSourceErrorKind::Source(_)
    ));
}

#[test]
fn engine_compile_file_reports_io_errors() {
    let root = unique_test_dir("missing_file");
    let path = root.join("missing.vela");
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");

    let error = engine
        .compile_file(&path)
        .expect_err("missing source file should fail");

    assert!(matches!(error.kind, EngineSourceErrorKind::Io { .. }));
}

#[test]
fn engine_exposes_registry_hot_reload_abi() {
    let player_key = TypeKey::new(TypeId::new(1), "Player");
    let method = HostMethodId::new(9);
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_type(
            TypeDesc::new(player_key.clone())
                .schema_hash(SchemaHash::new(0xfeed))
                .host_type(HostTypeId::new(1))
                .method(
                    MethodDesc::new(method, "grant_exp")
                        .effects(MethodEffectSet::host_write())
                        .access(MethodAccess::new().reflect_callable(true)),
                ),
        )
        .register_native_fn(
            NativeFunctionDesc::new("game::reward::grant", NativeFunctionId::new(22))
                .param("player", TypeHint::Host(player_key))
                .returns(TypeHint::unit())
                .effects(EffectSet::event_emit())
                .access(FunctionAccess::public().reflect_callable(true)),
            |_| Ok(OwnedValue::Unit),
        )
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_with_id(
            SourceId::new(1),
            r#"
fn main(player: Player) {
    player.grant_exp(10);
    return 1;
}
"#,
        )
        .expect("initial hot reload compile");
    let update = engine
        .compile_hot_reload_update_with_id(
            &initial,
            SourceId::new(2),
            r#"
fn main(player: Player) {
    player.grant_exp(11);
    return 2;
}
"#,
        )
        .expect("unchanged engine ABI should be hot-reload compatible");
    let mut runtime = HotReloadRuntime::new(initial);
    let version = runtime.apply_hot_update(update).expect("apply update");
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        state_values: None,
    };

    let mut budget = vela_vm::budget::ExecutionBudget::unbounded();
    assert_eq!(
        engine
            .into_vm()
            .run_linked_program_with_host_budget_and_caches(
                version.linked_artifact(),
                "main",
                &[OwnedValue::HostRef(host_ref)],
                &mut host,
                &mut budget,
                None
            ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );
}

#[test]
fn runtime_applies_engine_hot_reload_updates() {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_with_id(SourceId::new(1), "fn main() { return 1; }")
        .expect("initial hot reload compile");
    assert!(initial.linked_program().function_count() > 0);
    let update = engine
        .compile_hot_reload_update_with_id(&initial, SourceId::new(2), "fn main() { return 2; }")
        .expect("compatible update should compile");
    assert!(update.linked_program().function_count() > 0);
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );

    let report = runtime.apply_hot_update(update).expect("apply update");
    assert!(report.accepted);
    assert_eq!(report.changed_functions, vec!["main".to_owned()]);
    assert_eq!(
        runtime
            .hot_reload_version()
            .expect("current hot reload version")
            .id,
        report.to_version.expect("accepted version id")
    );
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );
}

#[test]
fn runtime_rebinds_vm_states_after_reload_image_swap() {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_with_id(
            SourceId::new(1),
            r#"
struct ServerState {
    level: i64,
    name: String,
}

state state: ServerState = ServerState { level: 0, name: "" };

fn make_state() {
    return ServerState { level: 5, name: "boot" };
}

fn bump(amount) {
    state.level += amount;
    return state.level;
}
"#,
        )
        .expect("initial hot reload compile");
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");
    let state = runtime
        .call("make_state", CallArgs::new(), CallOptions::unbounded())
        .expect("factory should run");
    runtime
        .set_state("main::state", state)
        .expect("script global should insert");

    runtime
        .stage_hot_reload_update(
            r#"
struct ServerState {
    level: i64,
    name: String,
}

state state: ServerState = ServerState { level: 0, name: "" };

fn make_state() {
    return ServerState { level: 0, name: "reload" };
}

fn bump(amount) {
    state.level += amount;
    return state.level + 100;
}
"#,
        )
        .expect("runtime supports source text update")
        .expect("stage source text update");
    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("pending report");

    assert!(report.accepted);
    assert_eq!(
        runtime
            .hot_reload_version()
            .expect("current hot reload version")
            .id,
        report.to_version.expect("accepted version id")
    );

    let bumped = runtime
        .call(
            "bump",
            CallArgs::from_positional([OwnedValue::Scalar(vela_common::ScalarValue::I64(2))]),
            CallOptions::unbounded(),
        )
        .expect("bump after reload should run");

    assert_eq!(
        runtime.value_to_owned(&bumped),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(107)))
    );
    assert_eq!(
        script_record_field(
            &runtime
                .state("main::state")
                .expect("script global should materialize")
                .expect("script global should exist"),
            "level",
        ),
        Some(&OwnedValue::Scalar(vela_common::ScalarValue::I64(7)))
    );
}

#[test]
fn reload_preserves_compatible_state_and_initializes_only_added_vm_state() {
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_with_id(
            SourceId::new(301),
            "state counter: i64 = 1; fn read() { return counter; }",
        )
        .expect("initial generation");
    let update = engine
        .compile_hot_reload_update_with_id(
            &initial,
            SourceId::new(302),
            "state added: i64 = 7; state counter: i64 = 999; fn read() { return counter + added; }",
        )
        .expect("compatible state update");
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime initializes");
    runtime
        .set_state("main::counter", 10_i64)
        .expect("set state");

    let report = runtime.apply_hot_update(update).expect("apply update");

    assert!(report.accepted);
    assert_eq!(report.added_states, ["main::added"]);
    assert_eq!(report.initializer_changed_states, ["main::counter"]);
    assert_eq!(
        runtime.state("main::counter"),
        Ok(Some(OwnedValue::from(10_i64)))
    );
    assert_eq!(
        runtime.state("main::added"),
        Ok(Some(OwnedValue::from(7_i64)))
    );
    let value = runtime
        .call("read", CallArgs::new(), CallOptions::unbounded())
        .expect("new generation reads both state cells");
    assert_eq!(runtime.value_to_owned(&value), Ok(OwnedValue::from(17_i64)));
}

#[test]
fn reload_rejects_public_state_export_breaks_without_publishing_image_or_state() {
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_with_id(
            SourceId::new(307),
            "pub state value: i64 = 1; fn read() { return value; }",
        )
        .expect("initial generation");
    let removal = engine.compile_hot_reload_update_with_id(
        &initial,
        SourceId::new(308),
        "fn read() { return 0; }",
    );
    let downgrade = engine.compile_hot_reload_update_with_id(
        &initial,
        SourceId::new(309),
        "state value: i64 = 1; fn read() { return value; }",
    );
    let initial_id = initial.id;
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime initializes");
    runtime
        .set_state("main::value", 11_i64)
        .expect("state replacement");

    for (update, expected_code) in [
        (removal, "reload.state.export_removed"),
        (downgrade, "reload.state.visibility_downgraded"),
    ] {
        let report = runtime
            .apply_hot_update_result_report(hot_reload_result(update))
            .expect("runtime returns a rejection report");

        assert!(!report.accepted);
        assert_eq!(report.errors[0].code, expected_code);
        assert_eq!(
            runtime.hot_reload_version().expect("active version").id,
            initial_id
        );
        assert_eq!(
            runtime.state("main::value"),
            Ok(Some(OwnedValue::from(11_i64)))
        );
        let value = runtime
            .call("read", CallArgs::new(), CallOptions::unbounded())
            .expect("old image remains callable");
        assert_eq!(runtime.value_to_owned(&value), Ok(OwnedValue::from(11_i64)));
    }
}

#[test]
fn added_state_initializer_failure_rolls_back_image_and_state_publication() {
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_with_id(
            SourceId::new(303),
            "state counter: i64 = 3; fn read() { return counter; }",
        )
        .expect("initial generation");
    let initial_id = initial.id;
    let update = engine
        .compile_hot_reload_update_with_id(
            &initial,
            SourceId::new(304),
            "fn recurse() -> i64 { return recurse(); } state added: i64 = recurse(); state counter: i64 = 3; fn read() { return counter + 100; }",
        )
        .expect("statically valid update");
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime initializes");

    let report = runtime
        .apply_hot_update(update)
        .expect("reload reports rejection");

    assert!(!report.accepted);
    assert_eq!(report.errors[0].code, "reload.state.initializer_failed");
    assert_eq!(
        runtime.hot_reload_version().expect("active version").id,
        initial_id
    );
    assert_eq!(runtime.state("main::added"), Ok(None));
    let value = runtime
        .call("read", CallArgs::new(), CallOptions::unbounded())
        .expect("old generation remains callable");
    assert_eq!(runtime.value_to_owned(&value), Ok(OwnedValue::from(3_i64)));
}

#[test]
fn shared_update_initializes_added_state_independently_per_runtime() {
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_with_id(
            SourceId::new(305),
            "state existing: i64 = 1; fn read() { return existing; }",
        )
        .expect("initial generation");
    let update = engine
        .compile_hot_reload_update_with_id(
            &initial,
            SourceId::new(306),
            "state added: i64 = 7; state existing: i64 = 1; fn read() { return existing + added; }",
        )
        .expect("compatible update");
    let mut first =
        Runtime::from_hot_reload_version(engine.clone(), initial.clone()).expect("first runtime");
    let mut second = Runtime::from_hot_reload_version(engine, initial).expect("second runtime");

    assert!(
        first
            .apply_hot_update(update.clone())
            .expect("first apply")
            .accepted
    );
    assert!(
        second
            .apply_hot_update(update)
            .expect("second apply")
            .accepted
    );
    first
        .set_state("main::added", 20_i64)
        .expect("first update");

    assert_eq!(
        first.state("main::added"),
        Ok(Some(OwnedValue::from(20_i64)))
    );
    assert_eq!(
        second.state("main::added"),
        Ok(Some(OwnedValue::from(7_i64)))
    );
}

fn script_record_field<'value>(
    value: &'value OwnedValue,
    field: &str,
) -> Option<&'value OwnedValue> {
    let OwnedValue::Record { fields, .. } = value else {
        return None;
    };
    fields.get(field)
}

#[test]
fn runtime_stages_engine_hot_reload_until_check_reload_safe_point() {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_with_id(SourceId::new(1), "fn main() { return 1; }")
        .expect("initial hot reload compile");
    let update = engine
        .compile_hot_reload_update_with_id(&initial, SourceId::new(2), "fn main() { return 2; }")
        .expect("compatible update should compile");
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    runtime
        .stage_hot_update(update)
        .expect("stage pending update");
    assert!(
        runtime
            .has_pending_hot_update()
            .expect("hot reload runtime should report pending update")
    );
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("pending report");

    assert!(report.accepted);
    assert!(
        report
            .version()
            .expect("accepted report should carry version")
            .linked_program()
            .function_count()
            > 0
    );
    assert_eq!(report.changed_functions, vec!["main".to_owned()]);
    assert!(
        !runtime
            .has_pending_hot_update()
            .expect("pending update should be consumed")
    );
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );
}

#[test]
fn runtime_stages_source_text_hot_reload_until_check_reload_safe_point() {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_with_id(SourceId::new(1), "fn main() { return 1; }")
        .expect("initial hot reload compile");
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    runtime
        .stage_hot_reload_update("fn main() { return 2; }")
        .expect("runtime supports source text update")
        .expect("stage source text update");
    assert!(
        runtime
            .has_pending_hot_update()
            .expect("hot reload runtime should report pending update")
    );
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("pending report");

    assert!(report.accepted);
    assert_eq!(report.changed_functions, vec!["main".to_owned()]);
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );
}

#[test]
fn runtime_stages_source_text_hot_reload_rejection_until_check_reload_safe_point() {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_with_id(SourceId::new(1), "pub fn main() -> i64 { return 1; }")
        .expect("initial hot reload compile");
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    runtime
        .stage_hot_reload_update("pub fn main() -> f64 { return 2.0; }")
        .expect("runtime supports source text update")
        .expect("stage rejected source text update");
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("pending report");

    assert!(!report.accepted);
    let HotReloadErrorKind::ChangedFunctionReturnAbi {
        function,
        old,
        new,
        source_span,
    } = &report.errors[0].error.kind
    else {
        panic!("expected changed function return ABI");
    };
    assert_eq!(function, "main");
    assert_eq!(old.as_deref(), Some("i64"));
    assert_eq!(new.as_deref(), Some("f64"));
    assert!(source_span.is_some());
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
}

#[test]
fn runtime_tick_boundary_safe_point_consumes_staged_reload() {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_with_id(SourceId::new(1), "fn main() { return 1; }")
        .expect("initial hot reload compile");
    let update = engine
        .compile_hot_reload_update_with_id(&initial, SourceId::new(2), "fn main() { return 2; }")
        .expect("compatible update should compile");
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    runtime
        .stage_hot_update(update)
        .expect("stage pending update");
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );

    let report = runtime
        .check_reload_at_tick_boundary()
        .expect("check reload at tick boundary")
        .expect("pending report");

    assert!(report.accepted);
    assert_eq!(report.changed_functions, vec!["main".to_owned()]);
    assert!(
        !runtime
            .has_pending_hot_update()
            .expect("pending update should be consumed")
    );
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );
    assert_eq!(
        runtime
            .check_reload_at_tick_boundary()
            .expect("check empty tick boundary"),
        None
    );
}

#[test]
fn runtime_tick_boundary_safe_point_reports_staged_reload_rejection() {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_with_id(SourceId::new(1), "pub fn main() -> i64 { return 1; }")
        .expect("initial hot reload compile");
    let update = hot_reload_result(engine.compile_hot_reload_update_with_id(
        &initial,
        SourceId::new(2),
        "pub fn main() -> f64 { return 2.0; }",
    ))
    .expect_err("return hint change should be rejected");
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    runtime
        .stage_hot_update_result(Err(update))
        .expect("stage rejected update");
    let report = runtime
        .check_reload_at_tick_boundary()
        .expect("check reload at tick boundary")
        .expect("pending report");

    assert!(!report.accepted);
    let HotReloadErrorKind::ChangedFunctionReturnAbi {
        function,
        old,
        new,
        source_span,
    } = &report.errors[0].error.kind
    else {
        panic!("expected changed function return ABI");
    };
    assert_eq!(function, "main");
    assert_eq!(old.as_deref(), Some("i64"));
    assert_eq!(new.as_deref(), Some("f64"));
    assert!(source_span.is_some());
    assert!(
        !runtime
            .has_pending_hot_update()
            .expect("pending rejection should be consumed")
    );
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
}

#[test]
fn runtime_tick_boundary_safe_point_reports_staged_module_export_rejection() {
    let initial_abi = HotReloadAbi::empty().module(
        ModuleAbi::new("host::reward").export(ModuleExportAbi::function("grant_reward", 11)),
    );
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial =
        compile_initial_with_abi(SourceId::new(1), "fn main() { return 1; }", initial_abi)
            .map(|version| link_test_version(&engine, version))
            .expect("initial hot reload compile");
    let update_abi = HotReloadAbi::empty().module(ModuleAbi::new("host::reward"));
    let update = compile_update_with_abi(
        &initial,
        SourceId::new(2),
        "fn main() { return 2; }",
        update_abi,
    )
    .expect_err("module export ABI change should be rejected");
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");
    runtime
        .stage_hot_update_result(Err(update))
        .expect("stage rejected module export update");
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );

    let report = runtime
        .check_reload_at_tick_boundary()
        .expect("tick boundary should report staged module rejection")
        .expect("staged module export rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.module.changed_abi");
    assert_eq!(report.errors[0].target.as_deref(), Some("host::reward"));
    assert_changed_module_abi_repair_hint(&report);
    let HotReloadErrorKind::ChangedModuleAbi { old, new, .. } = &report.errors[0].error.kind else {
        panic!("expected changed module ABI");
    };
    assert_eq!(old, &vec![ModuleExportAbi::function("grant_reward", 11)]);
    assert_eq!(new, &Vec::<ModuleExportAbi>::new());
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
}

#[test]
fn runtime_tick_boundary_safe_point_reports_staged_removed_function_abi_rejection() {
    let initial_abi = HotReloadAbi::empty().function(FunctionAbi::new(
        "host::reward::grant",
        EffectAbi::host_read(),
        AccessAbi::new(true, true),
    ));
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial =
        compile_initial_with_abi(SourceId::new(1), "fn main() { return 1; }", initial_abi)
            .map(|version| link_test_version(&engine, version))
            .expect("initial hot reload compile");
    let update = compile_update_with_abi(
        &initial,
        SourceId::new(2),
        "fn main() { return 2; }",
        HotReloadAbi::empty(),
    )
    .expect_err("removed function ABI should be rejected");
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");
    runtime
        .stage_hot_update_result(Err(update))
        .expect("stage rejected removed function update");
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );

    let report = runtime
        .check_reload_at_tick_boundary()
        .expect("tick boundary should report staged function rejection")
        .expect("staged removed function ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.function.removed_abi");
    assert_eq!(
        report.errors[0].target.as_deref(),
        Some("host::reward::grant")
    );
    assert_removed_function_abi_repair_hint(&report);
    let HotReloadErrorKind::RemovedFunctionAbi { function, .. } = &report.errors[0].error.kind
    else {
        panic!("expected removed function ABI");
    };
    assert_eq!(function, "host::reward::grant");
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
}

#[test]
fn runtime_tick_boundary_safe_point_reports_staged_removed_method_abi_rejection() {
    let initial_abi = HotReloadAbi::empty().method(MethodAbi::new(
        "Player",
        "grant_exp",
        EffectAbi::host_write(),
        AccessAbi::new(true, true),
    ));
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial =
        compile_initial_with_abi(SourceId::new(1), "fn main() { return 1; }", initial_abi)
            .map(|version| link_test_version(&engine, version))
            .expect("initial hot reload compile");
    let update = compile_update_with_abi(
        &initial,
        SourceId::new(2),
        "fn main() { return 2; }",
        HotReloadAbi::empty(),
    )
    .expect_err("removed method ABI should be rejected");
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");
    runtime
        .stage_hot_update_result(Err(update))
        .expect("stage rejected removed method update");
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );

    let report = runtime
        .check_reload_at_tick_boundary()
        .expect("tick boundary should report staged method rejection")
        .expect("staged removed method ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.method.removed_abi");
    assert_eq!(report.errors[0].target.as_deref(), Some("Player.grant_exp"));
    assert_removed_method_abi_repair_hint(&report);
    let HotReloadErrorKind::RemovedMethodAbi {
        type_name, method, ..
    } = &report.errors[0].error.kind
    else {
        panic!("expected removed method ABI");
    };
    assert_eq!(type_name, "Player");
    assert_eq!(method, "grant_exp");
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
}

#[test]
fn runtime_tick_boundary_safe_point_reports_staged_removed_module_rejection() {
    let initial_abi = HotReloadAbi::empty().module(ModuleAbi::new("host::reward"));
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial =
        compile_initial_with_abi(SourceId::new(1), "fn main() { return 1; }", initial_abi)
            .map(|version| link_test_version(&engine, version))
            .expect("initial hot reload compile");
    let update = compile_update_with_abi(
        &initial,
        SourceId::new(2),
        "fn main() { return 2; }",
        HotReloadAbi::empty(),
    )
    .expect_err("removed module ABI should be rejected");
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");
    runtime
        .stage_hot_update_result(Err(update))
        .expect("stage rejected removed module update");
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );

    let report = runtime
        .check_reload_at_tick_boundary()
        .expect("tick boundary should report staged module rejection")
        .expect("staged removed module rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.module.removed_abi");
    assert_eq!(report.errors[0].target.as_deref(), Some("host::reward"));
    assert_removed_module_abi_repair_hint(&report);
    let HotReloadErrorKind::RemovedModuleAbi { module, .. } = &report.errors[0].error.kind else {
        panic!("expected removed module ABI");
    };
    assert_eq!(module, "host::reward");
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
}

#[test]
fn explicit_event_end_reload_check_consumes_staged_update_after_call() {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_with_id(SourceId::new(1), "fn main() { return 1; }")
        .expect("initial hot reload compile");
    let update = engine
        .compile_hot_reload_update_with_id(&initial, SourceId::new(2), "fn main() { return 2; }")
        .expect("compatible update should compile");
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    runtime
        .stage_hot_update(update)
        .expect("stage pending update");
    let value = runtime
        .call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx)
        .expect("event call should run");
    let reload = runtime
        .check_reload()
        .expect("reload check should run")
        .expect("staged reload should be consumed");

    assert_eq!(value, OwnedValue::Scalar(vela_common::ScalarValue::I64(1)));
    assert!(reload.accepted);
    assert_eq!(reload.changed_functions, vec!["main".to_owned()]);
    assert!(
        !runtime
            .has_pending_hot_update()
            .expect("pending update should be consumed")
    );
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );
}

#[test]
fn runtime_event_end_safe_point_keeps_nested_calls_on_old_version_until_return() {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_with_id(
            SourceId::new(1),
            r#"
fn helper() {
    return 1;
}

fn main() {
    return helper();
}
"#,
        )
        .expect("initial hot reload compile");
    let update = engine
        .compile_hot_reload_update_with_id(
            &initial,
            SourceId::new(2),
            r#"
fn helper() {
    return 2;
}

fn main() {
    return helper();
}
"#,
        )
        .expect("compatible update should compile");
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    runtime
        .stage_hot_update(update)
        .expect("stage pending update");
    let value = runtime
        .call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx)
        .expect("event call should run on the old version");
    let reload = runtime
        .check_reload()
        .expect("reload check should run")
        .expect("staged reload should be consumed");

    assert_eq!(value, OwnedValue::Scalar(vela_common::ScalarValue::I64(1)));
    assert!(reload.accepted);
    assert_eq!(
        reload.changed_functions,
        vec!["helper".to_owned(), "main".to_owned()]
    );
    assert!(
        !runtime
            .has_pending_hot_update()
            .expect("pending update should be consumed")
    );
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );
}

#[test]
fn explicit_event_end_reload_check_reports_staged_rejection_after_call() {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_with_id(SourceId::new(1), "pub fn main() -> i64 { return 1; }")
        .expect("initial hot reload compile");
    let update = hot_reload_result(engine.compile_hot_reload_update_with_id(
        &initial,
        SourceId::new(2),
        "pub fn main() -> f64 { return 2.0; }",
    ))
    .expect_err("return hint change should be rejected");
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    runtime
        .stage_hot_update_result(Err(update))
        .expect("stage rejected update");
    let value = runtime
        .call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx)
        .expect("event call should run before reporting reload rejection");
    let reload = runtime
        .check_reload()
        .expect("reload check should run")
        .expect("staged rejection should be consumed");

    assert_eq!(value, OwnedValue::Scalar(vela_common::ScalarValue::I64(1)));
    assert!(!reload.accepted);
    let HotReloadErrorKind::ChangedFunctionReturnAbi {
        function,
        old,
        new,
        source_span,
    } = &reload.errors[0].error.kind
    else {
        panic!("expected changed function return ABI");
    };
    assert_eq!(function, "main");
    assert_eq!(old.as_deref(), Some("i64"));
    assert_eq!(new.as_deref(), Some("f64"));
    assert!(source_span.is_some());
    assert!(
        !runtime
            .has_pending_hot_update()
            .expect("pending rejection should be consumed")
    );
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
}

#[test]
fn runtime_checks_reload_at_explicit_safe_point() {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_with_id(
            SourceId::new(1),
            r#"
fn main(player: Player) {
    player.level += 1;
    return player.level;
}
"#,
        )
        .expect("initial hot reload compile");
    let update = engine
        .compile_hot_reload_update_with_id(
            &initial,
            SourceId::new(2),
            r#"
fn main(player: Player) {
    player.level += 2;
    return player.level + 100;
}
"#,
        )
        .expect("compatible update should compile");
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let level_path = HostPath::new(host_ref).field(FieldId::new(1));
    let mut adapter = MockStateAdapter::new();
    adapter.insert_diagnostic_path_value(
        level_path,
        HostValue::Scalar(vela_common::ScalarValue::I64(10)),
    );
    let mut tx = HostAccess::new();

    assert_eq!(
        runtime.call_raw(
            "main",
            &[OwnedValue::HostRef(host_ref)],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx,
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(11)))
    );
    runtime
        .stage_hot_update(update)
        .expect("stage pending update");

    let report = runtime
        .check_reload_at_tick_boundary()
        .expect("reload check should run")
        .expect("pending update should be consumed");
    assert!(report.accepted);
    assert_eq!(report.changed_functions, vec!["main".to_owned()]);

    let mut next_tx = HostAccess::new();
    assert_eq!(
        runtime.call_raw(
            "main",
            &[OwnedValue::HostRef(host_ref)],
            CallOptions::unbounded(),
            &mut adapter,
            &mut next_tx,
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(113)))
    );
}

#[test]
fn runtime_write_error_does_not_consume_pending_reload() {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_with_id(
            SourceId::new(1),
            r#"
fn main(player: Player) {
    player.level += 1;
    return player.level;
}
"#,
        )
        .expect("initial hot reload compile");
    let update = engine
        .compile_hot_reload_update_with_id(
            &initial,
            SourceId::new(2),
            r#"
fn main(player: Player) {
    player.level += 2;
    return player.level;
}
"#,
        )
        .expect("compatible update should compile");
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let level_path = HostPath::new(host_ref).field(FieldId::new(1));
    let mut adapter = MockStateAdapter::new();
    adapter.insert_diagnostic_path_value(
        level_path.clone(),
        HostValue::Scalar(vela_common::ScalarValue::I64(10)),
    );
    runtime
        .stage_hot_update(update)
        .expect("stage pending update");
    adapter.deny_diagnostic_path_write(level_path.clone());
    let mut tx = HostAccess::new();

    let error = runtime
        .call_raw(
            "main",
            &[OwnedValue::HostRef(host_ref)],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx,
        )
        .expect_err("denied host write should fail during call");

    assert!(matches!(
        error.kind(),
        VmErrorKind::Host(HostErrorKind::PermissionDenied {
            path,
            action: "write",
        }) if path == level_path
    ));
    assert!(
        runtime
            .has_pending_hot_update()
            .expect("failed write should not consume pending reload")
    );
}
