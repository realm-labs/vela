use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

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

    let root = unique_test_dir("prelude_compile_file_runtime_flow");
    fs::create_dir_all(root.path()).expect("create compile_file test dir");
    let source = root.path().join("main.vela");
    fs::write(
        &source,
        r#"
fn main(player: Player, amount: int) {
    player.grant_exp(amount);
    return amount;
}
"#,
    )
    .expect("write source file");
    let program = engine
        .compile_file(&source)
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
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx,
        )
        .expect("runtime call should run");

    assert_eq!(result, OwnedValue::Int(12));
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
fn prelude_imports_cover_script_arg_conversion_traits() {
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 7);
    let proxy = PathProxy::new(HostPath::new(host_ref).field(FieldId::new(9)));
    let args = args![host(host_ref), proxy.clone(), Some(3_i64), "tag"];

    assert_eq!(args.required::<HostRef>(0), Ok(host_ref));
    assert_eq!(args.required::<PathProxy>(1), Ok(proxy));
    assert_eq!(args.required::<Option<i64>>(2), Ok(Some(3)));
    assert_eq!(String::from_script_arg(&args[3]), Ok("tag".to_owned()));
    assert_eq!(
        "done".into_script_arg(),
        OwnedValue::String("done".to_owned())
    );
    assert_eq!((1_u32, 42_u64, 7_u32).into_host_ref(), host_ref);
}

#[test]
fn prelude_imports_cover_compile_dir_runtime_call_embedding_flow() {
    let root = unique_test_dir("prelude_compile_dir_runtime_call");
    let game_dir = root.path().join("game");
    fs::create_dir_all(&game_dir).expect("create game module dir");
    fs::write(
        game_dir.join("main.vela"),
        r#"
fn main(amount: int) {
    return game::grant_bonus(amount = amount, base = game::config::BASE);
}
"#,
    )
    .expect("write main module");
    fs::write(
        game_dir.join("config.vela"),
        r#"
pub const BASE: int = 10;
"#,
    )
    .expect("write config module");
    fs::write(root.path().join("ignored.txt"), "fn main() { return 99; }")
        .expect("write ignored non-source file");

    let engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::grant_bonus", NativeFunctionId::new(44))
                .param("base", TypeHint::Int)
                .param("amount", TypeHint::Int)
                .returns(TypeHint::Int)
                .effects(EffectSet::pure())
                .access(FunctionAccess::public()),
            #[allow(clippy::result_large_err)]
            |args| {
                let [OwnedValue::Int(base), OwnedValue::Int(amount)] = args else {
                    return Ok(OwnedValue::Null);
                };
                Ok(OwnedValue::Int(base + amount))
            },
        )
        .build()
        .expect("engine should build");
    let registry = engine.registry();
    let function = registry
        .function_by_name("game::grant_bonus")
        .expect("native function metadata should register");
    assert_eq!(function.id, NativeFunctionId::new(44));
    assert_eq!(function.params[0].name, "base");
    assert_eq!(function.params[0].type_hint.as_deref(), Some("int"));
    assert_eq!(function.params[1].name, "amount");
    assert_eq!(function.params[1].type_hint.as_deref(), Some("int"));
    assert_eq!(function.return_type.as_deref(), Some("int"));
    assert!(function.access.required_permissions().is_empty());

    let program = engine
        .compile_dir(root.path())
        .expect("directory modules should compile");
    assert!(program.function("ignored.main").is_none());
    let mut runtime = Runtime::new(engine, program);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call(
            "game::main::main",
            &args![5],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(15))
    );
    assert!(tx.patches().is_empty());
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
    fn accepts_call_output(_output: CallOutput) {}
    fn accepts_event_safe_point_report(_report: EventCallSafePointReport) {}
    fn accepts_hot_reload_result(_result: HotReloadResult<ProgramVersion>) {}
    fn accepts_report_diagnostics(_diagnostics: Vec<HotReloadDiagnostic>) {}
    fn accepts_report_detail(_detail: Option<HotReloadDiagnosticDetail>) {}
    fn accepts_report_lines(_lines: Vec<HotReloadReportLine>) {}
    fn accepts_report_line_kind(_kind: Option<HotReloadReportLineKind>) {}
    fn accepts_version_id(_version: Option<ProgramVersionId>) {}
    fn accepts_code_object(_code: Option<Arc<CodeObject>>) {}
    fn accepts_script_metadata(_metadata: Option<&ModuleGraph>) {}
    fn accepts_module_path(_path: ModulePath) {}
    fn accepts_module_id(_module: Option<ModuleId>) {}
    fn accepts_decl_id(_declaration: Option<HirDeclId>) {}
    fn accepts_declaration_index(_index: Option<&DeclarationIndex>) {}
    fn accepts_declaration(_declaration: Option<&Declaration>) {}
    fn accepts_declaration_kind(_kind: DeclarationKind) {}
    fn accepts_script_methods(_methods: &ScriptMethodTable) {}
    fn accepts_script_method(_method: Option<&ScriptMethod>) {}

    accepts_update_result(Err(reload_error));
    accepts_safe_point_report(None);
    accepts_call_output(CallOutput::new(OwnedValue::Null, PatchTx::new()));
    accepts_event_safe_point_report(EventCallSafePointReport {
        value: OwnedValue::Null,
        reload: None,
    });
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
    let module_path = ModulePath::from_qualified("");
    let module = metadata.and_then(|metadata| metadata.module_id(&module_path));
    let declaration_index =
        module.and_then(|module| metadata.and_then(|metadata| metadata.module(module)));
    let declaration_id = declaration_index.and_then(|index| index.get("Player"));
    let declaration =
        declaration_id.and_then(|id| metadata.and_then(|metadata| metadata.declaration(id)));
    let method = version.script_method("Player", "bonus");

    assert!(code.is_some());
    assert!(metadata.is_some());
    assert!(module.is_some());
    assert!(declaration_index.is_some());
    assert!(declaration_id.is_some());
    assert_eq!(
        declaration.map(|declaration| declaration.kind),
        Some(DeclarationKind::Struct)
    );
    assert!(method.is_some());
    assert!(version.script_method_function("Player", "bonus").is_some());

    accepts_code_object(code);
    accepts_script_metadata(metadata);
    accepts_module_path(module_path);
    accepts_module_id(module);
    accepts_declaration_index(declaration_index);
    accepts_decl_id(declaration_id);
    accepts_declaration(declaration);
    accepts_declaration_kind(DeclarationKind::Struct);
    accepts_script_methods(version.script_methods());
    accepts_script_method(method);
    accepts_code_object(version.script_method_function("Player", "bonus"));
}

struct TestDir(PathBuf);

impl TestDir {
    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn unique_test_dir(name: &str) -> TestDir {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "vela_engine_{name}_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time after epoch")
            .as_nanos()
    ));
    TestDir(path)
}
