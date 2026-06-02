use super::*;

#[test]
fn function_effect_and_access_abi_changes_are_rejected() {
    let old_abi = HotReloadAbi::empty().function(FunctionAbi::new(
        "game::reward::grant",
        EffectAbi::host_read(),
        AccessAbi::new(true, true, vec!["reward.read".to_owned()]),
    ));
    let changed_effects = HotReloadAbi::empty().function(FunctionAbi::new(
        "game::reward::grant",
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
            function: "game::reward::grant".to_owned(),
            old: EffectAbi::host_read(),
            new: EffectAbi::host_write(),
            source_span: None,
        }
    );

    let changed_access = HotReloadAbi::empty().function(FunctionAbi::new(
        "game::reward::grant",
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
            function: "game::reward::grant".to_owned(),
            old: AccessAbi::new(true, true, vec!["reward.read".to_owned()]),
            new: AccessAbi::new(true, true, vec!["reward.write".to_owned()]),
            source_span: None,
        }
    );

    let changed_callability = HotReloadAbi::empty().function(FunctionAbi::new(
        "game::reward::grant",
        EffectAbi::host_read(),
        AccessAbi::function(true, true, false, vec!["reward.read".to_owned()]),
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
            old: AccessAbi::new(true, true, vec!["reward.read".to_owned()]),
            new: AccessAbi::function(true, true, false, vec!["reward.read".to_owned()]),
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
        .param(ParamAbi::new("amount").type_hint("int")),
    );
    let changed_param = HotReloadAbi::empty().function(
        FunctionAbi::new(
            "game::reward::grant",
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
            function: "game::reward::grant".to_owned(),
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
            "game::reward::grant",
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
            "game::reward::grant",
            EffectAbi::host_read(),
            AccessAbi::public(),
        )
        .return_type("int"),
    );
    let changed_return = HotReloadAbi::empty().function(
        FunctionAbi::new(
            "game::reward::grant",
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
            function: "game::reward::grant".to_owned(),
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
            "game::reward::grant",
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
