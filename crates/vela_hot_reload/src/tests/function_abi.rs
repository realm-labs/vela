use super::*;

#[test]
fn function_effect_and_access_abi_changes_are_rejected() {
    let old_abi = HotReloadAbi::empty().function(FunctionAbi::new(
        "game::reward::grant",
        EffectAbi::host_read(),
        AccessAbi::new(true, true),
    ));
    let changed_effects = HotReloadAbi::empty().function(FunctionAbi::new(
        "game::reward::grant",
        EffectAbi::host_write(),
        AccessAbi::new(true, true),
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
            function: "game::reward::grant".to_owned(),
            old: EffectAbi::host_read(),
            new: EffectAbi::host_write(),
            source_span: None,
        }
    );

    let changed_access = HotReloadAbi::empty().function(FunctionAbi::new(
        "game::reward::grant",
        EffectAbi::host_read(),
        AccessAbi::new(false, true),
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
            function: "game::reward::grant".to_owned(),
            old: AccessAbi::new(true, true),
            new: AccessAbi::new(false, true),
            source_span: None,
        }
    );

    let changed_callability = HotReloadAbi::empty().function(FunctionAbi::new(
        "game::reward::grant",
        EffectAbi::host_read(),
        AccessAbi::function(true, true, false),
    ));
    let error = compile_update_with_abi(
        &initial,
        SourceId::new(4),
        "fn main() { return 4; }",
        changed_callability,
    )
    .expect_err("function callability change should fail");
    assert_eq!(
        error.kind,
        HotReloadErrorKind::ChangedFunctionAccess {
            function: "game::reward::grant".to_owned(),
            old: AccessAbi::new(true, true),
            new: AccessAbi::function(true, true, false),
            source_span: None,
        }
    );
}

#[test]
fn function_event_abi_changes_are_rejected() {
    let old_abi = HotReloadAbi::empty().function(
        FunctionAbi::new(
            "game::reward::grant",
            EffectAbi::event_emit(),
            AccessAbi::public(),
        )
        .event("monster.kill"),
    );
    let changed_event = HotReloadAbi::empty().function(
        FunctionAbi::new(
            "game::reward::grant",
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
            function: "game::reward::grant".to_owned(),
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
        "game::reward::grant",
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
            function: "game::reward::grant".to_owned(),
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
            "game::reward::grant",
            EffectAbi::host_read(),
            AccessAbi::public(),
        )
        .param(ParamAbi::new("player").type_hint("Player"))
        .param(ParamAbi::new("amount").type_hint("i64")),
    );
    let changed_param = HotReloadAbi::empty().function(
        FunctionAbi::new(
            "game::reward::grant",
            EffectAbi::host_read(),
            AccessAbi::public(),
        )
        .param(ParamAbi::new("player").type_hint("Player"))
        .param(ParamAbi::new("amount").type_hint("f64"))
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
            function: "game::reward::grant".to_owned(),
            old: vec![
                ParamAbi::new("player").type_hint("Player"),
                ParamAbi::new("amount").type_hint("i64"),
            ],
            new: vec![
                ParamAbi::new("player").type_hint("Player"),
                ParamAbi::new("amount").type_hint("f64"),
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
                ParamAbi::new("amount").type_hint("i64"),
            ],
            new: vec![
                ParamAbi::new("player").type_hint("Player"),
                ParamAbi::new("amount").type_hint("f64"),
            ],
        })
    );
    assert_eq!(report.errors[0].source_span, Some(span));
    assert!(report.render_lines().iter().any(|line| {
        line.text
            == "parameter ABI: old=(player:Player, amount:i64) new=(player:Player, amount:f64)"
    }));

    let added_required = HotReloadAbi::empty().function(
        FunctionAbi::new(
            "game::reward::grant",
            EffectAbi::host_read(),
            AccessAbi::public(),
        )
        .param(ParamAbi::new("player").type_hint("Player"))
        .param(ParamAbi::new("amount").type_hint("i64"))
        .param(ParamAbi::new("reason").type_hint("String")),
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
            function: "game::reward::grant".to_owned(),
            added: vec!["reason".to_owned()],
        }
    );

    let added_defaulted = HotReloadAbi::empty().function(
        FunctionAbi::new(
            "game::reward::grant",
            EffectAbi::host_read(),
            AccessAbi::public(),
        )
        .param(ParamAbi::new("player").type_hint("Player"))
        .param(ParamAbi::new("amount").type_hint("i64"))
        .param(ParamAbi::new("reason").type_hint("String").defaulted(true)),
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
fn function_descriptor_primitive_parameter_changes_are_rejected_by_tag() {
    let cases = [
        ("i32", "i64"),
        ("i64", "u64"),
        ("f32", "f64"),
        ("bytes", "string"),
    ];

    for (index, (old_hint, new_hint)) in cases.into_iter().enumerate() {
        let span = Span::new(SourceId::new(80 + index as u32), 20, 45);
        let old_abi = HotReloadAbi::empty().function(
            FunctionAbi::new(
                "game::reward::grant",
                EffectAbi::host_read(),
                AccessAbi::public(),
            )
            .param(ParamAbi::new("amount").type_hint(old_hint)),
        );
        let changed_param = HotReloadAbi::empty().function(
            FunctionAbi::new(
                "game::reward::grant",
                EffectAbi::host_read(),
                AccessAbi::public(),
            )
            .param(ParamAbi::new("amount").type_hint(new_hint))
            .source_span(span),
        );

        let error = old_abi
            .ensure_compatible_update(&changed_param)
            .expect_err("primitive parameter ABI change should fail");
        assert_eq!(error.code(), "reload.function.parameter_abi_changed");
        assert_eq!(error.source_span(), Some(span));
        let report = HotReloadReport::rejected(ProgramVersionId(80 + index as u64), error);
        assert_eq!(report.errors[0].source_span, Some(span));
        assert!(report.render_lines().iter().any(|line| {
            line.text == format!("parameter ABI: old=(amount:{old_hint}) new=(amount:{new_hint})")
        }));
    }
}

#[test]
fn function_descriptor_parameterized_container_changes_are_rejected() {
    let span = Span::new(SourceId::new(91), 20, 45);
    let old_abi = HotReloadAbi::empty().function(
        FunctionAbi::new(
            "game::reward::grant",
            EffectAbi::host_read(),
            AccessAbi::public(),
        )
        .param(ParamAbi::new("values").type_hint("Array<i64>"))
        .param(ParamAbi::new("scores").type_hint("Map<String, i64>")),
    );
    let changed_param = HotReloadAbi::empty().function(
        FunctionAbi::new(
            "game::reward::grant",
            EffectAbi::host_read(),
            AccessAbi::public(),
        )
        .param(ParamAbi::new("values").type_hint("Array<String>"))
        .param(ParamAbi::new("scores").type_hint("Map<String, String>"))
        .source_span(span),
    );

    let error = old_abi
        .ensure_compatible_update(&changed_param)
        .expect_err("parameterized container parameter ABI change should fail");
    assert_eq!(error.code(), "reload.function.parameter_abi_changed");
    assert_eq!(error.source_span(), Some(span));
    let report = HotReloadReport::rejected(ProgramVersionId(91), error);
    assert!(report.render_lines().iter().any(|line| {
        line.text
            == "parameter ABI: old=(values:Array<i64>, scores:Map<String, i64>) new=(values:Array<String>, scores:Map<String, String>)"
    }));
}

#[test]
fn function_descriptor_return_abi_changes_are_rejected() {
    let span = Span::new(SourceId::new(13), 15, 35);
    let old_abi = HotReloadAbi::empty().function(
        FunctionAbi::new(
            "game::reward::grant",
            EffectAbi::host_read(),
            AccessAbi::public(),
        )
        .return_type("i64"),
    );
    let changed_return = HotReloadAbi::empty().function(
        FunctionAbi::new(
            "game::reward::grant",
            EffectAbi::host_read(),
            AccessAbi::public(),
        )
        .return_type("f64")
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
            function: "game::reward::grant".to_owned(),
            old: Some("i64".to_owned()),
            new: Some("f64".to_owned()),
            source_span: Some(Box::new(span)),
        }
    );
    let report = HotReloadReport::rejected(ProgramVersionId(13), error);
    assert_eq!(report.errors[0].code, "reload.function.return_abi_changed");
    assert_eq!(
        report.errors[0].detail,
        Some(HotReloadDiagnosticDetail::FunctionReturnAbi {
            old: Some("i64".to_owned()),
            new: Some("f64".to_owned()),
        })
    );
    assert_eq!(report.errors[0].source_span, Some(span));
    assert!(
        report
            .render_lines()
            .iter()
            .any(|line| line.text == "function return ABI: old=i64 new=f64")
    );
}

#[test]
fn removed_function_abi_is_rejected() {
    let span = Span::new(SourceId::new(9), 10, 25);
    let old_abi = HotReloadAbi::empty().function(
        FunctionAbi::new(
            "game::reward::grant",
            EffectAbi::host_read(),
            AccessAbi::new(true, true),
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
            function: "game::reward::grant".to_owned(),
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
