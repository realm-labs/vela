use vela_engine::engine::Engine;
use vela_engine::runtime::{CallArgs, CallOptions, Runtime};
use vela_hot_reload::error::HotReloadErrorKind;
use vela_hot_reload::runtime::HotReloadRuntime;
use vela_vm::owned_value::OwnedValue;

#[test]
fn ordinary_reload_rejects_stale_updates_and_replay() {
    let engine = Engine::builder().build().unwrap();
    let base = engine
        .compile_hot_reload_initial("fn main() { return 0; }")
        .unwrap();
    let added = engine
        .compile_hot_reload_update(&base, "fn main() { return 1; } fn helper() { return 2; }")
        .unwrap();
    let stale = engine
        .compile_hot_reload_update(&base, "fn main() { return 3; }")
        .unwrap();
    let mut runtime = HotReloadRuntime::new(base);
    runtime.apply_hot_update(added.clone()).unwrap();
    let current = runtime.current();
    assert!(matches!(
        runtime.apply_hot_update(stale.clone()).unwrap_err().kind,
        HotReloadErrorKind::UpdateBaseMismatch { .. }
    ));
    assert!(!runtime.apply_hot_update_report(added).accepted);
    runtime.staging_handle().stage_reload_update(stale);
    let report = runtime.check_reload().unwrap();
    assert!(!report.accepted);
    assert_eq!(report.errors[0].code, "reload.update.base_mismatch");
    assert!(std::sync::Arc::ptr_eq(&current, &runtime.current()));
    assert!(current.function("helper").is_some());
}

#[test]
fn independent_initial_generations_do_not_match_even_with_identical_source() {
    let engine = Engine::builder().build().unwrap();
    let source = "fn main() { return 0; }";
    let base = engine.compile_hot_reload_initial(source).unwrap();
    let other = engine.compile_hot_reload_initial(source).unwrap();
    assert_eq!(base.id, other.id);
    let update = engine
        .compile_hot_reload_update(&base, "fn main() { return 1; }")
        .unwrap();
    let mut runtime = HotReloadRuntime::new(other);
    assert!(matches!(
        runtime.apply_hot_update(update).unwrap_err().kind,
        HotReloadErrorKind::UpdateBaseMismatch { .. }
    ));
}

#[test]
fn exact_generation_fanout_supports_successive_updates_with_independent_staging() {
    let engine = Engine::builder().build().unwrap();
    let base = engine
        .compile_hot_reload_initial("fn main() { return 0; }")
        .unwrap();
    let update = engine
        .compile_hot_reload_update(&base, "fn main() { return 1; }")
        .unwrap();
    let mut first = HotReloadRuntime::new(base.clone());
    let mut second = HotReloadRuntime::new(base);
    let producer = first.staging_handle();
    producer.clone().stage_reload_update(update.clone());
    assert!(!second.has_pending_update());
    assert!(first.check_reload().unwrap().accepted);
    second.apply_hot_update(update).unwrap();
    let next = engine
        .compile_hot_reload_update(&first.current(), "fn main() { return 2; }")
        .unwrap();
    first.apply_hot_update(next.clone()).unwrap();
    second.apply_hot_update(next).unwrap();
    assert_eq!(first.current(), second.current());
}

#[test]
fn stale_runtime_update_is_rejected_before_initializing_new_state() {
    let engine = Engine::builder().build().unwrap();
    let base = engine
        .compile_hot_reload_initial("fn main() { return 0; }")
        .unwrap();
    let added = engine
        .compile_hot_reload_update(&base, "fn main() { return 1; }")
        .unwrap();
    // This initializer would exhaust its budget if activation ran it.
    let stale = engine.compile_hot_reload_update(&base,
        "fn recurse() -> i64 { return recurse(); } state added: i64 = recurse(); fn main() { return added; }").unwrap();
    let mut runtime = Runtime::from_hot_reload_version(engine, base).unwrap();
    runtime.stage_reload_update(added).unwrap();
    assert!(runtime.activate_reload().unwrap().unwrap().accepted);
    runtime.stage_reload_update(stale).unwrap();
    let report = runtime.activate_reload().unwrap().unwrap();
    assert!(!report.accepted);
    assert_eq!(report.errors[0].code, "reload.update.base_mismatch");
    let result = runtime
        .call(
            "main",
            CallArgs::new(),
            CallOptions::new(10_000, 1024 * 1024, 64),
        )
        .unwrap();
    assert_eq!(runtime.value_to_owned(&result).unwrap(), OwnedValue::i64(1));
}
