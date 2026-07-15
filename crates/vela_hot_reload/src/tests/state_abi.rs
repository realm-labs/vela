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
fn state_abi_reports_addition_and_removal_as_distinct_changes() {
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
