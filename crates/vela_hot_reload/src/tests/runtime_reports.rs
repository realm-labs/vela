use super::*;
use vela_bytecode::InstructionOffset;
use vela_bytecode::compiler::options::CompilerOptions;
use vela_hir::module_graph::{ModulePath, ModuleSource};
use vela_vm::owned_value::OwnedValue;

const HOT_RELOAD_PARAMETER_ABI_V1: &str =
    include_str!("../../../../tests/fixtures/diagnostics/hot_reload_parameter_abi_v1.vela");
const HOT_RELOAD_PARAMETER_ABI_V2: &str =
    include_str!("../../../../tests/fixtures/diagnostics/hot_reload_parameter_abi_v2.vela");
const HOT_RELOAD_PARAMETER_ABI_EXPECTED: &str =
    include_str!("../../../../tests/fixtures/diagnostics/hot_reload_parameter_abi.expected");

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
        run_linked_version(&runtime.current(), "main", &[]),
        Ok(OwnedValue::Int(30))
    );
}

#[test]
fn program_version_owns_bytecode_offset_profile_slots() {
    let version = compile_initial(
        SourceId::new(1),
        r#"
fn helper() {
    return 5;
}

fn main() {
    return helper();
}
"#,
    )
    .expect("compile initial");

    let main = version.function("main").expect("main function");
    let main_profile = version.function_profile("main").expect("main profile");
    let helper = version.function("helper").expect("helper function");
    let helper_profile = version.function_profile("helper").expect("helper profile");

    assert_eq!(main_profile.instruction_count(), main.instructions.len());
    assert_eq!(
        helper_profile.instruction_count(),
        helper.instructions.len()
    );
    assert!(main_profile.contains_offset(InstructionOffset(0)));
    assert!(!main_profile.contains_offset(InstructionOffset(main.instructions.len())));
    assert_eq!(
        version.profile().function_names().collect::<Vec<_>>(),
        vec!["helper", "main"]
    );
}

#[test]
fn program_version_owns_program_image_indexes() {
    let version = compile_initial(
        SourceId::new(1),
        r#"
global state: Player;

fn helper() {
    return 5;
}

fn main() {
    return helper();
}
"#,
    )
    .expect("compile initial");

    let helper_index = version
        .program_image()
        .function_index("helper")
        .expect("helper should have image index");
    assert_eq!(
        version
            .program_image()
            .function(helper_index)
            .expect("helper index should resolve")
            .name,
        "helper"
    );
    assert_eq!(
        version.program_image().global_slot("main::state"),
        Some(vela_common::GlobalSlot::new(0))
    );
}

#[test]
fn program_version_preserves_global_layout() {
    let version = compile_initial(
        SourceId::new(1),
        r#"
global state: Player;

fn main() {
    return 1;
}
"#,
    )
    .expect("compile initial");

    assert_eq!(version.global_names(), ["main::state".to_owned()]);
    assert_eq!(
        version.program_image().global_names(),
        version.global_names()
    );
    assert!(
        version.program_image().global_slot("main::state").is_some(),
        "program image should keep global slot metadata"
    );
}

#[test]
fn hot_reload_rebuilds_program_image_for_next_version() {
    let initial =
        compile_initial(SourceId::new(1), "fn main() { return 20; }").expect("compile initial");
    let mut runtime = HotReloadRuntime::new(initial);
    let old = runtime.current();
    assert!(old.program_image().function_index("helper").is_none());

    let update = compile_update(
        &old,
        SourceId::new(2),
        r#"
global state: Player;

fn helper() {
    return 5;
}

fn main() {
    return helper();
}
"#,
    )
    .expect("compile update");

    runtime.apply_hot_update(update).expect("apply update");
    let new = runtime.current();

    assert!(old.program_image().function_index("helper").is_none());
    assert!(new.program_image().function_index("helper").is_some());
    assert_eq!(
        new.program_image().global_slot("main::state"),
        Some(vela_common::GlobalSlot::new(0))
    );
}

#[test]
fn hot_reload_rebuilds_profile_for_next_version() {
    let initial =
        compile_initial(SourceId::new(1), "fn main() { return 20; }").expect("compile initial");
    let mut runtime = HotReloadRuntime::new(initial);
    let old = runtime.current();
    assert!(old.function_profile("helper").is_none());

    let update = compile_update(
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
    )
    .expect("compile update");

    runtime.apply_hot_update(update).expect("apply update");
    let new = runtime.current();

    assert_eq!(old.id, ProgramVersionId(0));
    assert_eq!(new.id, ProgramVersionId(1));
    assert!(old.function_profile("helper").is_none());
    let helper = new.function("helper").expect("new helper function");
    let helper_profile = new.function_profile("helper").expect("new helper profile");
    assert_eq!(
        helper_profile.instruction_count(),
        helper.instructions.len()
    );
}

#[test]
fn hot_reload_rebuilds_global_layout_for_next_version() {
    let initial =
        compile_initial(SourceId::new(1), "fn main() { return 20; }").expect("compile initial");
    let mut runtime = HotReloadRuntime::new(initial);
    let old = runtime.current();
    assert!(old.global_names().is_empty());

    let update = compile_update(
        &old,
        SourceId::new(2),
        r#"
global state: Player;

fn main() {
    return 30;
}
"#,
    )
    .expect("compile update");

    runtime.apply_hot_update(update).expect("apply update");
    let new = runtime.current();

    assert_eq!(old.id, ProgramVersionId(0));
    assert_eq!(new.id, ProgramVersionId(1));
    assert_eq!(new.global_names(), ["main::state".to_owned()]);
    assert!(
        new.program_image().global_slot("main::state").is_some(),
        "new program image should keep global slot metadata"
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
        run_linked_version(&runtime.current(), "main", &[]),
        Ok(OwnedValue::Int(20))
    );

    let report = runtime
        .check_reload()
        .expect("safe point should consume pending update");

    assert!(report.accepted);
    assert_eq!(report.changed_functions, ["main"]);
    assert!(!runtime.has_pending_update());
    assert_eq!(
        run_linked_version(&runtime.current(), "main", &[]),
        Ok(OwnedValue::Int(30))
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
    assert!(runtime.current().function_profile("helper").is_none());
    assert_eq!(
        runtime
            .current()
            .function_profile("main")
            .expect("current main profile")
            .instruction_count(),
        old.function_profile("main")
            .expect("old main profile")
            .instruction_count()
    );
    assert!(!runtime.has_pending_update());
    assert_eq!(report.errors[0].code, "reload.function.new_denied");
    assert_eq!(
        run_linked_version(&runtime.current(), "main", &[]),
        Ok(OwnedValue::Int(20))
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
        run_linked_version(&version, "main", &[]),
        Ok(OwnedValue::Int(5))
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
fn accepted_module_update_report_renders_module_impact() {
    let initial = compile_initial_modules_with_abi_and_options(
        &module_sources(4),
        HotReloadAbi::empty(),
        &CompilerOptions::default(),
    )
    .expect("compile initial modules");
    let mut runtime = HotReloadRuntime::new(initial);
    let update = compile_update_modules_with_abi_and_options_and_policy(
        &runtime.current(),
        &module_sources(7),
        runtime.current().abi().clone(),
        &CompilerOptions::default(),
        &HotReloadPolicy::default(),
    )
    .expect("compile module update");

    let report = runtime.apply_hot_update_report(update);
    let lines = report.render_lines();

    assert_eq!(report.changed_functions, ["game::reward::grant"]);
    assert_eq!(report.changed_modules, ["game::reward"]);
    assert_eq!(report.impacted_modules, ["game::main", "game::reward"]);
    assert!(lines.contains(&HotReloadReportLine::new(
        HotReloadReportLineKind::ChangedModules,
        None,
        None,
        "changed modules: game::reward",
    )));
    assert!(lines.contains(&HotReloadReportLine::new(
        HotReloadReportLineKind::ImpactedModules,
        None,
        None,
        "impacted modules: game::main, game::reward",
    )));
}

#[test]
fn program_version_exposes_read_only_module_and_script_method_metadata() {
    let initial = compile_initial_modules_with_abi_and_options(
        &script_method_module_sources(),
        HotReloadAbi::empty(),
        &CompilerOptions::default(),
    )
    .expect("compile initial modules");
    let runtime = HotReloadRuntime::new(initial);
    let current = runtime.current();
    let function_names = current.function_names().collect::<Vec<_>>();

    assert!(function_names.contains(&"game::main::main"));
    assert!(function_names.contains(&"game::main.__impl.BonusSource.for.game::main::Player.bonus"));

    let method = current
        .script_method("game::main::Player", "bonus")
        .expect("compiled script method should be visible from version metadata");
    assert_eq!(
        method.function,
        "game::main.__impl.BonusSource.for.game::main::Player.bonus"
    );
    assert_eq!(
        current.script_method_by_id("game::main::Player", method.id),
        Some(method)
    );
    assert_eq!(
        current
            .script_method_function("game::main::Player", "bonus")
            .as_ref()
            .map(|function| function.name.as_str()),
        Some("game::main.__impl.BonusSource.for.game::main::Player.bonus")
    );
    assert_eq!(
        current
            .script_method_function_by_id("game::main::Player", method.id)
            .as_ref()
            .map(|function| function.name.as_str()),
        Some("game::main.__impl.BonusSource.for.game::main::Player.bonus")
    );

    let metadata = current
        .script_metadata()
        .expect("compiled modules should preserve module metadata");
    let module = metadata
        .module_id(&ModulePath::from_qualified("game::main"))
        .expect("main module should be indexed");
    assert!(metadata.module_source_hash(module).is_some());
    assert_eq!(
        run_linked_version(&current, "game::main::main", &[]),
        Ok(OwnedValue::Int(12))
    );
}

#[test]
fn program_version_exposes_inherent_script_method_metadata() {
    let initial = compile_initial_modules_with_abi_and_options(
        &inherent_script_method_module_sources(),
        HotReloadAbi::empty(),
        &CompilerOptions::default(),
    )
    .expect("compile initial modules");
    let runtime = HotReloadRuntime::new(initial);
    let current = runtime.current();
    let function_name = "game::main.__impl.game::main::Player.bonus";

    assert!(current.function_names().any(|name| name == function_name));

    let method = current
        .script_method("game::main::Player", "bonus")
        .expect("compiled inherent script method should be visible");
    assert_eq!(method.function, function_name);
    assert_eq!(
        current.script_method_by_id("game::main::Player", method.id),
        Some(method)
    );
    assert_eq!(
        current
            .script_method_function_by_id("game::main::Player", method.id)
            .as_ref()
            .map(|function| function.name.as_str()),
        Some(function_name)
    );
    assert_eq!(
        run_linked_version(&current, "game::main::main", &[]),
        Ok(OwnedValue::Int(12))
    );
}

#[test]
fn hot_update_exposes_read_only_preflight_metadata() {
    let initial = compile_initial_modules_with_abi_and_options(
        &script_method_module_sources(),
        HotReloadAbi::empty(),
        &CompilerOptions::default(),
    )
    .expect("compile initial modules");
    let runtime = HotReloadRuntime::new(initial);
    let update = compile_update_modules_with_abi_and_options_and_policy(
        &runtime.current(),
        &script_method_module_sources_with_bonus("self.level + amount + 1"),
        runtime.current().abi().clone(),
        &CompilerOptions::default(),
        &HotReloadPolicy::default(),
    )
    .expect("compile update");
    let changed_functions = update.changed_function_names().collect::<Vec<_>>();

    assert_eq!(
        changed_functions,
        [
            "game::main.__impl.BonusSource.for.game::main::Player.bonus",
            "game::main::main"
        ]
    );
    assert!(
        update
            .function_names()
            .any(|name| name == "game::main::main")
    );
    assert_eq!(update.changed_modules(), ["game::main"]);
    assert_eq!(update.impacted_modules(), ["game::main"]);

    let method = update
        .script_method("game::main::Player", "bonus")
        .expect("compiled script method should be visible from update metadata");
    assert_eq!(
        update.script_method_by_id("game::main::Player", method.id),
        Some(method)
    );
    assert_eq!(
        update
            .script_method_function("game::main::Player", "bonus")
            .as_ref()
            .map(|function| function.name.as_str()),
        Some("game::main.__impl.BonusSource.for.game::main::Player.bonus")
    );
    assert_eq!(
        update
            .script_method_function_by_id("game::main::Player", method.id)
            .as_ref()
            .map(|function| function.name.as_str()),
        Some("game::main.__impl.BonusSource.for.game::main::Player.bonus")
    );
    let metadata = update
        .script_metadata()
        .expect("compiled update should preserve module metadata");
    assert!(
        metadata
            .module_id(&ModulePath::from_qualified("game::main"))
            .is_some()
    );
}

#[test]
fn hot_update_exposes_inherent_script_method_metadata() {
    let initial = compile_initial_modules_with_abi_and_options(
        &inherent_script_method_module_sources(),
        HotReloadAbi::empty(),
        &CompilerOptions::default(),
    )
    .expect("compile initial modules");
    let runtime = HotReloadRuntime::new(initial);
    let update = compile_update_modules_with_abi_and_options_and_policy(
        &runtime.current(),
        &inherent_script_method_module_sources_with_bonus("self.level + amount + 1"),
        runtime.current().abi().clone(),
        &CompilerOptions::default(),
        &HotReloadPolicy::default(),
    )
    .expect("compile update");

    assert_eq!(
        update.changed_function_names().collect::<Vec<_>>(),
        [
            "game::main.__impl.game::main::Player.bonus",
            "game::main::main"
        ]
    );
    let method = update
        .script_method("game::main::Player", "bonus")
        .expect("compiled inherent script method should be visible from update");
    assert_eq!(
        update
            .script_method_function_by_id("game::main::Player", method.id)
            .as_ref()
            .map(|function| function.name.as_str()),
        Some("game::main.__impl.game::main::Player.bonus")
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
        run_linked_version(&old, "main", &[]),
        Ok(OwnedValue::Int(20))
    );
    assert_eq!(
        run_linked_version(&new, "main", &[]),
        Ok(OwnedValue::Int(30))
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
                new: AccessAbi::new(true, false),
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
            new: AccessAbi::new(true, false),
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
                function: "game::reward::grant".to_owned(),
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
                new: AccessAbi::new(true, false),
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
                "method access: old=(public=true reflective=true callable=true) new=(public=true reflective=false callable=false)",
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
        run_linked_version(&runtime.current(), "main", &[]),
        Ok(OwnedValue::Int(1))
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
fn rejected_reload_report_fixture_renders_parameter_abi_span_and_hint() {
    let initial_source = normalized_fixture(HOT_RELOAD_PARAMETER_ABI_V1);
    let updated_source = normalized_fixture(HOT_RELOAD_PARAMETER_ABI_V2);
    let initial = compile_initial(SourceId::new(1), &initial_source)
        .expect("compile initial hot reload diagnostic fixture");
    let mut runtime = HotReloadRuntime::new(initial);

    let report = runtime.apply_hot_update_result_report(compile_update(
        &runtime.current(),
        SourceId::new(2),
        &updated_source,
    ));

    assert!(!report.accepted);
    let rendered = render_report_lines_for_fixture(&report);

    assert_eq!(
        rendered.trim_end(),
        normalized_fixture(HOT_RELOAD_PARAMETER_ABI_EXPECTED).trim_end()
    );
}

fn normalized_fixture(source: &str) -> String {
    source.replace("\r\n", "\n")
}

fn render_report_lines_for_fixture(report: &HotReloadReport) -> String {
    report
        .render_lines()
        .into_iter()
        .map(|line| {
            let span = line
                .span
                .map(|span| {
                    format!(
                        " @ source {}:{}..{}",
                        span.source.get(),
                        span.start,
                        span.end
                    )
                })
                .unwrap_or_default();
            format!("{:?}{span}: {}", line.kind, line.text)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn module_sources(reward: i64) -> Vec<ModuleSource> {
    vec![
        ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_qualified("game::main"),
            r#"
use game::reward::grant

fn main() {
    return grant() + 1;
}
"#,
        ),
        ModuleSource::new(
            SourceId::new(2),
            ModulePath::from_qualified("game::reward"),
            format!(
                r#"
pub fn grant() {{
    return {reward};
}}
"#
            ),
        ),
    ]
}

fn script_method_module_sources() -> Vec<ModuleSource> {
    script_method_module_sources_with_bonus("self.level + amount")
}

fn script_method_module_sources_with_bonus(bonus_expression: &str) -> Vec<ModuleSource> {
    vec![ModuleSource::new(
        SourceId::new(1),
        ModulePath::from_qualified("game::main"),
        format!(
            r#"
trait BonusSource {{ fn bonus(self, amount) -> int; }}
struct Player {{ level: int }}

impl BonusSource for Player {{
    fn bonus(self, amount) -> int {{
        return {bonus_expression};
    }}
}}

fn main() {{
    let player = Player {{ level: 7 }};
    return player.bonus(5);
}}
"#,
        ),
    )]
}

fn inherent_script_method_module_sources() -> Vec<ModuleSource> {
    inherent_script_method_module_sources_with_bonus("self.level + amount")
}

fn inherent_script_method_module_sources_with_bonus(bonus_expression: &str) -> Vec<ModuleSource> {
    vec![ModuleSource::new(
        SourceId::new(1),
        ModulePath::from_qualified("game::main"),
        format!(
            r#"
struct Player {{ level: int }}

impl Player {{
    fn bonus(self, amount) -> int {{
        return {bonus_expression};
    }}
}}

fn main() {{
    let player = Player {{ level: 7 }};
    return player.bonus(5);
}}
"#,
        ),
    )]
}
