use vela_engine::engine::Engine;
use vela_engine::runtime::{CallArgs, CallOptions, Runtime};
use vela_hot_reload::error::HotReloadErrorKind;
use vela_hot_reload::runtime::HotReloadRuntime;
use vela_vm::owned_value::OwnedValue;

#[test]
fn ordinary_reload_rejects_stale_updates_and_replay() {
    let engine = Engine::builder()
        .build()
        .expect("engine fixture should build");
    let base = engine
        .compile_hot_reload_initial("fn main() { return 0; }")
        .expect("valid reload fixture operation should succeed");
    let added = engine
        .compile_hot_reload_update(&base, "fn main() { return 1; } fn helper() { return 2; }")
        .expect("valid reload fixture operation should succeed");
    let stale = engine
        .compile_hot_reload_update(&base, "fn main() { return 3; }")
        .expect("valid reload fixture operation should succeed");
    let mut runtime = HotReloadRuntime::new(base);
    runtime
        .apply_hot_update(added.clone())
        .expect("valid reload fixture operation should succeed");
    let current = runtime.current();
    assert!(matches!(
        runtime
            .apply_hot_update(stale.clone())
            .expect_err("mismatched update must reject")
            .kind,
        HotReloadErrorKind::UpdateBaseMismatch { .. }
    ));
    assert!(!runtime.apply_hot_update_report(added).accepted);
    runtime.staging_handle().stage_reload_update(stale);
    let report = runtime
        .check_reload()
        .expect("valid reload fixture operation should succeed");
    assert!(!report.accepted);
    assert_eq!(report.errors[0].code, "reload.update.base_mismatch");
    assert!(std::sync::Arc::ptr_eq(&current, &runtime.current()));
    assert!(current.function("helper").is_some());
}

#[test]
fn independent_initial_generations_do_not_match_even_with_identical_source() {
    let engine = Engine::builder()
        .build()
        .expect("engine fixture should build");
    let source = "fn main() { return 0; }";
    let base = engine
        .compile_hot_reload_initial(source)
        .expect("valid reload fixture operation should succeed");
    let other = engine
        .compile_hot_reload_initial(source)
        .expect("valid reload fixture operation should succeed");
    assert_eq!(base.id, other.id);
    let update = engine
        .compile_hot_reload_update(&base, "fn main() { return 1; }")
        .expect("valid reload fixture operation should succeed");
    let mut runtime = HotReloadRuntime::new(other);
    assert!(matches!(
        runtime
            .apply_hot_update(update)
            .expect_err("mismatched update must reject")
            .kind,
        HotReloadErrorKind::UpdateBaseMismatch { .. }
    ));
}

#[test]
fn exact_generation_fanout_supports_successive_updates_with_independent_staging() {
    let engine = Engine::builder()
        .build()
        .expect("engine fixture should build");
    let base = engine
        .compile_hot_reload_initial("fn main() { return 0; }")
        .expect("valid reload fixture operation should succeed");
    let update = engine
        .compile_hot_reload_update(&base, "fn main() { return 1; }")
        .expect("valid reload fixture operation should succeed");
    let mut first = HotReloadRuntime::new(base.clone());
    let mut second = HotReloadRuntime::new(base);
    let producer = first.staging_handle();
    producer.clone().stage_reload_update(update.clone());
    assert!(!second.has_pending_update());
    assert!(
        first
            .check_reload()
            .expect("valid reload fixture operation should succeed")
            .accepted
    );
    second
        .apply_hot_update(update)
        .expect("valid reload fixture operation should succeed");
    let next = engine
        .compile_hot_reload_update(&first.current(), "fn main() { return 2; }")
        .expect("valid reload fixture operation should succeed");
    first
        .apply_hot_update(next.clone())
        .expect("valid reload fixture operation should succeed");
    second
        .apply_hot_update(next)
        .expect("valid reload fixture operation should succeed");
    assert_eq!(first.current(), second.current());
}

#[test]
fn stale_runtime_update_is_rejected_before_initializing_new_state() {
    let engine = Engine::builder()
        .build()
        .expect("engine fixture should build");
    let base = engine
        .compile_hot_reload_initial("fn main() { return 0; }")
        .expect("valid reload fixture operation should succeed");
    let added = engine
        .compile_hot_reload_update(&base, "fn main() { return 1; }")
        .expect("valid reload fixture operation should succeed");
    // This initializer would exhaust its budget if activation ran it.
    let stale = engine.compile_hot_reload_update(&base,
        "fn recurse() -> i64 { return recurse(); } state added: i64 = recurse(); fn main() { return added; }").expect("valid reload fixture operation should succeed");
    let mut runtime = Runtime::from_hot_reload_version(engine, base)
        .expect("valid reload fixture operation should succeed");
    runtime
        .stage_reload_update(added)
        .expect("valid reload fixture operation should succeed");
    assert!(
        runtime
            .activate_reload()
            .expect("valid reload fixture operation should succeed")
            .expect("valid reload fixture operation should succeed")
            .accepted
    );
    runtime
        .stage_reload_update(stale)
        .expect("valid reload fixture operation should succeed");
    let report = runtime
        .activate_reload()
        .expect("valid reload fixture operation should succeed")
        .expect("valid reload fixture operation should succeed");
    assert!(!report.accepted);
    assert_eq!(report.errors[0].code, "reload.update.base_mismatch");
    let result = runtime
        .call(
            "main",
            CallArgs::new(),
            CallOptions::new(10_000, 1024 * 1024, 64),
        )
        .expect("valid reload fixture operation should succeed");
    assert_eq!(
        runtime
            .value_to_owned(&result)
            .expect("valid reload fixture operation should succeed"),
        OwnedValue::i64(1)
    );
}
