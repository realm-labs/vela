use super::*;

#[test]
fn runtime_compiles_hot_reload_changed_file_from_active_version() {
    let root = unique_test_dir("runtime_hot_reload_changed_file");
    let reward_file = write_reward_modules(&root, "return grant();", 2);
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(2))
    );

    write_reward_module(&reward_file, 6);
    let update = runtime
        .compile_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("changed file update should compile");
    let report = runtime
        .apply_hot_update(update)
        .expect("runtime should apply changed file update");

    assert!(report.accepted);
    assert_eq!(report.changed_functions, vec!["game::reward::grant"]);
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(6))
    );
}

#[test]
fn runtime_stages_hot_reload_changed_file_until_check_reload_safe_point() {
    let root = unique_test_dir("runtime_stage_hot_reload_changed_file");
    let reward_file = write_reward_modules(&root, "return grant();", 2);
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(2))
    );

    write_reward_module(&reward_file, 6);
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("changed file update should stage");
    assert!(
        runtime
            .has_pending_hot_update()
            .expect("changed file update should be pending")
    );
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(2))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged changed-file report");

    assert!(report.accepted);
    assert_eq!(report.changed_functions, vec!["game::reward::grant"]);
    assert_eq!(report.changed_modules, vec!["game::reward"]);
    assert_eq!(
        report.impacted_modules,
        vec!["game::main".to_owned(), "game::reward".to_owned()]
    );
    assert!(
        !runtime
            .has_pending_hot_update()
            .expect("safe point should consume changed-file update")
    );
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(6))
    );
}

#[test]
fn runtime_stages_changed_file_private_helper_addition_until_safe_point() {
    private_helper_addition_report(
        "runtime_stage_changed_file_private_helper_addition",
        ScriptFunctionReloadWorkflow::ChangedFile,
    );
}

#[test]
fn runtime_stages_changed_file_public_function_addition_until_safe_point() {
    public_function_addition_report(
        "runtime_stage_changed_file_public_function_addition",
        ScriptFunctionReloadWorkflow::ChangedFile,
    );
}

#[test]
fn runtime_stages_changed_file_hot_reload_rejection_until_safe_point() {
    let root = unique_test_dir("runtime_stage_changed_file_rejection");
    let reward_file = write_reward_modules(&root, "return grant();", 2);
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .hot_reload_policy(HotReloadPolicy::locked_down())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    write_reward_module_with_helper(&reward_file, 6);
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("hot reload rejection should be staged");
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(2))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert!(matches!(
        report.errors[0].error.kind,
        HotReloadErrorKind::NewFunctionDenied { ref function }
            if function == "game::reward::helper"
    ));
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(2))
    );
}

#[test]
fn runtime_stages_changed_file_return_abi_rejection_until_safe_point() {
    let root = unique_test_dir("runtime_stage_changed_file_return_abi");
    let reward_file = write_typed_reward_modules(&root, "return grant();", "int", "2");
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(2))
    );

    write_typed_reward_module(&reward_file, "float", "6.0");
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("changed-file return ABI rejection should be staged");
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(2))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged changed-file return ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.function.return_abi_changed");
    assert_function_return_repair_hint(&report);
    let HotReloadErrorKind::ChangedFunctionReturnAbi {
        function,
        old,
        new,
        source_span,
    } = &report.errors[0].error.kind
    else {
        panic!("expected changed function return ABI");
    };
    assert_eq!(function, "game::reward::grant");
    assert_eq!(old.as_deref(), Some("int"));
    assert_eq!(new.as_deref(), Some("float"));
    assert!(source_span.is_some());
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(2))
    );
}

#[test]
fn runtime_stages_changed_file_required_parameter_rejection_until_safe_point() {
    let root = unique_test_dir("runtime_stage_changed_file_required_parameter");
    let reward_file = write_typed_reward_modules(&root, "return 2;", "int", "2");
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(2))
    );

    write_reward_module_with_signature(&reward_file, "(amount: int) -> int", "amount");
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("changed-file required parameter rejection should be staged");
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(2))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged changed-file required parameter rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(
        report.errors[0].code,
        "reload.function.required_added_parameters"
    );
    assert_required_parameter_repair_hint(&report);
    let HotReloadErrorKind::AddedFunctionParametersWithoutDefaults { function, added } =
        &report.errors[0].error.kind
    else {
        panic!("expected added required parameters");
    };
    assert_eq!(function, "game::reward::grant");
    assert_eq!(added, &vec!["amount".to_owned()]);
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(2))
    );
}

#[test]
fn runtime_stages_changed_file_script_function_access_rejection_until_safe_point() {
    let root = unique_test_dir("runtime_stage_changed_file_script_access");
    let reward_file = write_reward_modules(&root, "return grant();", 2);
    let main_file = root.join("game").join("main.vela");
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(2))
    );

    std::fs::write(
        &main_file,
        r#"
fn main() {
    return 3;
}
"#,
    )
    .expect("write main without reward import");
    std::fs::write(
        &reward_file,
        r#"
fn grant() {
    return 6;
}
"#,
    )
    .expect("write reward without public export");
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("changed-file script function access ABI rejection should be staged");
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(2))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged changed-file script function access ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.function.access_changed");
    assert_changed_function_access_rejection(&report, "game::reward::grant");
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(2))
    );
}

#[test]
fn runtime_stages_changed_file_removed_function_rejection_until_safe_point() {
    let kind = removed_script_function_rejection_kind(
        "runtime_stage_changed_file_removed_function",
        ScriptFunctionReloadWorkflow::ChangedFile,
    );

    let HotReloadErrorKind::RemovedFunction { function } = kind else {
        panic!("expected removed script function");
    };
    assert_eq!(function, "game::reward::helper");
}
