use super::*;

#[test]
fn deleted_function_parameters_are_rejected() {
    let initial = compile_initial(SourceId::new(1), "fn main(value) { return value; }")
        .expect("compile initial");

    let error = compile_update(&initial, SourceId::new(2), "fn main() { return 0; }")
        .expect_err("deleted param");

    assert_eq!(
        error.kind,
        HotReloadErrorKind::DeletedFunctionParameters {
            function: "main".to_owned(),
            old: vec!["value".to_owned()],
            new: Vec::new(),
        }
    );
}

#[test]
fn script_schema_abi_changes_are_rejected_during_compile_update() {
    let initial = compile_initial(
        SourceId::new(1),
        r#"
struct Reward {
    item_id: string
    count: int
}

fn main() {
    return 1;
}
"#,
    )
    .expect("initial script schema should compile");

    let error = compile_update(
        &initial,
        SourceId::new(2),
        r#"
struct Reward {
    item_id: string
    count: float
}

fn main() {
    return 1;
}
"#,
    )
    .expect_err("changed script schema should be rejected");

    assert_eq!(error.code(), "reload.schema.abi_changed");
    assert_eq!(error.target(), Some("Reward".to_owned()));
}

#[test]
fn script_schema_defaulted_field_additions_are_accepted_during_compile_update() {
    let initial = compile_initial(
        SourceId::new(1),
        r#"
struct Reward {
    item_id: string
}

fn main() {
    return 1;
}
"#,
    )
    .expect("initial script schema should compile");

    let update = compile_update(
        &initial,
        SourceId::new(2),
        r#"
struct Reward {
    item_id: string
    count: int = 1
}

fn main() {
    return 1;
}
"#,
    )
    .expect("defaulted script schema addition should be accepted");

    let mut runtime = HotReloadRuntime::new(initial);
    runtime.apply_hot_update(update).expect("apply update");
    assert_eq!(
        Vm::new().run_program(&runtime.current().to_program(), "main", &[]),
        Ok(Value::Int(1))
    );
}

#[test]
fn script_schema_stable_id_member_renames_are_accepted_during_compile_update() {
    let initial = compile_initial(
        SourceId::new(1),
        r#"
struct Reward {
    #[id(101)]
    item_id: string
    #[id(102)]
    count: int
}

enum QuestProgress {
    #[id(201)]
    Active
}

fn main() {
    return 1;
}
"#,
    )
    .expect("initial script schema should compile");

    let update = compile_update(
        &initial,
        SourceId::new(2),
        r#"
struct Reward {
    #[id(101)]
    item: string
    #[id(102)]
    quantity: int
}

enum QuestProgress {
    #[id(201)]
    Started
    #[id(202)]
    Finished
}

fn main() {
    return 2;
}
"#,
    )
    .expect("stable-id script schema renames should be accepted");

    let mut runtime = HotReloadRuntime::new(initial);
    runtime.apply_hot_update(update).expect("apply update");
    assert_eq!(
        Vm::new().run_program(&runtime.current().to_program(), "main", &[]),
        Ok(Value::Int(2))
    );
}

#[test]
fn script_schema_invalid_stable_ids_are_rejected_during_compile_update() {
    let initial = compile_initial(
        SourceId::new(1),
        r#"
struct Reward {
    #[id(101)]
    item_id: string
}

fn main() {
    return 1;
}
"#,
    )
    .expect("initial script schema should compile");

    let error = compile_update(
        &initial,
        SourceId::new(2),
        r#"
struct Reward {
    #[id(101)]
    item_id: string
    #[id(101)]
    count: int
}

fn main() {
    return 1;
}
"#,
    )
    .expect_err("duplicate stable ids should be rejected before reload ABI checks");

    assert_eq!(error.code(), "reload.compile");
    assert!(
        error
            .source_diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code.as_deref() == Some("hir::duplicate_field_id") })
    );
}

#[test]
fn script_function_return_abi_changes_are_rejected_during_compile_update() {
    let initial = compile_initial(
        SourceId::new(1),
        r#"
fn main() -> int {
    return 1;
}
"#,
    )
    .expect("initial script function should compile");

    let error = compile_update(
        &initial,
        SourceId::new(2),
        r#"
fn main() -> float {
    return 1;
}
"#,
    )
    .expect_err("changed script function return ABI should be rejected");

    assert_eq!(error.code(), "reload.function.return_abi_changed");
    assert_eq!(error.target(), Some("main".to_owned()));
}

#[test]
fn reordered_function_parameters_are_rejected() {
    let initial = compile_initial(SourceId::new(1), "fn main(player, monster) { return 1; }")
        .expect("compile initial");

    let error = compile_update(
        &initial,
        SourceId::new(2),
        "fn main(monster, player) { return 2; }",
    )
    .expect_err("reordered params");

    assert_eq!(
        error.kind,
        HotReloadErrorKind::ChangedFunctionParameters {
            function: "main".to_owned(),
            old: vec!["player".to_owned(), "monster".to_owned()],
            new: vec!["monster".to_owned(), "player".to_owned()],
        }
    );
}

#[test]
fn appended_function_parameters_are_accepted() {
    let initial = compile_initial(SourceId::new(1), "fn main(player) { return 1; }")
        .expect("compile initial");

    compile_update(
        &initial,
        SourceId::new(2),
        "fn main(player, amount = 1) { return amount; }",
    )
    .expect("defaulted append keeps existing parameter ABI");
}

#[test]
fn appended_function_parameters_without_defaults_are_rejected() {
    let initial = compile_initial(SourceId::new(1), "fn main(player) { return 1; }")
        .expect("compile initial");

    let error = compile_update(
        &initial,
        SourceId::new(2),
        "fn main(player, amount) { return amount; }",
    )
    .expect_err("required appended param should break existing callers");

    assert_eq!(
        error.kind,
        HotReloadErrorKind::AddedFunctionParametersWithoutDefaults {
            function: "main".to_owned(),
            added: vec!["amount".to_owned()],
        }
    );
}

#[test]
fn policy_can_reject_defaulted_parameter_additions() {
    let initial = compile_initial(SourceId::new(1), "fn main(player) { return 1; }")
        .expect("compile initial");
    let policy = HotReloadPolicy::new().with_defaulted_parameter_additions(false);

    let error = compile_update_with_policy(
        &initial,
        SourceId::new(2),
        "fn main(player, amount = 1) { return amount; }",
        &policy,
    )
    .expect_err("policy should reject appended params");

    assert_eq!(
        error.kind,
        HotReloadErrorKind::AddedFunctionParametersDenied {
            function: "main".to_owned(),
            added: vec!["amount".to_owned()],
        }
    );
}

#[test]
fn policy_can_reject_new_functions() {
    let initial =
        compile_initial(SourceId::new(1), "fn main() { return 1; }").expect("compile initial");
    let policy = HotReloadPolicy::new().with_new_functions(false);

    let error = compile_update_with_policy(
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
        &policy,
    )
    .expect_err("policy should reject new helper");

    assert_eq!(
        error.kind,
        HotReloadErrorKind::NewFunctionDenied {
            function: "helper".to_owned(),
        }
    );
}

#[test]
fn removed_script_functions_are_rejected() {
    let initial = compile_initial(
        SourceId::new(1),
        r#"
fn helper() {
    return 2;
}

fn main() {
    return helper();
}
"#,
    )
    .expect("compile initial");
    let mut runtime = HotReloadRuntime::new(initial);

    let report = runtime.apply_hot_update_result_report(compile_update(
        &runtime.current(),
        SourceId::new(2),
        r#"
fn main() {
    return 3;
}
"#,
    ));

    assert!(!report.accepted);
    assert_eq!(report.from_version, ProgramVersionId(0));
    assert_eq!(runtime.current().id, ProgramVersionId(0));
    assert_eq!(report.errors.len(), 1);
    assert_eq!(report.errors[0].code, "reload.function.removed");
    assert_eq!(report.errors[0].target.as_deref(), Some("helper"));
    assert_eq!(
        report.errors[0].reason,
        "function `helper` was removed from the update source"
    );
    assert_eq!(
        report.errors[0].repair_hint.as_deref(),
        Some("keep the function declaration or restart with an explicit migration")
    );
    assert_eq!(
        report.errors[0].error.kind,
        HotReloadErrorKind::RemovedFunction {
            function: "helper".to_owned(),
        }
    );
}

#[test]
fn new_private_helper_functions_are_accepted() {
    let initial = compile_initial(SourceId::new(1), "fn main() { return 1; }").expect("initial");
    let mut runtime = HotReloadRuntime::new(initial);
    let update = compile_update(
        &runtime.current(),
        SourceId::new(2),
        r#"
fn helper() {
    return 7;
}

fn main() {
    return helper();
}
"#,
    )
    .expect("helper update");

    runtime.apply_hot_update(update).expect("apply update");

    assert_eq!(
        Vm::new().run_program(&runtime.current().to_program(), "main", &[]),
        Ok(Value::Int(7))
    );
}
