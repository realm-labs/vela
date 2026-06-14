use super::*;

#[test]
fn method_effect_and_access_abi_changes_are_rejected() {
    let old_abi = HotReloadAbi::empty().method(MethodAbi::new(
        "Player",
        "grant_exp",
        EffectAbi::host_write(),
        AccessAbi::new(true, true),
    ));
    let changed_effects = HotReloadAbi::empty().method(MethodAbi::new(
        "Player",
        "grant_exp",
        EffectAbi::host_read(),
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
        AccessAbi::new(true, false),
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
            old: AccessAbi::new(true, true),
            new: AccessAbi::new(true, false),
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
        .param(ParamAbi::new("amount").type_hint("i64")),
    );
    let changed_param = HotReloadAbi::empty().method(
        MethodAbi::new(
            "Player",
            "grant_exp",
            EffectAbi::host_write(),
            AccessAbi::public(),
        )
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
    .expect_err("method parameter ABI change should fail");
    assert_eq!(
        error.kind,
        HotReloadErrorKind::ChangedMethodParameterAbi {
            type_name: "Player".to_owned(),
            method: "grant_exp".to_owned(),
            old: vec![ParamAbi::new("amount").type_hint("i64")],
            new: vec![ParamAbi::new("amount").type_hint("f64")],
            source_span: Some(Box::new(span)),
        }
    );
    let report = HotReloadReport::rejected(ProgramVersionId(12), error);
    assert_eq!(report.errors[0].code, "reload.method.parameter_abi_changed");
    assert_eq!(
        report.errors[0].detail,
        Some(HotReloadDiagnosticDetail::MethodParameterAbiList {
            old: vec![ParamAbi::new("amount").type_hint("i64")],
            new: vec![ParamAbi::new("amount").type_hint("f64")],
        })
    );
    assert_eq!(report.errors[0].source_span, Some(span));
    assert!(
        report
            .render_lines()
            .iter()
            .any(|line| { line.text == "method parameter ABI: old=(amount:i64) new=(amount:f64)" })
    );

    let added_required = HotReloadAbi::empty().method(
        MethodAbi::new(
            "Player",
            "grant_exp",
            EffectAbi::host_write(),
            AccessAbi::public(),
        )
        .param(ParamAbi::new("amount").type_hint("i64"))
        .param(ParamAbi::new("reason").type_hint("String")),
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
        .param(ParamAbi::new("amount").type_hint("i64"))
        .param(ParamAbi::new("reason").type_hint("String").defaulted(true)),
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
        .return_type("i64"),
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
            old: Some("i64".to_owned()),
            new: Some("null".to_owned()),
            source_span: Some(Box::new(span)),
        }
    );
    let report = HotReloadReport::rejected(ProgramVersionId(14), error);
    assert_eq!(report.errors[0].code, "reload.method.return_abi_changed");
    assert_eq!(
        report.errors[0].detail,
        Some(HotReloadDiagnosticDetail::MethodReturnAbi {
            old: Some("i64".to_owned()),
            new: Some("null".to_owned()),
        })
    );
    assert_eq!(report.errors[0].source_span, Some(span));
    assert!(
        report
            .render_lines()
            .iter()
            .any(|line| line.text == "method return ABI: old=i64 new=null")
    );
}
