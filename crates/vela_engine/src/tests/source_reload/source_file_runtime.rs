use super::*;

#[test]
fn runtime_compiles_hot_reload_update_from_active_version() {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial(SourceId::new(1), "fn main() { return 1; }")
        .expect("initial hot reload compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    let first_update = runtime
        .compile_hot_reload_update(
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
        .expect("runtime should be hot-reload enabled")
        .expect("compatible update should compile");
    let first_report = runtime
        .apply_hot_update(first_update)
        .expect("runtime should apply first update");
    assert!(first_report.accepted);
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Int(2))
    );

    let rejected_update = runtime
        .compile_hot_reload_update(SourceId::new(3), "fn main() { return 3; }")
        .expect("runtime should be hot-reload enabled");
    let error = rejected_update.expect_err("active helper removal should be rejected");
    assert!(matches!(
        error.kind,
        HotReloadErrorKind::RemovedFunction { ref function } if function == "helper"
    ));
}

#[test]
fn runtime_compiles_hot_reload_update_file_from_active_version() {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial(SourceId::new(1), "fn main() { return 1; }")
        .expect("initial hot reload compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock should be after epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "vela-runtime-hot-reload-{pid}-{unique}.vela",
        pid = std::process::id()
    ));
    std::fs::write(&path, "fn main() { return 5; }").expect("update file should write");

    let update = runtime
        .compile_hot_reload_update_file(&path)
        .expect("runtime should be hot-reload enabled")
        .expect("file update should compile");
    std::fs::remove_file(&path).expect("update file should clean up");
    let report = runtime
        .apply_hot_update(update)
        .expect("runtime should apply file update");
    assert!(report.accepted);

    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Int(5))
    );
}

#[test]
fn runtime_stages_hot_reload_file_until_check_reload_safe_point() {
    let root = unique_test_dir("runtime_stage_hot_reload_file");
    std::fs::create_dir_all(&root).expect("create temp source dir");
    let path = root.join("main.vela");
    std::fs::write(&path, "fn main() { return 1; }").expect("write initial source");
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_file(&path)
        .expect("initial hot reload file compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Int(1))
    );

    std::fs::write(&path, "fn main() { return 5; }").expect("write updated source");
    runtime
        .stage_hot_reload_update_file(&path)
        .expect("runtime should be hot-reload enabled")
        .expect("file update should stage");
    assert!(
        runtime
            .has_pending_hot_update()
            .expect("file update should be pending")
    );
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Int(1))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged file report");

    assert!(report.accepted);
    assert_eq!(report.changed_functions, vec!["main"]);
    assert!(
        !runtime
            .has_pending_hot_update()
            .expect("safe point should consume file update")
    );
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Int(5))
    );
}

#[test]
fn runtime_stages_source_file_private_helper_addition_until_safe_point() {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let mut runtime = runtime_from_hot_reload_source(engine, "fn main() { return 1; }");
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    stage_source_update(
        &mut runtime,
        r#"
fn helper() {
    return 7;
}

fn main() {
    return helper();
}
"#,
    );
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Int(1))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged helper report");

    assert!(report.accepted);
    assert_eq!(report.changed_functions, vec!["helper", "main"]);
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Int(7))
    );
}

#[test]
fn runtime_stages_source_file_public_function_addition_until_safe_point() {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let mut runtime = runtime_from_hot_reload_source(engine, "pub fn main() { return 1; }");
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    stage_source_update(
        &mut runtime,
        r#"
pub fn helper() {
    return 7;
}

pub fn main() {
    return helper();
}
"#,
    );
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Int(1))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged public function report");

    assert!(report.accepted);
    assert_eq!(report.changed_functions, vec!["helper", "main"]);
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Int(7))
    );
    assert_eq!(
        runtime.call_raw(
            "helper",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(7))
    );
}

#[test]
fn runtime_stages_source_file_removed_function_rejection_until_safe_point() {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let mut runtime = runtime_from_hot_reload_source(
        engine,
        r#"
fn helper() {
    return 7;
}

fn main() {
    return helper();
}
"#,
    );
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    stage_source_update(
        &mut runtime,
        r#"
fn main() {
    return 3;
}
"#,
    );
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Int(7))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged removed function rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.function.removed");
    assert_eq!(report.errors[0].target.as_deref(), Some("helper"));
    let HotReloadErrorKind::RemovedFunction { function } = &report.errors[0].error.kind else {
        panic!("expected removed script function");
    };
    assert_eq!(function, "helper");
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Int(7))
    );
}
