use std::sync::Arc;

use vela_bytecode::compiler::compile_program_source_with_options;
use vela_engine::prelude::*;
use vela_host::mock::MockStateAdapter;
use vela_host::patch::PatchOp;

#[test]
fn prelude_imports_cover_runtime_embedding_flow() {
    let method = HostMethodId::new(23);
    let engine = Engine::builder()
        .register_type(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
                .host_type(HostTypeId::new(1))
                .method(MethodDesc::new(method, "grant_exp")),
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main(player: Player, amount: int) {
    player.grant_exp(amount);
    return amount;
}
"#,
        &engine.compiler_options(),
    )
    .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let player = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let args = args![host(player), 12];

    let result = runtime
        .call(
            "main",
            &args,
            CallOptions::gameplay(),
            &mut adapter,
            &mut tx,
        )
        .expect("runtime call should run");

    assert_eq!(result, Value::Int(12));
    assert_eq!(tx.patches()[0].path, HostPath::new(player),);
    assert_eq!(
        tx.patches()[0].op,
        PatchOp::CallHostMethod {
            method,
            args: vec![HostValue::Int(12)],
        },
    );
}

#[test]
fn prelude_imports_cover_source_and_reload_results() {
    let engine = Engine::builder().build().expect("engine should build");
    let compile_error: EngineSourceError = engine
        .compile_file("missing-prelude-source.vela")
        .expect_err("missing source should report an engine source error");

    assert!(matches!(
        compile_error.kind,
        EngineSourceErrorKind::Io { .. }
    ));

    let reload_result: EngineHotReloadSourceResult<ProgramVersion> =
        engine.compile_hot_reload_initial_file("missing-prelude-reload.vela");
    let reload_error = reload_result.expect_err("missing reload source should report source error");

    assert!(matches!(
        reload_error.kind,
        EngineHotReloadSourceErrorKind::Source(EngineSourceError {
            kind: EngineSourceErrorKind::Io { .. },
        })
    ));

    fn accepts_update_result(_result: EngineHotReloadSourceResult<HotUpdate>) {}
    fn accepts_safe_point_report(_report: Option<HotReloadReport>) {}
    fn accepts_event_safe_point_report(_report: EventCallSafePointReport) {}
    fn accepts_patch_safe_point_report(_report: PatchApplySafePointReport) {}
    fn accepts_hot_reload_result(_result: HotReloadResult<ProgramVersion>) {}
    fn accepts_report_diagnostics(_diagnostics: Vec<HotReloadDiagnostic>) {}
    fn accepts_report_detail(_detail: Option<HotReloadDiagnosticDetail>) {}
    fn accepts_report_lines(_lines: Vec<HotReloadReportLine>) {}
    fn accepts_report_line_kind(_kind: Option<HotReloadReportLineKind>) {}
    fn accepts_version_id(_version: Option<ProgramVersionId>) {}
    fn accepts_code_object(_code: Option<Arc<CodeObject>>) {}
    fn accepts_script_metadata(_metadata: Option<&ModuleGraph>) {}
    fn accepts_script_methods(_methods: &ScriptMethodTable) {}
    fn accepts_script_method(_method: Option<&ScriptMethod>) {}

    accepts_update_result(Err(reload_error));
    accepts_safe_point_report(None);
    accepts_event_safe_point_report(EventCallSafePointReport {
        value: Value::Null,
        reload: None,
    });
    accepts_patch_safe_point_report(PatchApplySafePointReport::default());
    accepts_hot_reload_result(engine.compile_hot_reload_initial(SourceId::new(2), "fn main() {}"));
    accepts_report_diagnostics(Vec::new());
    accepts_report_detail(None);
    accepts_report_lines(Vec::new());
    accepts_report_line_kind(None);
    accepts_version_id(None);

    let version = engine
        .compile_hot_reload_initial(
            SourceId::new(3),
            r#"
struct Player {
    score
}

trait Bonus {
    fn bonus(self)
}

impl Bonus for Player {
    fn bonus(self) {
        return self.score;
    }
}

fn main() {
    return Player { score: 7 }.bonus();
}
"#,
        )
        .expect("script metadata should compile");
    let code = version.function("main");
    let metadata = version.script_metadata();
    let method = version.script_method("Player", "bonus");

    assert!(code.is_some());
    assert!(metadata.is_some());
    assert!(method.is_some());
    assert!(version.script_method_function("Player", "bonus").is_some());

    accepts_code_object(code);
    accepts_script_metadata(metadata);
    accepts_script_methods(version.script_methods());
    accepts_script_method(method);
    accepts_code_object(version.script_method_function("Player", "bonus"));
}
