use super::*;

#[test]
fn trait_descriptor_abi_changes_are_rejected() {
    let span = Span::new(SourceId::new(15), 100, 140);
    let old_abi = HotReloadAbi::empty().trait_abi(
        TraitAbi::new("Damageable").method(
            TraitMethodAbi::new(1, "damage")
                .param(ParamAbi::new("amount").type_hint("i64"))
                .return_type("i64"),
        ),
    );
    let changed_return = HotReloadAbi::empty().trait_abi(
        TraitAbi::new("Damageable")
            .method(
                TraitMethodAbi::new(1, "damage")
                    .param(ParamAbi::new("amount").type_hint("i64"))
                    .return_type("f64"),
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
                    .param(ParamAbi::new("amount").type_hint("i64"))
                    .return_type("i64"),
            ],
            new: vec![
                TraitMethodAbi::new(1, "damage")
                    .param(ParamAbi::new("amount").type_hint("i64"))
                    .return_type("f64"),
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
                    .param(ParamAbi::new("amount").type_hint("i64"))
                    .return_type("i64"),
            ],
            new: vec![
                TraitMethodAbi::new(1, "damage")
                    .param(ParamAbi::new("amount").type_hint("i64"))
                    .return_type("f64"),
            ],
        })
    );
    assert_eq!(report.errors[0].source_span, Some(span));
    assert!(report.render_lines().iter().any(|line| {
        line.text
            == "trait method ABI: old=(damage#1(amount:i64)->i64) new=(damage#1(amount:i64)->f64)"
    }));

    let added_required = HotReloadAbi::empty().trait_abi(
        TraitAbi::new("Damageable")
            .method(
                TraitMethodAbi::new(1, "damage")
                    .param(ParamAbi::new("amount").type_hint("i64"))
                    .return_type("i64"),
            )
            .method(
                TraitMethodAbi::new(2, "heal")
                    .param(ParamAbi::new("amount").type_hint("i64"))
                    .return_type("i64"),
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
                    .param(ParamAbi::new("amount").type_hint("i64"))
                    .return_type("i64"),
            )
            .method(
                TraitMethodAbi::new(2, "heal")
                    .param(ParamAbi::new("amount").type_hint("i64"))
                    .return_type("i64")
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
fn module_export_abi_changes_are_rejected() {
    let span = Span::new(SourceId::new(17), 40, 80);
    let old_abi = HotReloadAbi::empty().module(
        ModuleAbi::new("game::reward").export(ModuleExportAbi::function("grant_reward", 11)),
    );
    let removed_export =
        HotReloadAbi::empty().module(ModuleAbi::new("game::reward").source_span(span));

    let error = old_abi
        .ensure_compatible_update(&removed_export)
        .expect_err("removed module export should fail");
    assert_eq!(
        error.kind,
        HotReloadErrorKind::ChangedModuleAbi {
            module: "game::reward".to_owned(),
            old: vec![ModuleExportAbi::function("grant_reward", 11)],
            new: vec![],
            source_span: Some(Box::new(span)),
        }
    );
    let report = HotReloadReport::rejected(ProgramVersionId(17), error);
    assert_eq!(report.errors[0].code, "reload.module.changed_abi");
    assert_eq!(
        report.errors[0].detail,
        Some(HotReloadDiagnosticDetail::ModuleExportAbiList {
            old: vec![ModuleExportAbi::function("grant_reward", 11)],
            new: vec![],
        })
    );
    assert_eq!(report.errors[0].source_span, Some(span));
    assert!(report.render_lines().iter().any(|line| {
        line.text == "module export ABI: old=(grant_reward:function#11) new=(<none>)"
    }));

    let changed_export = HotReloadAbi::empty().module(
        ModuleAbi::new("game::reward")
            .export(ModuleExportAbi::function("grant_reward", 12))
            .source_span(span),
    );
    let error = old_abi
        .ensure_compatible_update(&changed_export)
        .expect_err("changed module export target should fail");
    assert_eq!(error.code(), "reload.module.changed_abi");

    let appended_export = HotReloadAbi::empty().module(
        ModuleAbi::new("game::reward")
            .export(ModuleExportAbi::function("grant_reward", 11))
            .export(ModuleExportAbi::function("grant_bonus", 12)),
    );
    old_abi
        .ensure_compatible_update(&appended_export)
        .expect("added module exports should be accepted");
}

#[test]
fn removed_module_abi_is_rejected() {
    let span = Span::new(SourceId::new(18), 5, 25);
    let old_abi = HotReloadAbi::empty().module(ModuleAbi::new("game::reward").source_span(span));

    let error = old_abi
        .ensure_compatible_update(&HotReloadAbi::empty())
        .expect_err("removed module ABI should fail");
    assert_eq!(
        error.kind,
        HotReloadErrorKind::RemovedModuleAbi {
            module: "game::reward".to_owned(),
            source_span: Some(Box::new(span)),
        }
    );
    let report = HotReloadReport::rejected(ProgramVersionId(18), error);
    assert_eq!(report.errors[0].code, "reload.module.removed_abi");
    assert_eq!(
        report.errors[0].repair_hint.as_deref(),
        Some("restore the module ABI entry or restart with an explicit migration")
    );
    assert_eq!(report.errors[0].source_span, Some(span));
}

#[test]
fn module_abi_manifest_can_be_built_from_type_registry() {
    let mut registry = TypeRegistry::new();
    registry.register_module(ModuleDesc::new("game::reward"));
    registry.register_function(
        FunctionDesc::new(FunctionId::new(77), "grant_reward").module("game::reward"),
    );

    let abi = HotReloadAbi::from_registry(&registry);
    let expected = HotReloadAbi::empty()
        .function(FunctionAbi::new(
            "grant_reward",
            EffectAbi::pure(),
            AccessAbi::function(true, true, false),
        ))
        .module(
            ModuleAbi::new("game::reward").export(ModuleExportAbi::function("grant_reward", 77)),
        );

    abi.ensure_compatible_update(&expected)
        .expect("registry module ABI should match expected manifest");
    expected
        .ensure_compatible_update(&abi)
        .expect("expected module ABI should match registry manifest");
}

#[test]
fn removed_method_abi_is_rejected() {
    let span = Span::new(SourceId::new(9), 30, 45);
    let old_abi = HotReloadAbi::empty().method(
        MethodAbi::new(
            "Player",
            "grant_exp",
            EffectAbi::host_write(),
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
