fn private_helper_addition_report(test_name: &str, workflow: ScriptFunctionReloadWorkflow) {
    let root = unique_test_dir(test_name);
    let reward_file = write_reward_modules(&root, "return grant();", 2);
    let engine = Engine::builder().execution_profile(ExecutionProfile::trusted()).build().expect("engine should build");
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

    write_reward_module_calling_helper(&reward_file, 6);
    match workflow {
        ScriptFunctionReloadWorkflow::Directory => runtime
            .stage_hot_reload_update_dir(&root)
            .expect("runtime should be hot-reload enabled")
            .expect("dir helper addition should be staged"),
        ScriptFunctionReloadWorkflow::ChangedFile => runtime
            .stage_hot_reload_update_changed_file(&root, &reward_file)
            .expect("runtime should be hot-reload enabled")
            .expect("changed-file helper addition should be staged"),
    };
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
        .expect("staged helper addition report");

    assert!(report.accepted);
    assert!(
        report
            .changed_functions
            .contains(&"game::reward::grant".to_owned())
    );
    assert!(
        report
            .changed_functions
            .contains(&"game::reward::helper".to_owned())
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

fn public_function_addition_report(test_name: &str, workflow: ScriptFunctionReloadWorkflow) {
    let root = unique_test_dir(test_name);
    let reward_file = write_reward_modules(&root, "return grant();", 2);
    let engine = Engine::builder().execution_profile(ExecutionProfile::trusted()).build().expect("engine should build");
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

    write_reward_module_calling_public_helper(&reward_file, 6);
    match workflow {
        ScriptFunctionReloadWorkflow::Directory => runtime
            .stage_hot_reload_update_dir(&root)
            .expect("runtime should be hot-reload enabled")
            .expect("dir public function addition should be staged"),
        ScriptFunctionReloadWorkflow::ChangedFile => runtime
            .stage_hot_reload_update_changed_file(&root, &reward_file)
            .expect("runtime should be hot-reload enabled")
            .expect("changed-file public function addition should be staged"),
    };
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
    assert!(
        runtime
            .call(
                "game::reward::helper",
                &[],
                CallOptions::unbounded(),
                &mut adapter,
                &mut tx
            )
            .is_err()
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged public function addition report");

    assert!(report.accepted);
    assert!(
        report
            .changed_functions
            .contains(&"game::reward::grant".to_owned())
    );
    assert!(
        report
            .changed_functions
            .contains(&"game::reward::helper".to_owned())
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
    assert_eq!(
        runtime.call(
            "game::reward::helper",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(6))
    );
}

enum EventReloadWorkflow {
    Directory,
    ChangedFile,
}

fn event_parameter_reorder_rejection(test_name: &str, workflow: EventReloadWorkflow) {
    let root = unique_test_dir(test_name);
    let game_dir = root.join("game");
    std::fs::create_dir_all(&game_dir).expect("create module dir");
    let event_file = game_dir.join("events.vela");
    std::fs::write(
        &event_file,
        r#"
#[event("monster.kill")]
fn on_kill(player_id: int, monster_id: int) {
    return 1;
}
"#,
    )
    .expect("write event module");
    let engine = Engine::builder().execution_profile(ExecutionProfile::trusted()).build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call(
            "game::events::on_kill",
            &[OwnedValue::Int(7), OwnedValue::Int(11)],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(1))
    );

    std::fs::write(
        &event_file,
        r#"
#[event("monster.kill")]
fn on_kill(monster_id: int, player_id: int) {
    return 2;
}
"#,
    )
    .expect("write reordered event module");
    match workflow {
        EventReloadWorkflow::Directory => runtime
            .stage_hot_reload_update_dir(&root)
            .expect("runtime should be hot-reload enabled")
            .expect("dir event ABI rejection should be staged"),
        EventReloadWorkflow::ChangedFile => runtime
            .stage_hot_reload_update_changed_file(&root, &event_file)
            .expect("runtime should be hot-reload enabled")
            .expect("changed-file event ABI rejection should be staged"),
    };
    assert_eq!(
        runtime.call(
            "game::events::on_kill",
            &[OwnedValue::Int(7), OwnedValue::Int(11)],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(1))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged event ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.function.changed_parameters");
    let HotReloadErrorKind::ChangedFunctionParameters { function, old, new } =
        &report.errors[0].error.kind
    else {
        panic!("expected changed function parameters");
    };
    assert_eq!(function, "game::events::on_kill");
    assert_eq!(old, &vec!["player_id".to_owned(), "monster_id".to_owned()]);
    assert_eq!(new, &vec!["monster_id".to_owned(), "player_id".to_owned()]);
    assert_eq!(
        runtime.call(
            "game::events::on_kill",
            &[OwnedValue::Int(7), OwnedValue::Int(11)],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(1))
    );
}

fn event_target_rejection(test_name: &str, workflow: EventReloadWorkflow) {
    let root = unique_test_dir(test_name);
    let game_dir = root.join("game");
    std::fs::create_dir_all(&game_dir).expect("create module dir");
    let event_file = game_dir.join("events.vela");
    std::fs::write(
        &event_file,
        r#"
#[event("monster.kill")]
fn on_kill(player_id: int, monster_id: int) {
    return 1;
}
"#,
    )
    .expect("write event module");
    let engine = Engine::builder().execution_profile(ExecutionProfile::trusted()).build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call(
            "game::events::on_kill",
            &[OwnedValue::Int(7), OwnedValue::Int(11)],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(1))
    );

    std::fs::write(
        &event_file,
        r#"
#[event("quest.complete")]
fn on_kill(player_id: int, monster_id: int) {
    return 2;
}
"#,
    )
    .expect("write retargeted event module");
    match workflow {
        EventReloadWorkflow::Directory => runtime
            .stage_hot_reload_update_dir(&root)
            .expect("runtime should be hot-reload enabled")
            .expect("dir event target rejection should be staged"),
        EventReloadWorkflow::ChangedFile => runtime
            .stage_hot_reload_update_changed_file(&root, &event_file)
            .expect("runtime should be hot-reload enabled")
            .expect("changed-file event target rejection should be staged"),
    };
    assert_eq!(
        runtime.call(
            "game::events::on_kill",
            &[OwnedValue::Int(7), OwnedValue::Int(11)],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(1))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged event target rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.function.event_changed");
    let HotReloadErrorKind::ChangedFunctionEvent {
        function,
        old,
        new,
        source_span,
    } = &report.errors[0].error.kind
    else {
        panic!("expected changed function event");
    };
    assert_eq!(function, "game::events::on_kill");
    assert_eq!(old.as_deref(), Some("monster.kill"));
    assert_eq!(new.as_deref(), Some("quest.complete"));
    assert!(source_span.is_some());
    assert_eq!(
        runtime.call(
            "game::events::on_kill",
            &[OwnedValue::Int(7), OwnedValue::Int(11)],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(1))
    );
}

fn assert_changed_function_access_rejection(report: &HotReloadReport, expected_function: &str) {
    assert_eq!(
        report.errors[0].repair_hint.as_deref(),
        Some("preserve reflective access metadata or require host approval before reloading")
    );
    let HotReloadErrorKind::ChangedFunctionAccess {
        function,
        old,
        new,
        source_span,
    } = &report.errors[0].error.kind
    else {
        panic!("expected changed function access ABI");
    };
    assert_eq!(function, expected_function);
    assert!(old.public);
    assert!(!new.public);
    assert!(source_span.is_some());
}

fn assert_top_level_side_effect_repair_label(report: &HotReloadReport) {
    assert!(
        report.errors[0]
            .source_diagnostics
            .iter()
            .any(|diagnostic| {
                diagnostic.code.as_deref() == Some("hir::top_level_side_effect")
                    && diagnostic.labels.iter().any(|label| {
                        label
                            .message
                            .contains("move this work into a runtime function")
                    })
            })
    );
}

fn assert_function_return_repair_hint(report: &HotReloadReport) {
    assert_eq!(
        report.errors[0].repair_hint.as_deref(),
        Some("preserve the previous return type hint or restart with an explicit migration")
    );
}

fn assert_rendered_repair_hint(report: &HotReloadReport, expected: &str) {
    assert!(report.render_lines().iter().any(|line| {
        line.kind == HotReloadReportLineKind::RepairHint
            && line.text == format!("repair: {expected}")
    }));
}

fn assert_required_parameter_repair_hint(report: &HotReloadReport) {
    assert_eq!(
        report.errors[0].repair_hint.as_deref(),
        Some("give every appended parameter a default value")
    );
}

fn assert_changed_schema_abi_repair_hint(report: &HotReloadReport) {
    assert_eq!(
        report.errors[0].repair_hint.as_deref(),
        Some("preserve existing schema members, or add only defaulted fields during reload")
    );
}

fn assert_removed_schema_repair_hint(report: &HotReloadReport) {
    assert_eq!(
        report.errors[0].repair_hint.as_deref(),
        Some("restore the schema or restart with an explicit migration")
    );
}

fn assert_changed_trait_abi_repair_hint(report: &HotReloadReport) {
    assert_eq!(
        report.errors[0].repair_hint.as_deref(),
        Some(
            "preserve existing trait method IDs, names, parameters, return hints, and default status"
        )
    );
}

fn assert_removed_trait_abi_repair_hint(report: &HotReloadReport) {
    assert_eq!(
        report.errors[0].repair_hint.as_deref(),
        Some("restore the trait ABI entry or restart with an explicit migration")
    );
}

fn assert_changed_module_abi_repair_hint(report: &HotReloadReport) {
    assert_eq!(
        report.errors[0].repair_hint.as_deref(),
        Some("preserve existing module exports or restart with an explicit migration")
    );
}

fn assert_removed_module_abi_repair_hint(report: &HotReloadReport) {
    assert_eq!(
        report.errors[0].repair_hint.as_deref(),
        Some("restore the module ABI entry or restart with an explicit migration")
    );
}

fn assert_parameter_abi_repair_hint(report: &HotReloadReport) {
    assert_eq!(
        report.errors[0].repair_hint.as_deref(),
        Some("preserve existing parameter names, order, type hints, and defaults")
    );
}

fn assert_method_parameter_abi_repair_hint(report: &HotReloadReport) {
    assert_eq!(
        report.errors[0].repair_hint.as_deref(),
        Some("preserve existing method parameter names, order, type hints, and defaults")
    );
}

fn assert_effect_abi_repair_hint(report: &HotReloadReport) {
    assert_eq!(
        report.errors[0].repair_hint.as_deref(),
        Some("preserve the previous effect set or require host approval before reloading")
    );
}

fn assert_access_abi_repair_hint(report: &HotReloadReport) {
    assert_eq!(
        report.errors[0].repair_hint.as_deref(),
        Some("preserve reflective access metadata or require host approval before reloading")
    );
}

fn assert_removed_function_abi_repair_hint(report: &HotReloadReport) {
    assert_eq!(
        report.errors[0].repair_hint.as_deref(),
        Some("restore the function ABI entry or restart with an explicit migration")
    );
}

fn assert_removed_method_abi_repair_hint(report: &HotReloadReport) {
    assert_eq!(
        report.errors[0].repair_hint.as_deref(),
        Some("restore the method ABI entry or restart with an explicit migration")
    );
}

fn assert_method_return_repair_hint(report: &HotReloadReport) {
    assert_eq!(
        report.errors[0].repair_hint.as_deref(),
        Some("preserve the previous method return type hint or restart with an explicit migration")
    );
}
