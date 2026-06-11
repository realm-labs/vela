use super::*;

#[test]
fn runtime_preserves_program_when_engine_hot_reload_update_is_rejected() {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .hot_reload_policy(HotReloadPolicy::locked_down())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial(SourceId::new(1), "fn main() { return 1; }")
        .expect("initial hot reload compile");
    let update = engine.compile_hot_reload_update(
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
    );
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    let report = runtime
        .apply_hot_update_result_report(update)
        .expect("runtime should return rejection report");
    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
}

#[test]
fn runtime_rejects_hot_update_when_not_created_from_version() {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial(SourceId::new(1), "fn main() { return 1; }")
        .expect("initial hot reload compile");
    let update = engine
        .compile_hot_reload_update(&initial, SourceId::new(2), "fn main() { return 2; }")
        .expect("compatible update should compile");
    let mut runtime = Runtime::new(engine, initial.to_unlinked_program());

    assert!(matches!(
        runtime.apply_hot_update(update),
        Err(error) if error.kind == EngineErrorKind::RuntimeNotHotReloadEnabled
    ));
}

#[test]
fn runtime_rejects_compile_update_when_not_created_from_version() {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial(SourceId::new(1), "fn main() { return 1; }")
        .expect("initial hot reload compile");
    let runtime = Runtime::new(engine, initial.to_unlinked_program());

    assert!(matches!(
        runtime.compile_hot_reload_update(SourceId::new(2), "fn main() { return 2; }"),
        Err(error) if error.kind == EngineErrorKind::RuntimeNotHotReloadEnabled
    ));
}

#[test]
fn engine_applies_configured_hot_reload_policy() {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .hot_reload_policy(HotReloadPolicy::locked_down())
        .build()
        .expect("engine should build");
    assert_eq!(engine.hot_reload_policy(), &HotReloadPolicy::locked_down());
    let initial = engine
        .compile_hot_reload_initial(SourceId::new(1), "fn main() { return 1; }")
        .expect("initial hot reload compile");

    let error = engine
        .compile_hot_reload_update(
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
        .expect_err("locked-down policy should reject new helper functions");

    assert_eq!(
        error.kind,
        HotReloadErrorKind::NewFunctionDenied {
            function: "helper".to_owned(),
        }
    );
}
