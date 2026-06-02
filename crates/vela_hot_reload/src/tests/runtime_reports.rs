use super::*;

#[test]
fn new_calls_enter_new_code_after_update() {
    let initial =
        compile_initial(SourceId::new(1), "fn main() { return 20; }").expect("compile initial");
    let mut runtime = HotReloadRuntime::new(initial);
    let update = compile_update(
        &runtime.current(),
        SourceId::new(2),
        "fn main() { return 30; }",
    )
    .expect("compile update");

    runtime.apply_hot_update(update).expect("apply update");

    assert_eq!(
        Vm::new().run_program(&runtime.current().to_program(), "main", &[]),
        Ok(Value::Int(30))
    );
}

#[test]
fn staged_update_waits_for_check_reload_safe_point() {
    let initial =
        compile_initial(SourceId::new(1), "fn main() { return 20; }").expect("compile initial");
    let mut runtime = HotReloadRuntime::new(initial);
    let update = compile_update(
        &runtime.current(),
        SourceId::new(2),
        "fn main() { return 30; }",
    )
    .expect("compile update");

    assert_eq!(runtime.stage_hot_update(update), None);
    assert!(runtime.has_pending_update());
    assert_eq!(
        Vm::new().run_program(&runtime.current().to_program(), "main", &[]),
        Ok(Value::Int(20))
    );

    let report = runtime
        .check_reload()
        .expect("safe point should consume pending update");

    assert!(report.accepted);
    assert_eq!(report.changed_functions, ["main"]);
    assert!(!runtime.has_pending_update());
    assert_eq!(
        Vm::new().run_program(&runtime.current().to_program(), "main", &[]),
        Ok(Value::Int(30))
    );
}

#[test]
fn staged_rejected_update_reports_at_check_reload_safe_point() {
    let initial =
        compile_initial(SourceId::new(1), "fn main() { return 20; }").expect("compile initial");
    let mut runtime = HotReloadRuntime::new(initial);
    let old = runtime.current();
    let policy = HotReloadPolicy::locked_down();
    let update = compile_update_with_policy(
        &old,
        SourceId::new(2),
        r#"
fn helper() {
    return 5;
}

fn main() {
    return helper();
}
"#,
        &policy,
    );

    assert_eq!(runtime.stage_hot_update_result(update), None);
    assert!(runtime.has_pending_update());

    let report = runtime
        .check_reload()
        .expect("safe point should report pending rejection");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(runtime.current().id, old.id);
    assert!(!runtime.has_pending_update());
    assert_eq!(report.errors[0].code, "reload.function.new_denied");
    assert_eq!(
        Vm::new().run_program(&runtime.current().to_program(), "main", &[]),
        Ok(Value::Int(20))
    );
}

#[test]
fn apply_hot_update_report_summarizes_accepted_changes() {
    let initial =
        compile_initial(SourceId::new(1), "fn main() { return 20; }").expect("compile initial");
    let mut runtime = HotReloadRuntime::new(initial);
    let update = compile_update(
        &runtime.current(),
        SourceId::new(2),
        r#"
fn helper() {
    return 5;
}

fn main() {
    return helper();
}
"#,
    )
    .expect("compile update");

    let report = runtime.apply_hot_update_report(update);

    assert!(report.accepted);
    assert_eq!(report.from_version, ProgramVersionId(0));
    assert_eq!(report.to_version, Some(ProgramVersionId(1)));
    assert_eq!(report.changed_functions, ["helper", "main"]);
    assert!(report.errors.is_empty());
    let version = report.version().expect("accepted report version");
    assert_eq!(
        Vm::new().run_program(&version.to_program(), "main", &[]),
        Ok(Value::Int(5))
    );
}

#[test]
fn accepted_report_render_lines_include_changed_functions() {
    let initial =
        compile_initial(SourceId::new(1), "fn main() { return 20; }").expect("compile initial");
    let mut runtime = HotReloadRuntime::new(initial);
    let update = compile_update(
        &runtime.current(),
        SourceId::new(2),
        r#"
fn helper() {
    return 5;
}

fn main() {
    return helper();
}
"#,
    )
    .expect("compile update");

    let report = runtime.apply_hot_update_report(update);
    let lines = report.render_lines();

    assert_eq!(
        lines,
        vec![
            HotReloadReportLine::new(
                HotReloadReportLineKind::Summary,
                None,
                None,
                "hot reload accepted: v0 -> v1",
            ),
            HotReloadReportLine::new(
                HotReloadReportLineKind::ChangedFunctions,
                None,
                None,
                "changed functions: helper, main",
            ),
        ]
    );
}

#[test]
fn old_version_lifetime_preserves_old_code() {
    let initial =
        compile_initial(SourceId::new(1), "fn main() { return 20; }").expect("compile initial");
    let mut runtime = HotReloadRuntime::new(initial);
    let old = runtime.current();
    let update =
        compile_update(&old, SourceId::new(2), "fn main() { return 30; }").expect("update");

    let new = runtime.apply_hot_update(update).expect("apply update");

    assert_eq!(
        Vm::new().run_program(&old.to_program(), "main", &[]),
        Ok(Value::Int(20))
    );
    assert_eq!(
        Vm::new().run_program(&new.to_program(), "main", &[]),
        Ok(Value::Int(30))
    );
}

#[test]
fn rejected_report_carries_reason_and_repair_hint() {
    let error = HotReloadError {
        kind: HotReloadErrorKind::NewFunctionDenied {
            function: "helper".to_owned(),
        },
    };

    let report = HotReloadReport::rejected(ProgramVersionId(2), error.clone());

    assert!(!report.accepted);
    assert_eq!(report.from_version, ProgramVersionId(2));
    assert_eq!(report.to_version, None);
    assert!(report.changed_functions.is_empty());
    assert_eq!(report.version(), None);
    assert_eq!(report.errors.len(), 1);
    assert_eq!(report.errors[0].error, error);
    assert_eq!(report.errors[0].code, "reload.function.new_denied");
    assert_eq!(report.errors[0].target.as_deref(), Some("helper"));
    assert_eq!(report.errors[0].detail, None);
    assert_eq!(
        report.errors[0].reason,
        "new function `helper` is denied by reload policy"
    );
    assert_eq!(
        report.errors[0].repair_hint.as_deref(),
        Some("enable new functions in HotReloadPolicy or remove the new declaration")
    );
}

#[test]
fn rejected_report_targets_schema_and_method_errors() {
    let schema = HotReloadReport::rejected(
        ProgramVersionId(1),
        HotReloadError {
            kind: HotReloadErrorKind::ChangedSchema {
                type_name: "Player".to_owned(),
                old_hash: 1,
                new_hash: 2,
                source_span: None,
            },
        },
    );
    assert_eq!(schema.errors[0].code, "reload.schema.changed");
    assert_eq!(schema.errors[0].target.as_deref(), Some("Player"));
    assert_eq!(
        schema.errors[0].detail,
        Some(HotReloadDiagnosticDetail::SchemaHash {
            old_hash: 1,
            new_hash: Some(2),
        })
    );

    let method = HotReloadReport::rejected(
        ProgramVersionId(1),
        HotReloadError {
            kind: HotReloadErrorKind::ChangedMethodAccess {
                type_name: "Player".to_owned(),
                method: "grant_exp".to_owned(),
                old: AccessAbi::public(),
                new: AccessAbi::new(true, false, Vec::new()),
                source_span: None,
            },
        },
    );
    assert_eq!(method.errors[0].code, "reload.method.access_changed");
    assert_eq!(method.errors[0].target.as_deref(), Some("Player.grant_exp"));
    assert_eq!(
        method.errors[0].detail,
        Some(HotReloadDiagnosticDetail::MethodAccessAbi {
            old: AccessAbi::public(),
            new: AccessAbi::new(true, false, Vec::new()),
        })
    );
}

#[test]
fn rejected_report_carries_function_abi_details() {
    let deleted = HotReloadReport::rejected(
        ProgramVersionId(1),
        HotReloadError {
            kind: HotReloadErrorKind::DeletedFunctionParameters {
                function: "main".to_owned(),
                old: vec!["player".to_owned(), "monster".to_owned()],
                new: vec!["player".to_owned()],
            },
        },
    );
    assert_eq!(
        deleted.errors[0].detail,
        Some(HotReloadDiagnosticDetail::FunctionParameterList {
            old: vec!["player".to_owned(), "monster".to_owned()],
            new: vec!["player".to_owned()],
        })
    );

    let added = HotReloadReport::rejected(
        ProgramVersionId(1),
        HotReloadError {
            kind: HotReloadErrorKind::AddedFunctionParametersWithoutDefaults {
                function: "main".to_owned(),
                added: vec!["amount".to_owned()],
            },
        },
    );
    assert_eq!(
        added.errors[0].detail,
        Some(HotReloadDiagnosticDetail::AddedFunctionParameters {
            added: vec!["amount".to_owned()],
        })
    );

    let effects = HotReloadReport::rejected(
        ProgramVersionId(1),
        HotReloadError {
            kind: HotReloadErrorKind::ChangedFunctionEffects {
                function: "game.reward.grant".to_owned(),
                old: EffectAbi::host_read(),
                new: EffectAbi::host_write(),
                source_span: None,
            },
        },
    );
    assert_eq!(
        effects.errors[0].detail,
        Some(HotReloadDiagnosticDetail::FunctionEffectAbi {
            old: EffectAbi::host_read(),
            new: EffectAbi::host_write(),
        })
    );
}

#[test]
fn rejected_report_render_lines_include_detail_and_hint() {
    let report = HotReloadReport::rejected(
        ProgramVersionId(7),
        HotReloadError {
            kind: HotReloadErrorKind::ChangedMethodAccess {
                type_name: "Player".to_owned(),
                method: "grant_exp".to_owned(),
                old: AccessAbi::public(),
                new: AccessAbi::new(true, false, vec!["admin.reload".to_owned()]),
                source_span: None,
            },
        },
    );

    let lines = report.render_lines();

    assert_eq!(
        lines,
        vec![
            HotReloadReportLine::new(
                HotReloadReportLineKind::Summary,
                None,
                None,
                "hot reload rejected: v7 unchanged",
            ),
            HotReloadReportLine::new(
                HotReloadReportLineKind::Diagnostic,
                Some(0),
                None,
                "[reload.method.access_changed] Player.grant_exp: method `Player.grant_exp` changed reflective access ABI",
            ),
            HotReloadReportLine::new(
                HotReloadReportLineKind::Detail,
                Some(0),
                None,
                "method access: old=(public=true reflective=true callable=true permissions=[]) new=(public=true reflective=false callable=false permissions=[admin.reload])",
            ),
            HotReloadReportLine::new(
                HotReloadReportLineKind::RepairHint,
                Some(0),
                None,
                "repair: preserve reflective access metadata or require host approval before reloading",
            ),
        ]
    );
}

#[test]
fn apply_hot_update_result_report_preserves_current_version_on_rejection() {
    let initial =
        compile_initial(SourceId::new(1), "fn main() { return 1; }").expect("compile initial");
    let mut runtime = HotReloadRuntime::new(initial);
    let old = runtime.current();
    let policy = HotReloadPolicy::locked_down();
    let update = compile_update_with_policy(
        &old,
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
    );

    let report = runtime.apply_hot_update_result_report(update);

    assert!(!report.accepted);
    assert_eq!(report.from_version, old.id);
    assert_eq!(report.to_version, None);
    assert_eq!(runtime.current().id, old.id);
    assert_eq!(report.errors.len(), 1);
    assert_eq!(
        report.errors[0].reason,
        "new function `helper` is denied by reload policy"
    );
    assert_eq!(
        Vm::new().run_program(&runtime.current().to_program(), "main", &[]),
        Ok(Value::Int(1))
    );
}

#[test]
fn rejected_compile_report_carries_source_span_and_labels() {
    let initial = compile_initial(SourceId::new(1), "fn main(value) { return value; }")
        .expect("compile initial");
    let mut runtime = HotReloadRuntime::new(initial);

    let report = runtime.apply_hot_update_result_report(compile_update(
        &runtime.current(),
        SourceId::new(2),
        "fn main(value: Array<int>) { return value; }",
    ));

    assert!(!report.accepted);
    assert_eq!(report.errors.len(), 1);
    let diagnostic = &report.errors[0];
    assert_eq!(diagnostic.code, "reload.compile");
    assert_eq!(diagnostic.target, None);
    assert_eq!(diagnostic.detail, None);
    assert_eq!(
        diagnostic.source_span.expect("compile source span").source,
        SourceId::new(2)
    );
    assert!(diagnostic.source_diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("script type hints do not support generics")
    }));
    assert!(
        diagnostic
            .labels
            .iter()
            .any(|label| label.message == "remove generic type arguments")
    );
    let lines = report.render_lines();
    assert!(lines.iter().any(|line| {
        line.kind == HotReloadReportLineKind::SourceDiagnostic
            && line
                .text
                .contains("script type hints do not support generics")
    }));
    assert!(lines.iter().any(|line| {
        line.kind == HotReloadReportLineKind::SourceLabel
            && line.text.contains("remove generic type arguments")
            && line.span.is_some()
    }));
    assert_eq!(runtime.current().id, ProgramVersionId(0));
}
