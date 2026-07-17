use super::*;

#[test]
fn state_abi_preserves_exact_contracts_across_reorder_and_reports_initializer_changes() {
    let initial = compile_initial(
        SourceId::new(201),
        "state first: i64 = 1; state second: i64 = 2; fn main() { return first; }",
    )
    .expect("initial state generation");
    let update = compile_update(
        &initial,
        SourceId::new(202),
        "state second: i64 = 2; state first: i64 = 9; fn main() { return first; }",
    )
    .expect("state reorder and initializer-only change are compatible");
    let mut runtime = HotReloadRuntime::new(initial);
    let report = runtime.apply_hot_update_report(update);

    assert!(report.accepted);
    assert_eq!(report.initializer_changed_states, ["main::first"]);
    assert!(report.added_states.is_empty());
    assert!(report.removed_states.is_empty());
}

#[test]
fn state_abi_rejects_storage_and_exact_type_contract_changes() {
    let initial = compile_initial(
        SourceId::new(203),
        "state value: i64 = 1; fn main() { return value; }",
    )
    .expect("initial state generation");

    let storage_error = compile_update(
        &initial,
        SourceId::new(204),
        "extern state value: Player; fn main() { return value; }",
    )
    .expect_err("storage change must reject");
    assert!(matches!(
        storage_error.kind,
        HotReloadErrorKind::ChangedStateStorage { ref state, .. } if state == "main::value"
    ));
    assert_eq!(storage_error.code(), "reload.state.storage_changed");

    let type_error = compile_update(
        &initial,
        SourceId::new(205),
        "state value: String = \"changed\"; fn main() { return value; }",
    )
    .expect_err("type change must reject");
    assert!(matches!(
        type_error.kind,
        HotReloadErrorKind::ChangedStateType { ref state, .. } if state == "main::value"
    ));
    assert_eq!(type_error.code(), "reload.state.type_changed");
}

#[test]
fn state_abi_reports_private_addition_and_removal_as_distinct_changes() {
    let initial = compile_initial(
        SourceId::new(206),
        "state old_name: i64 = 1; fn main() { return 0; }",
    )
    .expect("initial state generation");
    let update = compile_update(
        &initial,
        SourceId::new(207),
        "state new_name: i64 = 2; fn main() { return 0; }",
    )
    .expect("state rename is remove plus add");
    let mut runtime = HotReloadRuntime::new(initial);
    let report = runtime.apply_hot_update_report(update);

    assert_eq!(report.added_states, ["main::new_name"]);
    assert_eq!(report.removed_states, ["main::old_name"]);
}

#[test]
fn state_abi_rejects_public_export_removal_and_visibility_downgrade() {
    let initial = compile_initial(
        SourceId::new(208),
        "pub state value: i64 = 1; fn main() { return value; }",
    )
    .expect("initial public state generation");

    let removal_error = compile_update(&initial, SourceId::new(209), "fn main() { return 0; }")
        .expect_err("public state export removal must reject");
    assert!(matches!(
        removal_error.kind,
        HotReloadErrorKind::RemovedStateExport { ref state, .. } if state == "main::value"
    ));
    assert_eq!(removal_error.code(), "reload.state.export_removed");

    let visibility_error = compile_update(
        &initial,
        SourceId::new(210),
        "state value: i64 = 1; fn main() { return value; }",
    )
    .expect_err("public state visibility downgrade must reject");
    assert!(matches!(
        visibility_error.kind,
        HotReloadErrorKind::DowngradedStateVisibility { ref state, .. }
            if state == "main::value"
    ));
    assert_eq!(
        visibility_error.code(),
        "reload.state.visibility_downgraded"
    );
}

#[test]
fn state_abi_accepts_private_to_public_export_addition() {
    let initial = compile_initial(
        SourceId::new(211),
        "state value: i64 = 1; fn main() { return value; }",
    )
    .expect("initial private state generation");
    let update = compile_update(
        &initial,
        SourceId::new(212),
        "pub state value: i64 = 9; fn main() { return value; }",
    )
    .expect("adding a public export is compatible");
    let mut runtime = HotReloadRuntime::new(initial);
    let report = runtime.apply_hot_update_report(update);

    assert!(report.accepted);
    assert_eq!(report.visibility_changed_states, ["main::value"]);
    assert_eq!(report.initializer_changed_states, ["main::value"]);
}

#[test]
fn state_abi_reports_transitive_initializer_helper_changes() {
    let initial = compile_initial(
        SourceId::new(213),
        r#"
fn inner() -> i64 { return 1; }
fn outer() -> i64 { return inner(); }
fn unrelated() -> i64 { return 10; }
state value: i64 = outer();
fn main() { return value; }
"#,
    )
    .expect("initial helper graph");
    let update = compile_update(
        &initial,
        SourceId::new(214),
        r#"
fn inner() -> i64 { return 9; }
fn outer() -> i64 { return inner(); }
fn unrelated() -> i64 { return 10; }
state value: i64 = outer();
fn main() { return value; }
"#,
    )
    .expect("transitive helper change is compatible");
    let mut runtime = HotReloadRuntime::new(initial);
    let report = runtime.apply_hot_update_report(update);

    assert!(report.accepted);
    assert_eq!(report.initializer_changed_states, ["main::value"]);
}

#[test]
fn state_abi_ignores_unrelated_helper_changes_for_initializer_reporting() {
    let initial = compile_initial(
        SourceId::new(215),
        r#"
fn inner() -> i64 { return 1; }
fn outer() -> i64 { return inner(); }
fn unrelated() -> i64 { return 10; }
state value: i64 = outer();
fn main() { return value; }
"#,
    )
    .expect("initial helper graph");
    let update = compile_update(
        &initial,
        SourceId::new(216),
        r#"
fn inner() -> i64 { return 1; }
fn outer() -> i64 { return inner(); }
fn unrelated() -> i64 { return 20; }
state value: i64 = outer();
fn main() { return value; }
"#,
    )
    .expect("unrelated helper change is compatible");
    let mut runtime = HotReloadRuntime::new(initial);
    let report = runtime.apply_hot_update_report(update);

    assert!(report.accepted);
    assert!(report.initializer_changed_states.is_empty());
}

#[test]
fn state_abi_initializer_call_graph_comparison_terminates_on_recursion() {
    let initial = compile_initial(
        SourceId::new(217),
        r#"
fn recurse() -> i64 { return recurse(); }
fn unrelated() -> i64 { return 10; }
state value: i64 = recurse();
fn main() { return 0; }
"#,
    )
    .expect("recursive pure initializer graph");
    let update = compile_update(
        &initial,
        SourceId::new(218),
        r#"
fn recurse() -> i64 { return recurse(); }
fn unrelated() -> i64 { return 20; }
state value: i64 = recurse();
fn main() { return 0; }
"#,
    )
    .expect("unrelated update across recursive graph");
    let mut runtime = HotReloadRuntime::new(initial);
    let report = runtime.apply_hot_update_report(update);

    assert!(report.accepted);
    assert!(report.initializer_changed_states.is_empty());
}

#[test]
fn state_abi_reports_helper_changes_called_from_nested_initializer_closures() {
    let initial = compile_initial(
        SourceId::new(219),
        r#"
fn helper() -> i64 { return 1; }
fn unrelated() -> i64 { return 10; }
fn unrelated_factory() { return || unrelated(); }
state callback: Closure = || helper();
fn main() { return 0; }
"#,
    )
    .expect("nested initializer closure graph");
    let update = compile_update(
        &initial,
        SourceId::new(220),
        r#"
fn helper() -> i64 { return 2; }
fn unrelated() -> i64 { return 20; }
fn unrelated_factory() { return || unrelated(); }
state callback: Closure = || helper();
fn main() { return 0; }
"#,
    )
    .expect("nested initializer closure helper update");
    let mut runtime = HotReloadRuntime::new(initial);
    let report = runtime.apply_hot_update_report(update);

    assert!(report.accepted);
    assert_eq!(report.initializer_changed_states, ["main::callback"]);
}

#[test]
fn state_abi_ignores_unrelated_nested_helper_changes() {
    let initial = compile_initial(
        SourceId::new(221),
        r#"
fn helper() -> i64 { return 1; }
fn unrelated() -> i64 { return 10; }
fn unrelated_factory() { return || unrelated(); }
state callback: Closure = || helper();
fn main() { return 0; }
"#,
    )
    .expect("nested initializer closure graph");
    let update = compile_update(
        &initial,
        SourceId::new(222),
        r#"
fn helper() -> i64 { return 1; }
fn unrelated() -> i64 { return 20; }
fn unrelated_factory() { return || unrelated(); }
state callback: Closure = || helper();
fn main() { return 0; }
"#,
    )
    .expect("unrelated nested helper update");
    let mut runtime = HotReloadRuntime::new(initial);
    let report = runtime.apply_hot_update_report(update);

    assert!(report.accepted);
    assert!(report.initializer_changed_states.is_empty());
}

#[test]
fn state_abi_nested_recursive_initializer_call_graph_terminates() {
    let initial = compile_initial(
        SourceId::new(223),
        r#"
fn recurse() -> i64 { return recurse(); }
fn unrelated() -> i64 { return 10; }
state callback: Closure = || recurse();
fn main() { return 0; }
"#,
    )
    .expect("nested recursive initializer graph");
    let update = compile_update(
        &initial,
        SourceId::new(224),
        r#"
fn recurse() -> i64 { return recurse(); }
fn unrelated() -> i64 { return 20; }
state callback: Closure = || recurse();
fn main() { return 0; }
"#,
    )
    .expect("unrelated update across nested recursive graph");
    let mut runtime = HotReloadRuntime::new(initial);
    let report = runtime.apply_hot_update_report(update);

    assert!(report.accepted);
    assert!(report.initializer_changed_states.is_empty());
}

#[test]
fn state_abi_reports_calls_from_nested_parameter_default_bodies() {
    let initial = compile_initial(
        SourceId::new(225),
        r#"
fn helper() -> i64 { return 1; }
fn build(callback = || helper()) -> Closure { return callback; }
state callback: Closure = build();
fn main() { return 0; }
"#,
    )
    .expect("nested parameter-default initializer graph");
    let update = compile_update(
        &initial,
        SourceId::new(226),
        r#"
fn helper() -> i64 { return 2; }
fn build(callback = || helper()) -> Closure { return callback; }
state callback: Closure = build();
fn main() { return 0; }
"#,
    )
    .expect("nested parameter-default helper update");
    let mut runtime = HotReloadRuntime::new(initial);
    let report = runtime.apply_hot_update_report(update);

    assert!(report.accepted);
    assert_eq!(report.initializer_changed_states, ["main::callback"]);
}
