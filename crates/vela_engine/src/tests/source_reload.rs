use vela_common::{FieldId, HostMethodId, HostObjectId, HostTypeId, SourceId, TypeId};
use vela_host::mock::MockStateAdapter;
use vela_host::patch::PatchOp;
use vela_host::path::{HostPath, HostRef};
use vela_host::tx::PatchTx;
use vela_host::value::HostValue;
use vela_hot_reload::error::HotReloadErrorKind;
use vela_hot_reload::policy::HotReloadPolicy;
use vela_hot_reload::runtime::HotReloadRuntime;
use vela_reflect::access::{MethodAccess, MethodEffectSet};
use vela_reflect::registry::{MethodDesc, SchemaHash, TypeDesc, TypeKey};
use vela_vm::HostExecution;
use vela_vm::value::Value;

use crate::engine::Engine;
use crate::error::EngineErrorKind;
use crate::native::{EffectSet, FunctionAccess, NativeFunctionDesc, NativeFunctionId, TypeHint};
use crate::reload::EngineHotReloadSourceErrorKind;
use crate::runtime::{CallOptions, Runtime};
use crate::source::EngineSourceErrorKind;

use super::player_type;

#[test]
fn engine_compile_file_uses_engine_compiler_options() {
    let root = unique_test_dir("compile_file");
    std::fs::create_dir_all(&root).expect("create temp source dir");
    let source = root.join("main.vela");
    std::fs::write(
        &source,
        r#"
fn main(player: Player) {
    player.level += 1;
    player.grant_exp(7);
    return player.level;
}
"#,
    )
    .expect("write source file");
    let method = HostMethodId::new(77);
    let engine = Engine::builder()
        .register_type(
            player_type(TypeId::new(1), HostTypeId::new(1))
                .method(MethodDesc::new(method, "grant_exp")),
        )
        .build()
        .expect("engine should build");

    let program = engine.compile_file(&source).expect("compile file");
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    adapter.insert_value(
        HostPath::new(host_ref).field(FieldId::new(1)),
        HostValue::Int(10),
    );
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine.into_vm().run_program_with_host(
            &program,
            "main",
            &[Value::HostRef(host_ref)],
            &mut host
        ),
        Ok(Value::Int(11))
    );
    assert_eq!(tx.patches()[0].op, PatchOp::Add(HostValue::Int(1)));
    assert_eq!(
        tx.patches()[1].op,
        PatchOp::CallHostMethod {
            method,
            args: vec![HostValue::Int(7)]
        }
    );
    std::fs::remove_dir_all(root).expect("clean temp source dir");
}

#[test]
fn engine_compile_dir_loads_vela_modules_deterministically() {
    let root = unique_test_dir("compile_dir");
    let game_dir = root.join("game");
    std::fs::create_dir_all(&game_dir).expect("create module dir");
    std::fs::write(
        game_dir.join("main.vela"),
        r#"
use game.reward.grant

fn main() {
    return grant() + game.config.BONUS;
}
"#,
    )
    .expect("write main module");
    std::fs::write(
        game_dir.join("reward.vela"),
        r#"
pub fn grant() {
    return 4;
}
"#,
    )
    .expect("write reward module");
    std::fs::write(
        game_dir.join("config.vela"),
        r#"
pub const BONUS: int = 6;
"#,
    )
    .expect("write config module");
    std::fs::write(root.join("ignored.txt"), "fn main() { return 99; }")
        .expect("write ignored file");
    let engine = Engine::builder().build().expect("engine should build");

    let program = engine.compile_dir(&root).expect("compile dir");

    assert_eq!(
        engine
            .into_vm()
            .run_program(&program, "game.main.main", &[]),
        Ok(Value::Int(10))
    );
    assert!(program.function("ignored.main").is_none());
    std::fs::remove_dir_all(root).expect("clean temp source dir");
}

#[test]
fn engine_compile_hot_reload_dir_loads_module_updates() {
    let root = unique_test_dir("hot_reload_dir");
    let game_dir = root.join("game");
    std::fs::create_dir_all(&game_dir).expect("create module dir");
    std::fs::write(
        game_dir.join("main.vela"),
        r#"
use game.reward.grant

fn main() {
    return grant() + 1;
}
"#,
    )
    .expect("write main module");
    std::fs::write(
        game_dir.join("reward.vela"),
        r#"
pub fn grant() {
    return 4;
}
"#,
    )
    .expect("write reward module");
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call(
            "game.main.main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(5))
    );

    std::fs::write(
        game_dir.join("reward.vela"),
        r#"
pub fn grant() {
    return 7;
}
"#,
    )
    .expect("write updated reward module");
    let current = runtime
        .hot_reload_version()
        .expect("current hot reload version");
    let update = runtime
        .engine()
        .compile_hot_reload_update_dir(&current, &root)
        .expect("compatible hot reload dir update");
    let report = runtime.apply_hot_update(update).expect("apply update");

    assert!(report.accepted);
    assert_eq!(
        report.changed_functions,
        vec!["game.main.main".to_owned(), "game.reward.grant".to_owned()]
    );
    assert_eq!(
        runtime.call(
            "game.main.main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(8))
    );
    std::fs::remove_dir_all(root).expect("clean temp source dir");
}

#[test]
fn engine_compile_hot_reload_file_reports_source_errors() {
    let root = unique_test_dir("missing_hot_reload_file");
    let path = root.join("missing.vela");
    let engine = Engine::builder().build().expect("engine should build");

    let error = engine
        .compile_hot_reload_initial_file(&path)
        .expect_err("missing hot reload source file should fail");

    assert!(matches!(
        error.kind,
        EngineHotReloadSourceErrorKind::Source(_)
    ));
}

#[test]
fn engine_compile_file_reports_io_errors() {
    let root = unique_test_dir("missing_file");
    let path = root.join("missing.vela");
    let engine = Engine::builder().build().expect("engine should build");

    let error = engine
        .compile_file(&path)
        .expect_err("missing source file should fail");

    assert!(matches!(error.kind, EngineSourceErrorKind::Io { .. }));
}

#[test]
fn engine_exposes_registry_hot_reload_abi() {
    let player_key = TypeKey::new(TypeId::new(1), "Player");
    let method = HostMethodId::new(9);
    let engine = Engine::builder()
        .register_type(
            TypeDesc::new(player_key.clone())
                .schema_hash(SchemaHash::new(0xfeed))
                .host_type(HostTypeId::new(1))
                .method(
                    MethodDesc::new(method, "grant_exp")
                        .effects(MethodEffectSet::host_write())
                        .access(
                            MethodAccess::new()
                                .reflect_callable(true)
                                .require_permission("player.write"),
                        ),
                ),
        )
        .register_native_fn(
            NativeFunctionDesc::new("game.reward.grant", NativeFunctionId::new(22))
                .param("player", TypeHint::Host(player_key))
                .returns(TypeHint::Null)
                .effects(EffectSet::event_emit())
                .access(
                    FunctionAccess::public()
                        .reflect_callable(true)
                        .require_permission("reward.grant"),
                ),
            |_| Ok(Value::Null),
        )
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial(
            SourceId::new(1),
            r#"
fn main(player: Player) {
    player.grant_exp(10);
    return 1;
}
"#,
        )
        .expect("initial hot reload compile");
    let update = engine
        .compile_hot_reload_update(
            &initial,
            SourceId::new(2),
            r#"
fn main(player: Player) {
    player.grant_exp(11);
    return 2;
}
"#,
        )
        .expect("unchanged engine ABI should be hot-reload compatible");
    let mut runtime = HotReloadRuntime::new(initial);
    let version = runtime.apply_hot_update(update).expect("apply update");
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine.into_vm().run_program_with_host(
            &version.to_program(),
            "main",
            &[Value::HostRef(host_ref)],
            &mut host
        ),
        Ok(Value::Int(2))
    );
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(
        tx.patches()[0].op,
        PatchOp::CallHostMethod {
            method,
            args: vec![HostValue::Int(11)]
        }
    );
}

#[test]
fn runtime_applies_engine_hot_reload_updates() {
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial(SourceId::new(1), "fn main() { return 1; }")
        .expect("initial hot reload compile");
    let update = engine
        .compile_hot_reload_update(&initial, SourceId::new(2), "fn main() { return 2; }")
        .expect("compatible update should compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(1))
    );

    let report = runtime.apply_hot_update(update).expect("apply update");
    assert!(report.accepted);
    assert_eq!(report.changed_functions, vec!["main".to_owned()]);
    assert_eq!(
        runtime
            .hot_reload_version()
            .expect("current hot reload version")
            .id,
        report.to_version.expect("accepted version id")
    );
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(2))
    );
}

#[test]
fn runtime_compiles_hot_reload_update_from_active_version() {
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial(SourceId::new(1), "fn main() { return 1; }")
        .expect("initial hot reload compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    let first_update = runtime
        .compile_hot_reload_update(
            SourceId::new(2),
            r#"
fn helper() {
    return 2;
}

fn main() {
    return helper();
}
"#,
        )
        .expect("runtime should be hot-reload enabled")
        .expect("compatible update should compile");
    let first_report = runtime
        .apply_hot_update(first_update)
        .expect("runtime should apply first update");
    assert!(first_report.accepted);
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(2))
    );

    let rejected_update = runtime
        .compile_hot_reload_update(SourceId::new(3), "fn main() { return 3; }")
        .expect("runtime should be hot-reload enabled");
    let error = rejected_update.expect_err("active helper removal should be rejected");
    assert!(matches!(
        error.kind,
        HotReloadErrorKind::RemovedFunction { ref function } if function == "helper"
    ));
}

#[test]
fn runtime_compiles_hot_reload_update_file_from_active_version() {
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial(SourceId::new(1), "fn main() { return 1; }")
        .expect("initial hot reload compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock should be after epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "vela-runtime-hot-reload-{pid}-{unique}.vela",
        pid = std::process::id()
    ));
    std::fs::write(&path, "fn main() { return 5; }").expect("update file should write");

    let update = runtime
        .compile_hot_reload_update_file(&path)
        .expect("runtime should be hot-reload enabled")
        .expect("file update should compile");
    std::fs::remove_file(&path).expect("update file should clean up");
    let report = runtime
        .apply_hot_update(update)
        .expect("runtime should apply file update");
    assert!(report.accepted);

    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(5))
    );
}

#[test]
fn runtime_preserves_program_when_engine_hot_reload_update_is_rejected() {
    let engine = Engine::builder()
        .hot_reload_policy(HotReloadPolicy::locked_down())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial(SourceId::new(1), "fn main() { return 1; }")
        .expect("initial hot reload compile");
    let update = engine.compile_hot_reload_update(
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
    );
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    let report = runtime
        .apply_hot_update_result_report(update)
        .expect("runtime should return rejection report");
    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(1))
    );
}

#[test]
fn runtime_rejects_hot_update_when_not_created_from_version() {
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial(SourceId::new(1), "fn main() { return 1; }")
        .expect("initial hot reload compile");
    let update = engine
        .compile_hot_reload_update(&initial, SourceId::new(2), "fn main() { return 2; }")
        .expect("compatible update should compile");
    let mut runtime = Runtime::new(engine, initial.to_program());

    assert!(matches!(
        runtime.apply_hot_update(update),
        Err(error) if error.kind == EngineErrorKind::RuntimeNotHotReloadEnabled
    ));
}

#[test]
fn runtime_rejects_compile_update_when_not_created_from_version() {
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial(SourceId::new(1), "fn main() { return 1; }")
        .expect("initial hot reload compile");
    let runtime = Runtime::new(engine, initial.to_program());

    assert!(matches!(
        runtime.compile_hot_reload_update(SourceId::new(2), "fn main() { return 2; }"),
        Err(error) if error.kind == EngineErrorKind::RuntimeNotHotReloadEnabled
    ));
}

#[test]
fn engine_applies_configured_hot_reload_policy() {
    let engine = Engine::builder()
        .hot_reload_policy(HotReloadPolicy::locked_down())
        .build()
        .expect("engine should build");
    assert_eq!(engine.hot_reload_policy(), &HotReloadPolicy::locked_down());
    let initial = engine
        .compile_hot_reload_initial(SourceId::new(1), "fn main() { return 1; }")
        .expect("initial hot reload compile");

    let error = engine
        .compile_hot_reload_update(
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
        )
        .expect_err("locked-down policy should reject new helper functions");

    assert_eq!(
        error.kind,
        HotReloadErrorKind::NewFunctionDenied {
            function: "helper".to_owned(),
        }
    );
}

fn unique_test_dir(name: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "vela_engine_{name}_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time after epoch")
            .as_nanos()
    ));
    path
}
