use super::*;
use vela_common::{FunctionId, HostMethodId, MethodId, SourceId, Span, TypeId};
use vela_reflect::{
    FunctionAccess, FunctionDesc, FunctionEffectSet, FunctionParamDesc, MethodAccess, MethodDesc,
    MethodEffectSet, MethodParamDesc, SchemaHash, TraitDesc, TraitMethodDesc, TypeDesc, TypeKey,
    TypeRegistry,
};
use vela_vm::{Value, Vm};

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
                "method access: old=(public=true reflective=true permissions=[]) new=(public=true reflective=false permissions=[admin.reload])",
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

#[test]
fn schema_abi_changes_are_rejected() {
    let old_abi = HotReloadAbi::empty().schema(SchemaAbi::new("Reward", SchemaHash::new(0x1111)));
    let new_abi = HotReloadAbi::empty().schema(SchemaAbi::new("Reward", SchemaHash::new(0x2222)));
    let initial = compile_initial_with_abi(SourceId::new(1), "fn main() { return 1; }", old_abi)
        .expect("initial");

    let error = compile_update_with_abi(
        &initial,
        SourceId::new(2),
        "fn main() { return 2; }",
        new_abi,
    )
    .expect_err("schema change should fail");

    assert_eq!(
        error.kind,
        HotReloadErrorKind::ChangedSchema {
            type_name: "Reward".to_owned(),
            old_hash: 0x1111,
            new_hash: 0x2222,
            source_span: None,
        }
    );
}

#[test]
fn removed_schema_abi_is_rejected() {
    let old_abi = HotReloadAbi::empty().schema(SchemaAbi::new("Reward", SchemaHash::new(0x1111)));
    let initial = compile_initial_with_abi(SourceId::new(1), "fn main() { return 1; }", old_abi)
        .expect("initial");

    let error = compile_update_with_abi(
        &initial,
        SourceId::new(2),
        "fn main() { return 2; }",
        HotReloadAbi::empty(),
    )
    .expect_err("removed schema should fail");

    assert_eq!(
        error.kind,
        HotReloadErrorKind::RemovedSchema {
            type_name: "Reward".to_owned(),
            old_hash: 0x1111,
            source_span: None,
        }
    );
}

#[test]
fn function_effect_and_access_abi_changes_are_rejected() {
    let old_abi = HotReloadAbi::empty().function(FunctionAbi::new(
        "game.reward.grant",
        EffectAbi::host_read(),
        AccessAbi::new(true, true, vec!["reward.read".to_owned()]),
    ));
    let changed_effects = HotReloadAbi::empty().function(FunctionAbi::new(
        "game.reward.grant",
        EffectAbi::host_write(),
        AccessAbi::new(true, true, vec!["reward.read".to_owned()]),
    ));
    let initial =
        compile_initial_with_abi(SourceId::new(1), "fn main() { return 1; }", old_abi.clone())
            .expect("initial");

    let error = compile_update_with_abi(
        &initial,
        SourceId::new(2),
        "fn main() { return 2; }",
        changed_effects,
    )
    .expect_err("effect change should fail");
    assert_eq!(
        error.kind,
        HotReloadErrorKind::ChangedFunctionEffects {
            function: "game.reward.grant".to_owned(),
            old: EffectAbi::host_read(),
            new: EffectAbi::host_write(),
            source_span: None,
        }
    );

    let changed_access = HotReloadAbi::empty().function(FunctionAbi::new(
        "game.reward.grant",
        EffectAbi::host_read(),
        AccessAbi::new(true, true, vec!["reward.write".to_owned()]),
    ));
    let error = compile_update_with_abi(
        &initial,
        SourceId::new(3),
        "fn main() { return 3; }",
        changed_access,
    )
    .expect_err("access change should fail");
    assert_eq!(
        error.kind,
        HotReloadErrorKind::ChangedFunctionAccess {
            function: "game.reward.grant".to_owned(),
            old: AccessAbi::new(true, true, vec!["reward.read".to_owned()]),
            new: AccessAbi::new(true, true, vec!["reward.write".to_owned()]),
            source_span: None,
        }
    );
}

#[test]
fn function_event_abi_changes_are_rejected() {
    let old_abi = HotReloadAbi::empty().function(
        FunctionAbi::new(
            "game.reward.grant",
            EffectAbi::event_emit(),
            AccessAbi::public(),
        )
        .event("monster.kill"),
    );
    let changed_event = HotReloadAbi::empty().function(
        FunctionAbi::new(
            "game.reward.grant",
            EffectAbi::event_emit(),
            AccessAbi::public(),
        )
        .event("quest.complete"),
    );
    let initial =
        compile_initial_with_abi(SourceId::new(1), "fn main() { return 1; }", old_abi.clone())
            .expect("initial");

    let error = compile_update_with_abi(
        &initial,
        SourceId::new(2),
        "fn main() { return 2; }",
        changed_event,
    )
    .expect_err("event change should fail");
    assert_eq!(
        error.kind,
        HotReloadErrorKind::ChangedFunctionEvent {
            function: "game.reward.grant".to_owned(),
            old: Some("monster.kill".to_owned()),
            new: Some("quest.complete".to_owned()),
            source_span: None,
        }
    );

    let report = HotReloadReport::rejected(ProgramVersionId(7), error);
    assert_eq!(report.errors[0].code, "reload.function.event_changed");
    assert_eq!(
        report.errors[0].detail,
        Some(HotReloadDiagnosticDetail::FunctionEventAbi {
            old: Some("monster.kill".to_owned()),
            new: Some("quest.complete".to_owned()),
        })
    );
    assert!(
        report
            .render_lines()
            .iter()
            .any(|line| { line.text == "function event: old=monster.kill new=quest.complete" })
    );

    let removed_event = HotReloadAbi::empty().function(FunctionAbi::new(
        "game.reward.grant",
        EffectAbi::event_emit(),
        AccessAbi::public(),
    ));
    let error = compile_update_with_abi(
        &initial,
        SourceId::new(3),
        "fn main() { return 3; }",
        removed_event,
    )
    .expect_err("removed event should fail");
    assert_eq!(
        error.kind,
        HotReloadErrorKind::ChangedFunctionEvent {
            function: "game.reward.grant".to_owned(),
            old: Some("monster.kill".to_owned()),
            new: None,
            source_span: None,
        }
    );
}

#[test]
fn function_descriptor_parameter_abi_changes_are_rejected() {
    let span = Span::new(SourceId::new(8), 20, 45);
    let old_abi = HotReloadAbi::empty().function(
        FunctionAbi::new(
            "game.reward.grant",
            EffectAbi::host_read(),
            AccessAbi::public(),
        )
        .param(ParamAbi::new("player").type_hint("Player"))
        .param(ParamAbi::new("amount").type_hint("int")),
    );
    let changed_param = HotReloadAbi::empty().function(
        FunctionAbi::new(
            "game.reward.grant",
            EffectAbi::host_read(),
            AccessAbi::public(),
        )
        .param(ParamAbi::new("player").type_hint("Player"))
        .param(ParamAbi::new("amount").type_hint("float"))
        .source_span(span),
    );
    let initial =
        compile_initial_with_abi(SourceId::new(1), "fn main() { return 1; }", old_abi.clone())
            .expect("initial");

    let error = compile_update_with_abi(
        &initial,
        SourceId::new(2),
        "fn main() { return 2; }",
        changed_param,
    )
    .expect_err("parameter ABI change should fail");
    assert_eq!(
        error.kind,
        HotReloadErrorKind::ChangedFunctionParameterAbi {
            function: "game.reward.grant".to_owned(),
            old: vec![
                ParamAbi::new("player").type_hint("Player"),
                ParamAbi::new("amount").type_hint("int"),
            ],
            new: vec![
                ParamAbi::new("player").type_hint("Player"),
                ParamAbi::new("amount").type_hint("float"),
            ],
            source_span: Some(Box::new(span)),
        }
    );
    let report = HotReloadReport::rejected(ProgramVersionId(8), error);
    assert_eq!(
        report.errors[0].code,
        "reload.function.parameter_abi_changed"
    );
    assert_eq!(
        report.errors[0].detail,
        Some(HotReloadDiagnosticDetail::FunctionParameterAbiList {
            old: vec![
                ParamAbi::new("player").type_hint("Player"),
                ParamAbi::new("amount").type_hint("int"),
            ],
            new: vec![
                ParamAbi::new("player").type_hint("Player"),
                ParamAbi::new("amount").type_hint("float"),
            ],
        })
    );
    assert_eq!(report.errors[0].source_span, Some(span));
    assert!(report.render_lines().iter().any(|line| {
        line.text
            == "parameter ABI: old=(player:Player, amount:int) new=(player:Player, amount:float)"
    }));

    let added_required = HotReloadAbi::empty().function(
        FunctionAbi::new(
            "game.reward.grant",
            EffectAbi::host_read(),
            AccessAbi::public(),
        )
        .param(ParamAbi::new("player").type_hint("Player"))
        .param(ParamAbi::new("amount").type_hint("int"))
        .param(ParamAbi::new("reason").type_hint("string")),
    );
    let error = compile_update_with_abi(
        &initial,
        SourceId::new(3),
        "fn main() { return 3; }",
        added_required,
    )
    .expect_err("added required parameter should fail");
    assert_eq!(
        error.kind,
        HotReloadErrorKind::AddedFunctionParametersWithoutDefaults {
            function: "game.reward.grant".to_owned(),
            added: vec!["reason".to_owned()],
        }
    );

    let added_defaulted = HotReloadAbi::empty().function(
        FunctionAbi::new(
            "game.reward.grant",
            EffectAbi::host_read(),
            AccessAbi::public(),
        )
        .param(ParamAbi::new("player").type_hint("Player"))
        .param(ParamAbi::new("amount").type_hint("int"))
        .param(ParamAbi::new("reason").type_hint("string").defaulted(true)),
    );
    compile_update_with_abi(
        &initial,
        SourceId::new(4),
        "fn main() { return 4; }",
        added_defaulted,
    )
    .expect("added defaulted descriptor parameter should be accepted");
}

#[test]
fn function_descriptor_return_abi_changes_are_rejected() {
    let span = Span::new(SourceId::new(13), 15, 35);
    let old_abi = HotReloadAbi::empty().function(
        FunctionAbi::new(
            "game.reward.grant",
            EffectAbi::host_read(),
            AccessAbi::public(),
        )
        .return_type("int"),
    );
    let changed_return = HotReloadAbi::empty().function(
        FunctionAbi::new(
            "game.reward.grant",
            EffectAbi::host_read(),
            AccessAbi::public(),
        )
        .return_type("float")
        .source_span(span),
    );
    let initial = compile_initial_with_abi(SourceId::new(1), "fn main() { return 1; }", old_abi)
        .expect("initial");

    let error = compile_update_with_abi(
        &initial,
        SourceId::new(2),
        "fn main() { return 2; }",
        changed_return,
    )
    .expect_err("return ABI change should fail");
    assert_eq!(
        error.kind,
        HotReloadErrorKind::ChangedFunctionReturnAbi {
            function: "game.reward.grant".to_owned(),
            old: Some("int".to_owned()),
            new: Some("float".to_owned()),
            source_span: Some(Box::new(span)),
        }
    );
    let report = HotReloadReport::rejected(ProgramVersionId(13), error);
    assert_eq!(report.errors[0].code, "reload.function.return_abi_changed");
    assert_eq!(
        report.errors[0].detail,
        Some(HotReloadDiagnosticDetail::FunctionReturnAbi {
            old: Some("int".to_owned()),
            new: Some("float".to_owned()),
        })
    );
    assert_eq!(report.errors[0].source_span, Some(span));
    assert!(
        report
            .render_lines()
            .iter()
            .any(|line| line.text == "function return ABI: old=int new=float")
    );
}

#[test]
fn removed_function_abi_is_rejected() {
    let span = Span::new(SourceId::new(9), 10, 25);
    let old_abi = HotReloadAbi::empty().function(
        FunctionAbi::new(
            "game.reward.grant",
            EffectAbi::host_read(),
            AccessAbi::new(true, true, vec!["reward.read".to_owned()]),
        )
        .source_span(span),
    );
    let initial = compile_initial_with_abi(SourceId::new(1), "fn main() { return 1; }", old_abi)
        .expect("initial");

    let error = compile_update_with_abi(
        &initial,
        SourceId::new(2),
        "fn main() { return 2; }",
        HotReloadAbi::empty(),
    )
    .expect_err("removed function ABI should fail");

    assert_eq!(
        error.kind,
        HotReloadErrorKind::RemovedFunctionAbi {
            function: "game.reward.grant".to_owned(),
            source_span: Some(Box::new(span)),
        }
    );
    let report = HotReloadReport::rejected(ProgramVersionId(3), error);
    assert_eq!(report.errors[0].code, "reload.function.removed_abi");
    assert_eq!(
        report.errors[0].repair_hint.as_deref(),
        Some("restore the function ABI entry or restart with an explicit migration")
    );
    assert_eq!(report.errors[0].source_span, Some(span));
}

#[test]
fn method_effect_and_access_abi_changes_are_rejected() {
    let old_abi = HotReloadAbi::empty().method(MethodAbi::new(
        "Player",
        "grant_exp",
        EffectAbi::host_write(),
        AccessAbi::new(true, true, vec!["player.write".to_owned()]),
    ));
    let changed_effects = HotReloadAbi::empty().method(MethodAbi::new(
        "Player",
        "grant_exp",
        EffectAbi::host_read(),
        AccessAbi::new(true, true, vec!["player.write".to_owned()]),
    ));
    let initial =
        compile_initial_with_abi(SourceId::new(1), "fn main() { return 1; }", old_abi.clone())
            .expect("initial");

    let error = compile_update_with_abi(
        &initial,
        SourceId::new(2),
        "fn main() { return 2; }",
        changed_effects,
    )
    .expect_err("method effect change should fail");
    assert_eq!(
        error.kind,
        HotReloadErrorKind::ChangedMethodEffects {
            type_name: "Player".to_owned(),
            method: "grant_exp".to_owned(),
            old: EffectAbi::host_write(),
            new: EffectAbi::host_read(),
            source_span: None,
        }
    );

    let changed_access = HotReloadAbi::empty().method(MethodAbi::new(
        "Player",
        "grant_exp",
        EffectAbi::host_write(),
        AccessAbi::new(true, false, vec!["player.write".to_owned()]),
    ));
    let error = compile_update_with_abi(
        &initial,
        SourceId::new(3),
        "fn main() { return 3; }",
        changed_access,
    )
    .expect_err("method access change should fail");
    assert_eq!(
        error.kind,
        HotReloadErrorKind::ChangedMethodAccess {
            type_name: "Player".to_owned(),
            method: "grant_exp".to_owned(),
            old: AccessAbi::new(true, true, vec!["player.write".to_owned()]),
            new: AccessAbi::new(true, false, vec!["player.write".to_owned()]),
            source_span: None,
        }
    );
}

#[test]
fn method_descriptor_parameter_abi_changes_are_rejected() {
    let span = Span::new(SourceId::new(12), 40, 70);
    let old_abi = HotReloadAbi::empty().method(
        MethodAbi::new(
            "Player",
            "grant_exp",
            EffectAbi::host_write(),
            AccessAbi::public(),
        )
        .param(ParamAbi::new("amount").type_hint("int")),
    );
    let changed_param = HotReloadAbi::empty().method(
        MethodAbi::new(
            "Player",
            "grant_exp",
            EffectAbi::host_write(),
            AccessAbi::public(),
        )
        .param(ParamAbi::new("amount").type_hint("float"))
        .source_span(span),
    );
    let initial =
        compile_initial_with_abi(SourceId::new(1), "fn main() { return 1; }", old_abi.clone())
            .expect("initial");

    let error = compile_update_with_abi(
        &initial,
        SourceId::new(2),
        "fn main() { return 2; }",
        changed_param,
    )
    .expect_err("method parameter ABI change should fail");
    assert_eq!(
        error.kind,
        HotReloadErrorKind::ChangedMethodParameterAbi {
            type_name: "Player".to_owned(),
            method: "grant_exp".to_owned(),
            old: vec![ParamAbi::new("amount").type_hint("int")],
            new: vec![ParamAbi::new("amount").type_hint("float")],
            source_span: Some(Box::new(span)),
        }
    );
    let report = HotReloadReport::rejected(ProgramVersionId(12), error);
    assert_eq!(report.errors[0].code, "reload.method.parameter_abi_changed");
    assert_eq!(
        report.errors[0].detail,
        Some(HotReloadDiagnosticDetail::MethodParameterAbiList {
            old: vec![ParamAbi::new("amount").type_hint("int")],
            new: vec![ParamAbi::new("amount").type_hint("float")],
        })
    );
    assert_eq!(report.errors[0].source_span, Some(span));
    assert!(
        report.render_lines().iter().any(|line| {
            line.text == "method parameter ABI: old=(amount:int) new=(amount:float)"
        })
    );

    let added_required = HotReloadAbi::empty().method(
        MethodAbi::new(
            "Player",
            "grant_exp",
            EffectAbi::host_write(),
            AccessAbi::public(),
        )
        .param(ParamAbi::new("amount").type_hint("int"))
        .param(ParamAbi::new("reason").type_hint("string")),
    );
    let error = compile_update_with_abi(
        &initial,
        SourceId::new(3),
        "fn main() { return 3; }",
        added_required,
    )
    .expect_err("added required method parameter should fail");
    assert_eq!(error.code(), "reload.method.parameter_abi_changed");

    let added_defaulted = HotReloadAbi::empty().method(
        MethodAbi::new(
            "Player",
            "grant_exp",
            EffectAbi::host_write(),
            AccessAbi::public(),
        )
        .param(ParamAbi::new("amount").type_hint("int"))
        .param(ParamAbi::new("reason").type_hint("string").defaulted(true)),
    );
    compile_update_with_abi(
        &initial,
        SourceId::new(4),
        "fn main() { return 4; }",
        added_defaulted,
    )
    .expect("added defaulted method descriptor parameter should be accepted");
}

#[test]
fn method_descriptor_return_abi_changes_are_rejected() {
    let span = Span::new(SourceId::new(14), 60, 95);
    let old_abi = HotReloadAbi::empty().method(
        MethodAbi::new(
            "Player",
            "grant_exp",
            EffectAbi::host_write(),
            AccessAbi::public(),
        )
        .return_type("int"),
    );
    let changed_return = HotReloadAbi::empty().method(
        MethodAbi::new(
            "Player",
            "grant_exp",
            EffectAbi::host_write(),
            AccessAbi::public(),
        )
        .return_type("null")
        .source_span(span),
    );
    let initial = compile_initial_with_abi(SourceId::new(1), "fn main() { return 1; }", old_abi)
        .expect("initial");

    let error = compile_update_with_abi(
        &initial,
        SourceId::new(2),
        "fn main() { return 2; }",
        changed_return,
    )
    .expect_err("method return ABI change should fail");
    assert_eq!(
        error.kind,
        HotReloadErrorKind::ChangedMethodReturnAbi {
            type_name: "Player".to_owned(),
            method: "grant_exp".to_owned(),
            old: Some("int".to_owned()),
            new: Some("null".to_owned()),
            source_span: Some(Box::new(span)),
        }
    );
    let report = HotReloadReport::rejected(ProgramVersionId(14), error);
    assert_eq!(report.errors[0].code, "reload.method.return_abi_changed");
    assert_eq!(
        report.errors[0].detail,
        Some(HotReloadDiagnosticDetail::MethodReturnAbi {
            old: Some("int".to_owned()),
            new: Some("null".to_owned()),
        })
    );
    assert_eq!(report.errors[0].source_span, Some(span));
    assert!(
        report
            .render_lines()
            .iter()
            .any(|line| line.text == "method return ABI: old=int new=null")
    );
}

#[test]
fn trait_descriptor_abi_changes_are_rejected() {
    let span = Span::new(SourceId::new(15), 100, 140);
    let old_abi = HotReloadAbi::empty().trait_abi(
        TraitAbi::new("Damageable").method(
            TraitMethodAbi::new(1, "damage")
                .param(ParamAbi::new("amount").type_hint("int"))
                .return_type("int"),
        ),
    );
    let changed_return = HotReloadAbi::empty().trait_abi(
        TraitAbi::new("Damageable")
            .method(
                TraitMethodAbi::new(1, "damage")
                    .param(ParamAbi::new("amount").type_hint("int"))
                    .return_type("float"),
            )
            .source_span(span),
    );

    let error = old_abi
        .ensure_compatible_update(&changed_return)
        .expect_err("trait method return ABI change should fail");
    assert_eq!(
        error.kind,
        HotReloadErrorKind::ChangedTraitAbi {
            trait_name: "Damageable".to_owned(),
            old: vec![
                TraitMethodAbi::new(1, "damage")
                    .param(ParamAbi::new("amount").type_hint("int"))
                    .return_type("int"),
            ],
            new: vec![
                TraitMethodAbi::new(1, "damage")
                    .param(ParamAbi::new("amount").type_hint("int"))
                    .return_type("float"),
            ],
            source_span: Some(Box::new(span)),
        }
    );
    let report = HotReloadReport::rejected(ProgramVersionId(15), error);
    assert_eq!(report.errors[0].code, "reload.trait.changed_abi");
    assert_eq!(
        report.errors[0].detail,
        Some(HotReloadDiagnosticDetail::TraitMethodAbiList {
            old: vec![
                TraitMethodAbi::new(1, "damage")
                    .param(ParamAbi::new("amount").type_hint("int"))
                    .return_type("int"),
            ],
            new: vec![
                TraitMethodAbi::new(1, "damage")
                    .param(ParamAbi::new("amount").type_hint("int"))
                    .return_type("float"),
            ],
        })
    );
    assert_eq!(report.errors[0].source_span, Some(span));
    assert!(report.render_lines().iter().any(|line| {
        line.text
            == "trait method ABI: old=(damage#1(amount:int)->int) new=(damage#1(amount:int)->float)"
    }));

    let added_required = HotReloadAbi::empty().trait_abi(
        TraitAbi::new("Damageable")
            .method(
                TraitMethodAbi::new(1, "damage")
                    .param(ParamAbi::new("amount").type_hint("int"))
                    .return_type("int"),
            )
            .method(
                TraitMethodAbi::new(2, "heal")
                    .param(ParamAbi::new("amount").type_hint("int"))
                    .return_type("int"),
            ),
    );
    let error = old_abi
        .ensure_compatible_update(&added_required)
        .expect_err("added required trait method should fail");
    assert_eq!(error.code(), "reload.trait.changed_abi");

    let added_defaulted = HotReloadAbi::empty().trait_abi(
        TraitAbi::new("Damageable")
            .method(
                TraitMethodAbi::new(1, "damage")
                    .param(ParamAbi::new("amount").type_hint("int"))
                    .return_type("int"),
            )
            .method(
                TraitMethodAbi::new(2, "heal")
                    .param(ParamAbi::new("amount").type_hint("int"))
                    .return_type("int")
                    .defaulted(true),
            ),
    );
    old_abi
        .ensure_compatible_update(&added_defaulted)
        .expect("added defaulted trait method should be accepted");
}

#[test]
fn removed_trait_abi_is_rejected() {
    let span = Span::new(SourceId::new(16), 5, 25);
    let old_abi = HotReloadAbi::empty().trait_abi(TraitAbi::new("Damageable").source_span(span));

    let error = old_abi
        .ensure_compatible_update(&HotReloadAbi::empty())
        .expect_err("removed trait ABI should fail");
    assert_eq!(
        error.kind,
        HotReloadErrorKind::RemovedTraitAbi {
            trait_name: "Damageable".to_owned(),
            source_span: Some(Box::new(span)),
        }
    );
    let report = HotReloadReport::rejected(ProgramVersionId(16), error);
    assert_eq!(report.errors[0].code, "reload.trait.removed_abi");
    assert_eq!(
        report.errors[0].repair_hint.as_deref(),
        Some("restore the trait ABI entry or restart with an explicit migration")
    );
    assert_eq!(report.errors[0].source_span, Some(span));
}

#[test]
fn removed_method_abi_is_rejected() {
    let span = Span::new(SourceId::new(9), 30, 45);
    let old_abi = HotReloadAbi::empty().method(
        MethodAbi::new(
            "Player",
            "grant_exp",
            EffectAbi::host_write(),
            AccessAbi::new(true, true, vec!["player.write".to_owned()]),
        )
        .source_span(span),
    );
    let initial = compile_initial_with_abi(SourceId::new(1), "fn main() { return 1; }", old_abi)
        .expect("initial");

    let error = compile_update_with_abi(
        &initial,
        SourceId::new(2),
        "fn main() { return 2; }",
        HotReloadAbi::empty(),
    )
    .expect_err("removed method ABI should fail");

    assert_eq!(
        error.kind,
        HotReloadErrorKind::RemovedMethodAbi {
            type_name: "Player".to_owned(),
            method: "grant_exp".to_owned(),
            source_span: Some(Box::new(span)),
        }
    );
    let report = HotReloadReport::rejected(ProgramVersionId(3), error);
    assert_eq!(report.errors[0].code, "reload.method.removed_abi");
    assert_eq!(
        report.errors[0].repair_hint.as_deref(),
        Some("restore the method ABI entry or restart with an explicit migration")
    );
    assert_eq!(report.errors[0].source_span, Some(span));
}

#[test]
fn registry_abi_rejections_carry_new_declaration_spans() {
    let schema_span = Span::new(SourceId::new(9), 10, 25);
    let mut old_registry = TypeRegistry::new();
    old_registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(1), "Reward"))
            .schema_hash(SchemaHash::new(0x1111))
            .source_span(Span::new(SourceId::new(1), 1, 8)),
    );
    let mut new_registry = TypeRegistry::new();
    new_registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(1), "Reward"))
            .schema_hash(SchemaHash::new(0x2222))
            .source_span(schema_span),
    );

    let error = HotReloadAbi::from_registry(&old_registry)
        .ensure_compatible_update(&HotReloadAbi::from_registry(&new_registry))
        .expect_err("schema hash change should fail");
    assert_eq!(error.source_span(), Some(schema_span));
    let report = HotReloadReport::rejected(ProgramVersionId(1), error);
    assert_eq!(report.errors[0].source_span, Some(schema_span));
    assert!(
        report
            .render_lines()
            .iter()
            .any(|line| line.kind == HotReloadReportLineKind::Diagnostic
                && line.span == Some(schema_span))
    );

    let function_span = Span::new(SourceId::new(10), 30, 50);
    let old_abi = HotReloadAbi::empty().function(FunctionAbi::new(
        "game.reward.grant",
        EffectAbi::host_read(),
        AccessAbi::public(),
    ));
    let new_abi = HotReloadAbi::empty().function(
        FunctionAbi::new(
            "game.reward.grant",
            EffectAbi::host_write(),
            AccessAbi::public(),
        )
        .source_span(function_span),
    );
    let error = old_abi
        .ensure_compatible_update(&new_abi)
        .expect_err("function effect change should fail");
    assert_eq!(error.source_span(), Some(function_span));

    let method_span = Span::new(SourceId::new(11), 60, 75);
    let old_abi = HotReloadAbi::empty().method(MethodAbi::new(
        "Player",
        "grant_exp",
        EffectAbi::host_write(),
        AccessAbi::public(),
    ));
    let new_abi = HotReloadAbi::empty().method(
        MethodAbi::new(
            "Player",
            "grant_exp",
            EffectAbi::host_read(),
            AccessAbi::public(),
        )
        .source_span(method_span),
    );
    let error = old_abi
        .ensure_compatible_update(&new_abi)
        .expect_err("method effect change should fail");
    assert_eq!(error.source_span(), Some(method_span));
}

#[test]
fn trait_abi_manifest_can_be_built_from_type_registry() {
    let mut old_registry = TypeRegistry::new();
    old_registry.register_trait(
        TraitDesc::new("Damageable").method(
            TraitMethodDesc::new(MethodId::new(1), "damage")
                .param(MethodParamDesc::new("amount").type_hint("int"))
                .return_type("int"),
        ),
    );

    let mut reordered_registry = TypeRegistry::new();
    reordered_registry.register_trait(
        TraitDesc::new("Damageable")
            .method(
                TraitMethodDesc::new(MethodId::new(2), "heal")
                    .param(MethodParamDesc::new("amount").type_hint("int"))
                    .return_type("int")
                    .defaulted(true),
            )
            .method(
                TraitMethodDesc::new(MethodId::new(1), "damage")
                    .param(MethodParamDesc::new("amount").type_hint("int"))
                    .return_type("int"),
            ),
    );

    HotReloadAbi::from_registry(&old_registry)
        .ensure_compatible_update(&HotReloadAbi::from_registry(&reordered_registry))
        .expect("reordered trait methods plus defaulted additions should be accepted");

    let mut changed_registry = TypeRegistry::new();
    changed_registry.register_trait(
        TraitDesc::new("Damageable").method(
            TraitMethodDesc::new(MethodId::new(1), "damage")
                .param(MethodParamDesc::new("amount").type_hint("float"))
                .return_type("int"),
        ),
    );
    let error = HotReloadAbi::from_registry(&old_registry)
        .ensure_compatible_update(&HotReloadAbi::from_registry(&changed_registry))
        .expect_err("changed registry trait method ABI should be rejected");
    assert_eq!(error.code(), "reload.trait.changed_abi");
}

#[test]
fn abi_manifest_can_be_built_from_type_registry() {
    let player = TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
        .schema_hash(SchemaHash::new(0xfeed))
        .method(
            MethodDesc::new(HostMethodId::new(9), "grant_exp")
                .param(MethodParamDesc::new("amount").type_hint("int"))
                .return_type("int")
                .effects(MethodEffectSet::host_write())
                .access(
                    MethodAccess::new()
                        .reflect_callable(true)
                        .require_permission("player.write"),
                ),
        );
    let mut registry = TypeRegistry::new();
    registry.register(player);
    registry.register_function(
        FunctionDesc::new(FunctionId::new(11), "game.reward.grant")
            .param(FunctionParamDesc::new("player").type_hint("Player"))
            .param(FunctionParamDesc::new("amount").type_hint("int"))
            .return_type("int")
            .effects(FunctionEffectSet::event_emit())
            .access(
                FunctionAccess::new()
                    .reflect_visible(true)
                    .require_permission("reward.grant"),
            )
            .attr("event", "monster.kill"),
    );

    let abi = HotReloadAbi::from_registry(&registry);
    let initial =
        compile_initial_with_abi(SourceId::new(1), "fn main() { return 1; }", abi.clone())
            .expect("initial");

    compile_update_with_abi(&initial, SourceId::new(2), "fn main() { return 2; }", abi)
        .expect("unchanged registry ABI should be accepted");

    let mut changed_registry = TypeRegistry::new();
    changed_registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
            .schema_hash(SchemaHash::new(0xfeed))
            .method(
                MethodDesc::new(HostMethodId::new(9), "grant_exp")
                    .param(MethodParamDesc::new("amount").type_hint("int"))
                    .return_type("int")
                    .effects(MethodEffectSet::host_write())
                    .access(
                        MethodAccess::new()
                            .reflect_callable(true)
                            .require_permission("player.write"),
                    ),
            ),
    );
    changed_registry.register_function(
        FunctionDesc::new(FunctionId::new(11), "game.reward.grant")
            .param(FunctionParamDesc::new("player").type_hint("Player"))
            .param(FunctionParamDesc::new("amount").type_hint("int"))
            .return_type("int")
            .effects(FunctionEffectSet::event_emit())
            .access(
                FunctionAccess::new()
                    .reflect_visible(true)
                    .require_permission("reward.grant"),
            )
            .attr("event", "quest.complete"),
    );

    let error = compile_update_with_abi(
        &initial,
        SourceId::new(3),
        "fn main() { return 3; }",
        HotReloadAbi::from_registry(&changed_registry),
    )
    .expect_err("changed registry event binding should be rejected");
    assert_eq!(error.code(), "reload.function.event_changed");

    let mut changed_param_registry = TypeRegistry::new();
    changed_param_registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
            .schema_hash(SchemaHash::new(0xfeed))
            .method(
                MethodDesc::new(HostMethodId::new(9), "grant_exp")
                    .param(MethodParamDesc::new("amount").type_hint("int"))
                    .return_type("int")
                    .effects(MethodEffectSet::host_write())
                    .access(
                        MethodAccess::new()
                            .reflect_callable(true)
                            .require_permission("player.write"),
                    ),
            ),
    );
    changed_param_registry.register_function(
        FunctionDesc::new(FunctionId::new(11), "game.reward.grant")
            .param(FunctionParamDesc::new("player").type_hint("Player"))
            .param(FunctionParamDesc::new("amount").type_hint("float"))
            .return_type("int")
            .effects(FunctionEffectSet::event_emit())
            .access(
                FunctionAccess::new()
                    .reflect_visible(true)
                    .require_permission("reward.grant"),
            )
            .attr("event", "monster.kill"),
    );

    let error = compile_update_with_abi(
        &initial,
        SourceId::new(4),
        "fn main() { return 4; }",
        HotReloadAbi::from_registry(&changed_param_registry),
    )
    .expect_err("changed registry parameter ABI should be rejected");
    assert_eq!(error.code(), "reload.function.parameter_abi_changed");

    let mut changed_method_param_registry = TypeRegistry::new();
    changed_method_param_registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
            .schema_hash(SchemaHash::new(0xfeed))
            .method(
                MethodDesc::new(HostMethodId::new(9), "grant_exp")
                    .param(MethodParamDesc::new("amount").type_hint("float"))
                    .return_type("int")
                    .effects(MethodEffectSet::host_write())
                    .access(
                        MethodAccess::new()
                            .reflect_callable(true)
                            .require_permission("player.write"),
                    ),
            ),
    );
    changed_method_param_registry.register_function(
        FunctionDesc::new(FunctionId::new(11), "game.reward.grant")
            .param(FunctionParamDesc::new("player").type_hint("Player"))
            .param(FunctionParamDesc::new("amount").type_hint("int"))
            .return_type("int")
            .effects(FunctionEffectSet::event_emit())
            .access(
                FunctionAccess::new()
                    .reflect_visible(true)
                    .require_permission("reward.grant"),
            )
            .attr("event", "monster.kill"),
    );

    let error = compile_update_with_abi(
        &initial,
        SourceId::new(5),
        "fn main() { return 5; }",
        HotReloadAbi::from_registry(&changed_method_param_registry),
    )
    .expect_err("changed registry method parameter ABI should be rejected");
    assert_eq!(error.code(), "reload.method.parameter_abi_changed");

    let mut changed_function_return_registry = TypeRegistry::new();
    changed_function_return_registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
            .schema_hash(SchemaHash::new(0xfeed))
            .method(
                MethodDesc::new(HostMethodId::new(9), "grant_exp")
                    .param(MethodParamDesc::new("amount").type_hint("int"))
                    .return_type("int")
                    .effects(MethodEffectSet::host_write())
                    .access(
                        MethodAccess::new()
                            .reflect_callable(true)
                            .require_permission("player.write"),
                    ),
            ),
    );
    changed_function_return_registry.register_function(
        FunctionDesc::new(FunctionId::new(11), "game.reward.grant")
            .param(FunctionParamDesc::new("player").type_hint("Player"))
            .param(FunctionParamDesc::new("amount").type_hint("int"))
            .return_type("float")
            .effects(FunctionEffectSet::event_emit())
            .access(
                FunctionAccess::new()
                    .reflect_visible(true)
                    .require_permission("reward.grant"),
            )
            .attr("event", "monster.kill"),
    );

    let error = compile_update_with_abi(
        &initial,
        SourceId::new(6),
        "fn main() { return 6; }",
        HotReloadAbi::from_registry(&changed_function_return_registry),
    )
    .expect_err("changed registry function return ABI should be rejected");
    assert_eq!(error.code(), "reload.function.return_abi_changed");

    let mut changed_method_return_registry = TypeRegistry::new();
    changed_method_return_registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
            .schema_hash(SchemaHash::new(0xfeed))
            .method(
                MethodDesc::new(HostMethodId::new(9), "grant_exp")
                    .param(MethodParamDesc::new("amount").type_hint("int"))
                    .return_type("float")
                    .effects(MethodEffectSet::host_write())
                    .access(
                        MethodAccess::new()
                            .reflect_callable(true)
                            .require_permission("player.write"),
                    ),
            ),
    );
    changed_method_return_registry.register_function(
        FunctionDesc::new(FunctionId::new(11), "game.reward.grant")
            .param(FunctionParamDesc::new("player").type_hint("Player"))
            .param(FunctionParamDesc::new("amount").type_hint("int"))
            .return_type("int")
            .effects(FunctionEffectSet::event_emit())
            .access(
                FunctionAccess::new()
                    .reflect_visible(true)
                    .require_permission("reward.grant"),
            )
            .attr("event", "monster.kill"),
    );

    let error = compile_update_with_abi(
        &initial,
        SourceId::new(7),
        "fn main() { return 7; }",
        HotReloadAbi::from_registry(&changed_method_return_registry),
    )
    .expect_err("changed registry method return ABI should be rejected");
    assert_eq!(error.code(), "reload.method.return_abi_changed");
}
