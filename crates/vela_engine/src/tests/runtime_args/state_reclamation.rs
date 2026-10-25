use super::*;

#[test]
fn removed_closure_state_does_not_pin_its_own_generation() {
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_with_id(
            SourceId::new(24),
            r#"
state retired: Closure = || 5;
fn current() { return 1; }
"#,
        )
        .expect("initial generation");
    let vm_state = initial
        .linked_program()
        .states()
        .iter()
        .find(|state| state.qualified_name == "main::retired")
        .expect("VM state descriptor")
        .id;
    let update = engine
        .compile_reload_with_id(&initial, SourceId::new(25), "fn current() { return 2; }")
        .expect("private state removal is compatible");
    let mut runtime = Runtime::builder_from_hot_reload_version(engine, initial)
        .build()
        .expect("runtime initializes");

    let report = runtime
        .apply_reload_update_for_test(update)
        .expect("reload applies");
    assert!(report.accepted);
    assert_eq!(runtime.activate_reload(), Ok(None));

    assert_eq!(runtime.retained_generation_count(), 1);
    assert!(!runtime.retains_vm_state_id(vm_state));
}

#[test]
fn reload_safe_point_reclaims_removed_state_after_final_old_owner_drops() {
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_with_id(
            SourceId::new(26),
            r#"
state retired: i64 = 5;
fn make() { return || retired; }
fn invoke(callback) { return callback(); }
"#,
        )
        .expect("initial generation");
    let vm_state = initial
        .linked_program()
        .states()
        .iter()
        .find(|state| state.qualified_name == "main::retired")
        .expect("VM state descriptor")
        .id;
    let update = engine
        .compile_reload_with_id(
            &initial,
            SourceId::new(27),
            "fn make() { return || 0; } fn invoke(callback) { return callback(); }",
        )
        .expect("private state removal is compatible");
    let mut runtime = Runtime::builder_from_hot_reload_version(engine, initial)
        .build()
        .expect("runtime initializes");
    let old_closure = runtime
        .call("make", CallArgs::new(), CallOptions::unbounded())
        .expect("old closure");

    let report = runtime
        .apply_reload_update_for_test(update)
        .expect("reload applies");

    assert!(report.accepted);
    assert_eq!(runtime.retained_generation_count(), 2);
    assert!(runtime.retains_vm_state_id(vm_state));
    assert_eq!(runtime.activate_reload(), Ok(None));
    assert_eq!(runtime.retained_generation_count(), 2);
    assert!(runtime.retains_vm_state_id(vm_state));

    drop(old_closure);
    assert_eq!(runtime.activate_reload(), Ok(None));

    assert_eq!(runtime.retained_generation_count(), 1);
    assert!(!runtime.retains_vm_state_id(vm_state));
}
