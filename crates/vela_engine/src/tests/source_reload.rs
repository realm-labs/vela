use vela_common::{FieldId, HostMethodId, HostObjectId, HostTypeId, SourceId, TypeId};
use vela_host::error::HostErrorKind;
use vela_host::mock::MockStateAdapter;
use vela_host::patch::PatchOp;
use vela_host::path::{HostPath, HostRef};
use vela_host::tx::PatchTx;
use vela_host::value::HostValue;
use vela_hot_reload::error::HotReloadErrorKind;
use vela_hot_reload::policy::HotReloadPolicy;
use vela_hot_reload::runtime::HotReloadRuntime;
use vela_reflect::access::{MethodAccess, MethodEffectSet};
use vela_reflect::registry::{MethodDesc, MethodParamDesc, SchemaHash, TypeDesc, TypeKey};
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
}

#[test]
fn engine_compile_dir_loads_vela_modules_deterministically() {
    let root = unique_test_dir("compile_dir");
    let game_dir = root.join("game");
    std::fs::create_dir_all(&game_dir).expect("create module dir");
    std::fs::write(
        game_dir.join("main.vela"),
        r#"
use game::reward::grant

fn main() {
    return grant() + game::config::BONUS;
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
            .run_program(&program, "game::main::main", &[]),
        Ok(Value::Int(10))
    );
    assert!(program.function("ignored.main").is_none());
}

#[test]
fn engine_compile_hot_reload_dir_loads_module_updates() {
    let root = unique_test_dir("hot_reload_dir");
    let game_dir = root.join("game");
    std::fs::create_dir_all(&game_dir).expect("create module dir");
    std::fs::write(
        game_dir.join("main.vela"),
        r#"
use game::reward::grant

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
            "game::main::main",
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
        vec!["game::reward::grant".to_owned()]
    );
    assert_eq!(report.changed_modules, vec!["game::reward".to_owned()]);
    assert_eq!(
        report.impacted_modules,
        vec!["game::main".to_owned(), "game::reward".to_owned()]
    );
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(8))
    );
}

#[test]
fn runtime_stages_hot_reload_dir_until_check_reload_safe_point() {
    let root = unique_test_dir("runtime_stage_hot_reload_dir");
    let reward_file = write_reward_modules(&root, "return grant();", 2);
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    write_reward_module(&reward_file, 6);
    runtime
        .stage_hot_reload_update_dir(&root)
        .expect("runtime should be hot-reload enabled")
        .expect("dir update should stage");
    assert!(
        runtime
            .has_pending_hot_update()
            .expect("dir update should be pending")
    );
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged dir report");

    assert!(report.accepted);
    assert_eq!(report.changed_functions, vec!["game::reward::grant"]);
    assert_eq!(report.changed_modules, vec!["game::reward"]);
    assert!(
        !runtime
            .has_pending_hot_update()
            .expect("safe point should consume dir update")
    );
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(6))
    );
}

#[test]
fn runtime_stages_dir_hot_reload_rejection_until_safe_point() {
    let root = unique_test_dir("runtime_stage_dir_rejection");
    let reward_file = write_reward_modules(&root, "return grant();", 2);
    let engine = Engine::builder()
        .hot_reload_policy(HotReloadPolicy::locked_down())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    write_reward_module_with_helper(&reward_file, 6);
    runtime
        .stage_hot_reload_update_dir(&root)
        .expect("runtime should be hot-reload enabled")
        .expect("hot reload rejection should be staged");
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert!(matches!(
        report.errors[0].error.kind,
        HotReloadErrorKind::NewFunctionDenied { ref function }
            if function == "game::reward::helper"
    ));
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );
}

#[test]
fn runtime_stages_dir_return_abi_rejection_until_safe_point() {
    let root = unique_test_dir("runtime_stage_dir_return_abi");
    let reward_file = write_typed_reward_modules(&root, "return grant();", "int", "2");
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    write_typed_reward_module(&reward_file, "float", "6.0");
    runtime
        .stage_hot_reload_update_dir(&root)
        .expect("runtime should be hot-reload enabled")
        .expect("dir return ABI rejection should be staged");
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged dir return ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.function.return_abi_changed");
    let HotReloadErrorKind::ChangedFunctionReturnAbi {
        function,
        old,
        new,
        source_span,
    } = &report.errors[0].error.kind
    else {
        panic!("expected changed function return ABI");
    };
    assert_eq!(function, "game::reward::grant");
    assert_eq!(old.as_deref(), Some("int"));
    assert_eq!(new.as_deref(), Some("float"));
    assert!(source_span.is_some());
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );
}

#[test]
fn runtime_stages_dir_required_parameter_rejection_until_safe_point() {
    let root = unique_test_dir("runtime_stage_dir_required_parameter");
    let reward_file = write_typed_reward_modules(&root, "return 2;", "int", "2");
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    write_reward_module_with_signature(&reward_file, "(amount: int) -> int", "amount");
    runtime
        .stage_hot_reload_update_dir(&root)
        .expect("runtime should be hot-reload enabled")
        .expect("dir required parameter rejection should be staged");
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged dir required parameter rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(
        report.errors[0].code,
        "reload.function.required_added_parameters"
    );
    let HotReloadErrorKind::AddedFunctionParametersWithoutDefaults { function, added } =
        &report.errors[0].error.kind
    else {
        panic!("expected added required parameters");
    };
    assert_eq!(function, "game::reward::grant");
    assert_eq!(added, &vec!["amount".to_owned()]);
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );
}

#[test]
fn runtime_stages_dir_script_function_access_rejection_until_safe_point() {
    let root = unique_test_dir("runtime_stage_dir_script_function_access");
    let reward_file = write_reward_modules(&root, "return grant();", 2);
    let main_file = root.join("game").join("main.vela");
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    std::fs::write(
        &main_file,
        r#"
fn main() {
    return 3;
}
"#,
    )
    .expect("write main without reward import");
    std::fs::write(
        &reward_file,
        r#"
fn grant() {
    return 6;
}
"#,
    )
    .expect("write reward without public export");
    runtime
        .stage_hot_reload_update_dir(&root)
        .expect("runtime should be hot-reload enabled")
        .expect("dir script function access ABI rejection should be staged");
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged dir script function access ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.function.access_changed");
    assert_eq!(
        report.errors[0].repair_hint.as_deref(),
        Some("preserve reflective access metadata or require host approval before reloading")
    );
    let HotReloadErrorKind::ChangedFunctionAccess {
        function,
        old,
        new,
        source_span,
    } = &report.errors[0].error.kind
    else {
        panic!("expected changed function access ABI");
    };
    assert_eq!(function, "game::reward::grant");
    assert!(old.public);
    assert!(!new.public);
    assert_eq!(old.required_permissions, new.required_permissions);
    assert!(source_span.is_some());
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );
}

#[test]
fn runtime_stages_dir_native_effect_rejection_until_safe_point() {
    let kind = dir_native_rejection_kind(
        "runtime_stage_dir_native_effect",
        NativeFunctionDesc::new("game::native::grant_bonus", NativeFunctionId::new(22))
            .effects(EffectSet::host_read()),
        NativeFunctionDesc::new("game::native::grant_bonus", NativeFunctionId::new(22))
            .effects(EffectSet::host_write()),
        "reload.function.effects_changed",
    );

    let HotReloadErrorKind::ChangedFunctionEffects {
        function,
        old,
        new,
        source_span,
    } = kind
    else {
        panic!("expected changed native function effects");
    };
    assert_eq!(function, "game::native::grant_bonus");
    assert!(old.reads_host);
    assert!(!old.writes_host);
    assert!(new.reads_host);
    assert!(new.writes_host);
    assert!(source_span.is_none());
}

#[test]
fn runtime_stages_dir_native_access_rejection_until_safe_point() {
    let kind = dir_native_rejection_kind(
        "runtime_stage_dir_native_access",
        NativeFunctionDesc::new("game::native::grant_bonus", NativeFunctionId::new(22)).access(
            FunctionAccess::public()
                .reflect_callable(true)
                .require_permission("reward.read"),
        ),
        NativeFunctionDesc::new("game::native::grant_bonus", NativeFunctionId::new(22)).access(
            FunctionAccess::public()
                .reflect_callable(true)
                .require_permission("reward.write"),
        ),
        "reload.function.access_changed",
    );

    let HotReloadErrorKind::ChangedFunctionAccess {
        function,
        old,
        new,
        source_span,
    } = kind
    else {
        panic!("expected changed native function access");
    };
    assert_eq!(function, "game::native::grant_bonus");
    assert_eq!(old.required_permissions, vec!["reward.read"]);
    assert_eq!(new.required_permissions, vec!["reward.write"]);
    assert!(old.callable);
    assert!(new.callable);
    assert!(source_span.is_none());
}

#[test]
fn runtime_stages_dir_native_parameter_rejection_until_safe_point() {
    let kind = dir_native_rejection_kind(
        "runtime_stage_dir_native_parameter",
        NativeFunctionDesc::new("game::native::grant_bonus", NativeFunctionId::new(22))
            .param("amount", TypeHint::Int),
        NativeFunctionDesc::new("game::native::grant_bonus", NativeFunctionId::new(22))
            .param("amount", TypeHint::Float),
        "reload.function.parameter_abi_changed",
    );

    let HotReloadErrorKind::ChangedFunctionParameterAbi {
        function,
        old,
        new,
        source_span,
    } = kind
    else {
        panic!("expected changed native function parameter ABI");
    };
    assert_eq!(function, "game::native::grant_bonus");
    assert_eq!(old.len(), 1);
    assert_eq!(old[0].name, "amount");
    assert_eq!(old[0].type_hint.as_deref(), Some("int"));
    assert_eq!(new.len(), 1);
    assert_eq!(new[0].name, "amount");
    assert_eq!(new[0].type_hint.as_deref(), Some("float"));
    assert!(source_span.is_none());
}

#[test]
fn runtime_stages_dir_native_return_rejection_until_safe_point() {
    let kind = dir_native_rejection_kind(
        "runtime_stage_dir_native_return",
        NativeFunctionDesc::new("game::native::grant_bonus", NativeFunctionId::new(22))
            .returns(TypeHint::Int),
        NativeFunctionDesc::new("game::native::grant_bonus", NativeFunctionId::new(22))
            .returns(TypeHint::Float),
        "reload.function.return_abi_changed",
    );

    let HotReloadErrorKind::ChangedFunctionReturnAbi {
        function,
        old,
        new,
        source_span,
    } = kind
    else {
        panic!("expected changed native function return ABI");
    };
    assert_eq!(function, "game::native::grant_bonus");
    assert_eq!(old.as_deref(), Some("int"));
    assert_eq!(new.as_deref(), Some("float"));
    assert!(source_span.is_none());
}

#[test]
fn runtime_stages_dir_method_effect_rejection_until_safe_point() {
    let kind = dir_method_rejection_kind(
        "runtime_stage_dir_method_effect",
        MethodDesc::new(HostMethodId::new(9), "grant_exp").effects(MethodEffectSet::host_read()),
        MethodDesc::new(HostMethodId::new(9), "grant_exp").effects(MethodEffectSet::host_write()),
        "reload.method.effects_changed",
    );

    let HotReloadErrorKind::ChangedMethodEffects {
        type_name,
        method,
        old,
        new,
        source_span,
    } = kind
    else {
        panic!("expected changed host method effects");
    };
    assert_eq!(type_name, "Player");
    assert_eq!(method, "grant_exp");
    assert!(old.reads_host);
    assert!(!old.writes_host);
    assert!(new.reads_host);
    assert!(new.writes_host);
    assert!(source_span.is_none());
}

#[test]
fn runtime_stages_dir_method_access_rejection_until_safe_point() {
    let kind = dir_method_rejection_kind(
        "runtime_stage_dir_method_access",
        MethodDesc::new(HostMethodId::new(9), "grant_exp").access(
            MethodAccess::new()
                .reflect_callable(true)
                .require_permission("player.read"),
        ),
        MethodDesc::new(HostMethodId::new(9), "grant_exp").access(
            MethodAccess::new()
                .reflect_callable(false)
                .require_permission("player.read"),
        ),
        "reload.method.access_changed",
    );

    let HotReloadErrorKind::ChangedMethodAccess {
        type_name,
        method,
        old,
        new,
        source_span,
    } = kind
    else {
        panic!("expected changed host method access");
    };
    assert_eq!(type_name, "Player");
    assert_eq!(method, "grant_exp");
    assert_eq!(old.required_permissions, vec!["player.read"]);
    assert_eq!(new.required_permissions, vec!["player.read"]);
    assert!(old.callable);
    assert!(!new.callable);
    assert!(source_span.is_none());
}

#[test]
fn runtime_stages_dir_method_parameter_rejection_until_safe_point() {
    let kind = dir_method_rejection_kind(
        "runtime_stage_dir_method_parameter",
        MethodDesc::new(HostMethodId::new(9), "grant_exp")
            .param(MethodParamDesc::new("amount").type_hint("int")),
        MethodDesc::new(HostMethodId::new(9), "grant_exp")
            .param(MethodParamDesc::new("amount").type_hint("float")),
        "reload.method.parameter_abi_changed",
    );

    let HotReloadErrorKind::ChangedMethodParameterAbi {
        type_name,
        method,
        old,
        new,
        source_span,
    } = kind
    else {
        panic!("expected changed host method parameter ABI");
    };
    assert_eq!(type_name, "Player");
    assert_eq!(method, "grant_exp");
    assert_eq!(old.len(), 1);
    assert_eq!(old[0].name, "amount");
    assert_eq!(old[0].type_hint.as_deref(), Some("int"));
    assert_eq!(new.len(), 1);
    assert_eq!(new[0].name, "amount");
    assert_eq!(new[0].type_hint.as_deref(), Some("float"));
    assert!(source_span.is_none());
}

#[test]
fn runtime_stages_dir_method_return_rejection_until_safe_point() {
    let kind = dir_method_rejection_kind(
        "runtime_stage_dir_method_return",
        MethodDesc::new(HostMethodId::new(9), "grant_exp").return_type("int"),
        MethodDesc::new(HostMethodId::new(9), "grant_exp").return_type("null"),
        "reload.method.return_abi_changed",
    );

    let HotReloadErrorKind::ChangedMethodReturnAbi {
        type_name,
        method,
        old,
        new,
        source_span,
    } = kind
    else {
        panic!("expected changed host method return ABI");
    };
    assert_eq!(type_name, "Player");
    assert_eq!(method, "grant_exp");
    assert_eq!(old.as_deref(), Some("int"));
    assert_eq!(new.as_deref(), Some("null"));
    assert!(source_span.is_none());
}

#[test]
fn runtime_stages_dir_defaulted_schema_addition_until_safe_point() {
    let root = unique_test_dir("runtime_stage_dir_defaulted_schema_addition");
    let reward_file = write_schema_reward_modules(&root, 2, StructCountField::Absent);
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    write_schema_reward_module(&reward_file, 6, StructCountField::Defaulted);
    runtime
        .stage_hot_reload_update_dir(&root)
        .expect("runtime should be hot-reload enabled")
        .expect("dir defaulted schema addition should be staged");
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged dir schema addition report");

    assert!(report.accepted);
    assert_eq!(report.changed_functions, vec!["game::reward::grant"]);
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(6))
    );
}

#[test]
fn runtime_stages_dir_required_schema_field_rejection_until_safe_point() {
    let root = unique_test_dir("runtime_stage_dir_required_schema_field_rejection");
    let reward_file = write_schema_reward_modules(&root, 2, StructCountField::Absent);
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    write_schema_reward_module(&reward_file, 6, StructCountField::Required);
    runtime
        .stage_hot_reload_update_dir(&root)
        .expect("runtime should be hot-reload enabled")
        .expect("dir schema field rejection should be staged");
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged dir schema field rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.schema.abi_changed");
    assert_eq!(
        report.errors[0].target.as_deref(),
        Some("game::reward::Reward")
    );
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );
}

#[test]
fn runtime_stages_dir_schema_field_type_rejection_until_safe_point() {
    let root = unique_test_dir("runtime_stage_dir_schema_field_type_rejection");
    let reward_file = write_schema_reward_modules(&root, 2, StructCountField::Required);
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    write_schema_reward_module(&reward_file, 6, StructCountField::Float);
    runtime
        .stage_hot_reload_update_dir(&root)
        .expect("runtime should be hot-reload enabled")
        .expect("dir schema field type rejection should be staged");
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged dir schema field type rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.schema.abi_changed");
    assert_eq!(
        report.errors[0].target.as_deref(),
        Some("game::reward::Reward")
    );
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );
}

#[test]
fn runtime_stages_dir_defaulted_enum_variant_field_addition_until_safe_point() {
    let root = unique_test_dir("runtime_stage_dir_defaulted_enum_variant_field_addition");
    let reward_file = write_enum_reward_modules(&root, 2, EnumVariantCountField::Absent);
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    write_enum_reward_module(&reward_file, 6, EnumVariantCountField::Defaulted);
    runtime
        .stage_hot_reload_update_dir(&root)
        .expect("runtime should be hot-reload enabled")
        .expect("dir defaulted enum variant field addition should be staged");
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged dir enum variant field addition report");

    assert!(report.accepted);
    assert_eq!(report.changed_functions, vec!["game::reward::grant"]);
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(6))
    );
}

#[test]
fn runtime_stages_dir_required_enum_variant_field_rejection_until_safe_point() {
    let root = unique_test_dir("runtime_stage_dir_required_enum_variant_field_rejection");
    let reward_file = write_enum_reward_modules(&root, 2, EnumVariantCountField::Absent);
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    write_enum_reward_module(&reward_file, 6, EnumVariantCountField::Required);
    runtime
        .stage_hot_reload_update_dir(&root)
        .expect("runtime should be hot-reload enabled")
        .expect("dir enum variant field rejection should be staged");
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged dir enum variant field rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.schema.abi_changed");
    assert_eq!(
        report.errors[0].target.as_deref(),
        Some("game::reward::QuestProgress")
    );
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );
}

#[test]
fn runtime_stages_dir_compile_rejection_until_safe_point() {
    let root = unique_test_dir("runtime_stage_dir_compile_rejection");
    let reward_file = write_reward_modules(&root, "return grant();", 2);
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    std::fs::write(
        &reward_file,
        r#"
const BAD = register_event("monster.kill");

pub fn grant() {
    return 6;
}
"#,
    )
    .expect("write side-effecting module update");
    runtime
        .stage_hot_reload_update_dir(&root)
        .expect("runtime should be hot-reload enabled")
        .expect("compile rejection should be staged as a hot reload report");
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged dir compile rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.compile");
    assert!(
        report.errors[0]
            .source_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.as_deref() == Some("hir::top_level_side_effect"))
    );
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );
}

#[test]
fn runtime_returns_hot_reload_dir_source_errors_immediately() {
    let root = unique_test_dir("runtime_stage_dir_source_error");
    let _reward_file = write_reward_modules(&root, "return grant();", 2);
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let missing = root.join("missing_dir");

    let error = runtime
        .stage_hot_reload_update_dir(&missing)
        .expect("runtime should be hot-reload enabled")
        .expect_err("missing source root should not stage a hot reload report");

    assert!(matches!(
        error.kind,
        EngineHotReloadSourceErrorKind::Source(crate::source::EngineSourceError {
            kind: EngineSourceErrorKind::Io { .. }
        })
    ));
    assert!(
        !runtime
            .has_pending_hot_update()
            .expect("source error should not stage an update")
    );
}

#[test]
fn engine_compile_hot_reload_changed_file_reloads_module_root() {
    let root = unique_test_dir("hot_reload_changed_file");
    let reward_file = write_reward_modules(&root, "return grant() + 1;", 4);
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");

    write_reward_module(&reward_file, 9);
    let update = engine
        .compile_hot_reload_update_changed_file(&initial, &root, &reward_file)
        .expect("changed file update should compile");
    let mut runtime = HotReloadRuntime::new(initial);
    let report = runtime.apply_hot_update_report(update);

    assert!(report.accepted);
    assert_eq!(report.changed_functions, vec!["game::reward::grant"]);
    assert_eq!(report.changed_modules, vec!["game::reward"]);
    assert_eq!(
        report.impacted_modules,
        vec!["game::main".to_owned(), "game::reward".to_owned()]
    );
    assert_eq!(
        engine
            .into_vm()
            .run_program(&runtime.current().to_program(), "game::main::main", &[]),
        Ok(Value::Int(10))
    );
}

#[test]
fn engine_compile_hot_reload_changed_file_accepts_normalized_root_paths() {
    let root = unique_test_dir("hot_reload_changed_file_normalized_root");
    let reward_file = write_reward_modules(&root, "return grant();", 4);
    let root_with_current_segment = root.join(".");
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");

    write_reward_module(&reward_file, 8);
    let update = engine
        .compile_hot_reload_update_changed_file(&initial, &root_with_current_segment, &reward_file)
        .expect("changed file update should compile");
    let mut runtime = HotReloadRuntime::new(initial);
    let report = runtime.apply_hot_update_report(update);

    assert!(report.accepted);
    assert_eq!(report.changed_functions, vec!["game::reward::grant"]);
    assert_eq!(
        engine
            .into_vm()
            .run_program(&runtime.current().to_program(), "game::main::main", &[]),
        Ok(Value::Int(8))
    );
}

#[test]
fn engine_compile_hot_reload_changed_file_rejects_non_source_path() {
    let root = unique_test_dir("hot_reload_changed_file_invalid");
    let reward_file = write_reward_modules(&root, "return grant();", 4);
    let changed = root.join("ignored.txt");
    std::fs::write(&changed, "ignored").expect("write ignored file");
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");

    let error = engine
        .compile_hot_reload_update_changed_file(&initial, &root, &changed)
        .expect_err("non-source watcher path should be rejected");

    assert!(matches!(
        error.kind,
        EngineHotReloadSourceErrorKind::Source(crate::source::EngineSourceError {
            kind: EngineSourceErrorKind::InvalidSourcePath { .. }
        })
    ));
    assert!(reward_file.exists());
}

#[test]
fn engine_compile_hot_reload_changed_file_rejects_parent_dir_escape() {
    let root = unique_test_dir("hot_reload_changed_file_parent_escape");
    let reward_file = write_reward_modules(&root, "return grant();", 4);
    let changed = root.join("..").join("outside.vela");
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");

    let error = engine
        .compile_hot_reload_update_changed_file(&initial, &root, &changed)
        .expect_err("changed source path escaping the root should be rejected");

    assert!(matches!(
        error.kind,
        EngineHotReloadSourceErrorKind::Source(crate::source::EngineSourceError {
            kind: EngineSourceErrorKind::InvalidSourcePath { .. }
        })
    ));
    assert!(reward_file.exists());
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
            NativeFunctionDesc::new("game::reward::grant", NativeFunctionId::new(22))
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
fn runtime_stages_engine_hot_reload_until_check_reload_safe_point() {
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

    runtime
        .stage_hot_update(update)
        .expect("stage pending update");
    assert!(
        runtime
            .has_pending_hot_update()
            .expect("hot reload runtime should report pending update")
    );
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(1))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("pending report");

    assert!(report.accepted);
    assert_eq!(report.changed_functions, vec!["main".to_owned()]);
    assert!(
        !runtime
            .has_pending_hot_update()
            .expect("pending update should be consumed")
    );
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(2))
    );
}

#[test]
fn runtime_tick_boundary_safe_point_consumes_staged_reload() {
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

    runtime
        .stage_hot_update(update)
        .expect("stage pending update");
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(1))
    );

    let report = runtime
        .check_reload_at_tick_boundary()
        .expect("check reload at tick boundary")
        .expect("pending report");

    assert!(report.accepted);
    assert_eq!(report.changed_functions, vec!["main".to_owned()]);
    assert!(
        !runtime
            .has_pending_hot_update()
            .expect("pending update should be consumed")
    );
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(2))
    );
    assert_eq!(
        runtime
            .check_reload_at_tick_boundary()
            .expect("check empty tick boundary"),
        None
    );
}

#[test]
fn runtime_tick_boundary_safe_point_reports_staged_reload_rejection() {
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial(SourceId::new(1), "pub fn main() -> int { return 1; }")
        .expect("initial hot reload compile");
    let update = engine
        .compile_hot_reload_update(
            &initial,
            SourceId::new(2),
            "pub fn main() -> float { return 2.0; }",
        )
        .expect_err("return hint change should be rejected");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    runtime
        .stage_hot_update_result(Err(update))
        .expect("stage rejected update");
    let report = runtime
        .check_reload_at_tick_boundary()
        .expect("check reload at tick boundary")
        .expect("pending report");

    assert!(!report.accepted);
    let HotReloadErrorKind::ChangedFunctionReturnAbi {
        function,
        old,
        new,
        source_span,
    } = &report.errors[0].error.kind
    else {
        panic!("expected changed function return ABI");
    };
    assert_eq!(function, "main");
    assert_eq!(old.as_deref(), Some("int"));
    assert_eq!(new.as_deref(), Some("float"));
    assert!(source_span.is_some());
    assert!(
        !runtime
            .has_pending_hot_update()
            .expect("pending rejection should be consumed")
    );
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(1))
    );
}

#[test]
fn runtime_call_at_event_end_safe_point_consumes_staged_reload_after_call() {
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

    runtime
        .stage_hot_update(update)
        .expect("stage pending update");
    let report = runtime
        .call_at_event_end_safe_point("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx)
        .expect("event call should run");

    assert_eq!(report.value, Value::Int(1));
    let reload = report.reload.expect("staged reload should be consumed");
    assert!(reload.accepted);
    assert_eq!(reload.changed_functions, vec!["main".to_owned()]);
    assert!(
        !runtime
            .has_pending_hot_update()
            .expect("pending update should be consumed")
    );
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(2))
    );
}

#[test]
fn runtime_call_at_event_end_safe_point_reports_staged_reload_rejection() {
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial(SourceId::new(1), "pub fn main() -> int { return 1; }")
        .expect("initial hot reload compile");
    let update = engine
        .compile_hot_reload_update(
            &initial,
            SourceId::new(2),
            "pub fn main() -> float { return 2.0; }",
        )
        .expect_err("return hint change should be rejected");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    runtime
        .stage_hot_update_result(Err(update))
        .expect("stage rejected update");
    let report = runtime
        .call_at_event_end_safe_point("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx)
        .expect("event call should run before reporting reload rejection");

    assert_eq!(report.value, Value::Int(1));
    let reload = report.reload.expect("staged rejection should be consumed");
    assert!(!reload.accepted);
    let HotReloadErrorKind::ChangedFunctionReturnAbi {
        function,
        old,
        new,
        source_span,
    } = &reload.errors[0].error.kind
    else {
        panic!("expected changed function return ABI");
    };
    assert_eq!(function, "main");
    assert_eq!(old.as_deref(), Some("int"));
    assert_eq!(new.as_deref(), Some("float"));
    assert!(source_span.is_some());
    assert!(
        !runtime
            .has_pending_hot_update()
            .expect("pending rejection should be consumed")
    );
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(1))
    );
}

#[test]
fn runtime_checks_reload_around_patch_apply_safe_point() {
    let engine = Engine::builder()
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial(
            SourceId::new(1),
            r#"
fn main(player: Player) {
    player.level += 1;
    return player.level;
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
    player.level += 2;
    return player.level + 100;
}
"#,
        )
        .expect("compatible update should compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let level_path = HostPath::new(host_ref).field(FieldId::new(1));
    let mut adapter = MockStateAdapter::new();
    adapter.insert_value(level_path, HostValue::Int(10));
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call(
            "main",
            &[Value::HostRef(host_ref)],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx,
        ),
        Ok(Value::Int(11))
    );
    runtime
        .stage_hot_update(update)
        .expect("stage pending update");

    let safe_point = runtime
        .apply_patch_tx_at_safe_point(tx, &mut adapter)
        .expect("apply patches at safe point");

    let before = safe_point
        .before_apply_reload
        .expect("pending update should be consumed before patch apply");
    assert!(before.accepted);
    assert_eq!(before.changed_functions, vec!["main".to_owned()]);
    assert_eq!(safe_point.after_apply_reload, None);

    let mut next_tx = PatchTx::new();
    assert_eq!(
        runtime.call(
            "main",
            &[Value::HostRef(host_ref)],
            CallOptions::unbounded(),
            &mut adapter,
            &mut next_tx,
        ),
        Ok(Value::Int(113))
    );
}

#[test]
fn runtime_safe_point_error_keeps_before_apply_reload_report() {
    let engine = Engine::builder()
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial(
            SourceId::new(1),
            r#"
fn main(player: Player) {
    player.level += 1;
    return player.level;
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
    player.level += 2;
    return player.level;
}
"#,
        )
        .expect("compatible update should compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let level_path = HostPath::new(host_ref).field(FieldId::new(1));
    let mut adapter = MockStateAdapter::new();
    adapter.insert_value(level_path.clone(), HostValue::Int(10));
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call(
            "main",
            &[Value::HostRef(host_ref)],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx,
        ),
        Ok(Value::Int(11))
    );
    runtime
        .stage_hot_update(update)
        .expect("stage pending update");
    adapter.deny_write(level_path.clone());

    let error = runtime
        .apply_patch_tx_at_safe_point(tx, &mut adapter)
        .expect_err("denied host write should fail patch apply");

    assert!(matches!(
        error.host_error.kind,
        HostErrorKind::PermissionDenied {
            path,
            action: "write",
        } if path == level_path
    ));
    let before = error
        .before_apply_reload
        .expect("pending reload report should be preserved on host error");
    assert!(before.accepted);
    assert_eq!(before.changed_functions, vec!["main".to_owned()]);
    assert!(
        !runtime
            .has_pending_hot_update()
            .expect("reload report was consumed before patch apply")
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
fn runtime_stages_hot_reload_file_until_check_reload_safe_point() {
    let root = unique_test_dir("runtime_stage_hot_reload_file");
    std::fs::create_dir_all(&root).expect("create temp source dir");
    let path = root.join("main.vela");
    std::fs::write(&path, "fn main() { return 1; }").expect("write initial source");
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_file(&path)
        .expect("initial hot reload file compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(1))
    );

    std::fs::write(&path, "fn main() { return 5; }").expect("write updated source");
    runtime
        .stage_hot_reload_update_file(&path)
        .expect("runtime should be hot-reload enabled")
        .expect("file update should stage");
    assert!(
        runtime
            .has_pending_hot_update()
            .expect("file update should be pending")
    );
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(1))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged file report");

    assert!(report.accepted);
    assert_eq!(report.changed_functions, vec!["main"]);
    assert!(
        !runtime
            .has_pending_hot_update()
            .expect("safe point should consume file update")
    );
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(5))
    );
}

#[test]
fn runtime_stages_source_file_private_helper_addition_until_safe_point() {
    let engine = Engine::builder().build().expect("engine should build");
    let mut runtime = runtime_from_hot_reload_source(engine, "fn main() { return 1; }");
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    stage_source_update(
        &mut runtime,
        r#"
fn helper() {
    return 7;
}

fn main() {
    return helper();
}
"#,
    );
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(1))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged helper report");

    assert!(report.accepted);
    assert_eq!(report.changed_functions, vec!["helper", "main"]);
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(7))
    );
}

#[test]
fn runtime_stages_source_file_public_function_addition_until_safe_point() {
    let engine = Engine::builder().build().expect("engine should build");
    let mut runtime = runtime_from_hot_reload_source(engine, "pub fn main() { return 1; }");
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    stage_source_update(
        &mut runtime,
        r#"
pub fn helper() {
    return 7;
}

pub fn main() {
    return helper();
}
"#,
    );
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(1))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged public function report");

    assert!(report.accepted);
    assert_eq!(report.changed_functions, vec!["helper", "main"]);
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(7))
    );
    assert_eq!(
        runtime.call(
            "helper",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(7))
    );
}

#[test]
fn runtime_stages_source_file_defaulted_schema_addition_until_safe_point() {
    let engine = Engine::builder().build().expect("engine should build");
    let mut runtime = runtime_from_hot_reload_source(
        engine,
        r#"
struct Reward {
    item_id: string
}

fn main() {
    return 1;
}
"#,
    );
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    stage_source_update(
        &mut runtime,
        r#"
struct Reward {
    item_id: string
    count: int = 1
}

fn main() {
    return 2;
}
"#,
    );
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(1))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged schema addition report");

    assert!(report.accepted);
    assert_eq!(report.changed_functions, vec!["main"]);
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(2))
    );
}

#[test]
fn runtime_stages_source_file_required_schema_field_rejection_until_safe_point() {
    let engine = Engine::builder().build().expect("engine should build");
    let mut runtime = runtime_from_hot_reload_source(
        engine,
        r#"
struct Reward {
    item_id: string
}

fn main() {
    return 1;
}
"#,
    );
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    stage_source_update(
        &mut runtime,
        r#"
struct Reward {
    item_id: string
    count: int
}

fn main() {
    return 2;
}
"#,
    );
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(1))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged schema field rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.schema.abi_changed");
    assert_eq!(report.errors[0].target.as_deref(), Some("Reward"));
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(1))
    );
}

#[test]
fn runtime_stages_source_file_schema_field_type_rejection_until_safe_point() {
    let engine = Engine::builder().build().expect("engine should build");
    let mut runtime = runtime_from_hot_reload_source(
        engine,
        r#"
struct Reward {
    item_id: string
    count: int
}

fn main() {
    return 1;
}
"#,
    );
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    stage_source_update(
        &mut runtime,
        r#"
struct Reward {
    item_id: string
    count: float
}

fn main() {
    return 2;
}
"#,
    );
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(1))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged schema field type rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.schema.abi_changed");
    assert_eq!(report.errors[0].target.as_deref(), Some("Reward"));
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(1))
    );
}

#[test]
fn runtime_stages_source_file_defaulted_enum_variant_field_addition_until_safe_point() {
    let engine = Engine::builder().build().expect("engine should build");
    let mut runtime = runtime_from_hot_reload_source(
        engine,
        r#"
enum QuestProgress {
    Active {
        quest_id: string
    }
}

fn main() {
    return 1;
}
"#,
    );
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    stage_source_update(
        &mut runtime,
        r#"
enum QuestProgress {
    Active {
        quest_id: string
        count: int = 0
    }
}

fn main() {
    return 2;
}
"#,
    );
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(1))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged enum variant field addition report");

    assert!(report.accepted);
    assert_eq!(report.changed_functions, vec!["main"]);
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(2))
    );
}

#[test]
fn runtime_stages_source_file_required_enum_variant_field_rejection_until_safe_point() {
    let engine = Engine::builder().build().expect("engine should build");
    let mut runtime = runtime_from_hot_reload_source(
        engine,
        r#"
enum QuestProgress {
    Active {
        quest_id: string
    }
}

fn main() {
    return 1;
}
"#,
    );
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    stage_source_update(
        &mut runtime,
        r#"
enum QuestProgress {
    Active {
        quest_id: string
        count: int
    }
}

fn main() {
    return 2;
}
"#,
    );
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(1))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged enum variant field rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.schema.abi_changed");
    assert_eq!(report.errors[0].target.as_deref(), Some("QuestProgress"));
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(1))
    );
}

#[test]
fn runtime_stages_source_file_event_parameter_reorder_rejection_until_safe_point() {
    let engine = Engine::builder().build().expect("engine should build");
    let mut runtime = runtime_from_hot_reload_source(
        engine,
        r#"
#[event("monster.kill")]
fn on_kill(player_id: int, monster_id: int) {
    return 1;
}
"#,
    );
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    stage_source_update(
        &mut runtime,
        r#"
#[event("monster.kill")]
fn on_kill(monster_id: int, player_id: int) {
    return 2;
}
"#,
    );
    assert_eq!(
        runtime.call(
            "on_kill",
            &[Value::Int(7), Value::Int(11)],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(1))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged event ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.function.changed_parameters");
    let HotReloadErrorKind::ChangedFunctionParameters { function, old, new } =
        &report.errors[0].error.kind
    else {
        panic!("expected changed function parameters");
    };
    assert_eq!(function, "on_kill");
    assert_eq!(old, &vec!["player_id".to_owned(), "monster_id".to_owned()]);
    assert_eq!(new, &vec!["monster_id".to_owned(), "player_id".to_owned()]);
    assert_eq!(
        runtime.call(
            "on_kill",
            &[Value::Int(7), Value::Int(11)],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(1))
    );
}

#[test]
fn runtime_stages_source_file_event_target_rejection_until_safe_point() {
    let engine = Engine::builder().build().expect("engine should build");
    let mut runtime = runtime_from_hot_reload_source(
        engine,
        r#"
#[event("monster.kill")]
fn on_kill(player_id: int, monster_id: int) {
    return 1;
}
"#,
    );
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    stage_source_update(
        &mut runtime,
        r#"
#[event("quest.complete")]
fn on_kill(player_id: int, monster_id: int) {
    return 2;
}
"#,
    );
    assert_eq!(
        runtime.call(
            "on_kill",
            &[Value::Int(7), Value::Int(11)],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(1))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged event target rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.function.event_changed");
    let HotReloadErrorKind::ChangedFunctionEvent {
        function,
        old,
        new,
        source_span,
    } = &report.errors[0].error.kind
    else {
        panic!("expected changed function event");
    };
    assert_eq!(function, "on_kill");
    assert_eq!(old.as_deref(), Some("monster.kill"));
    assert_eq!(new.as_deref(), Some("quest.complete"));
    assert!(source_span.is_some());
    assert_eq!(
        runtime.call(
            "on_kill",
            &[Value::Int(7), Value::Int(11)],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(1))
    );
}

#[test]
fn runtime_stages_source_file_return_abi_rejection_until_safe_point() {
    let engine = Engine::builder().build().expect("engine should build");
    let mut runtime = runtime_from_hot_reload_source(
        engine,
        r#"
fn main() -> int {
    return 1;
}
"#,
    );
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    stage_source_update(
        &mut runtime,
        r#"
fn main() -> float {
    return 2.0;
}
"#,
    );
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(1))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged return ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.function.return_abi_changed");
    let HotReloadErrorKind::ChangedFunctionReturnAbi {
        function,
        old,
        new,
        source_span,
    } = &report.errors[0].error.kind
    else {
        panic!("expected changed function return ABI");
    };
    assert_eq!(function, "main");
    assert_eq!(old.as_deref(), Some("int"));
    assert_eq!(new.as_deref(), Some("float"));
    assert!(source_span.is_some());
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(1))
    );
}

#[test]
fn runtime_stages_source_file_required_parameter_addition_rejection_until_safe_point() {
    let engine = Engine::builder().build().expect("engine should build");
    let mut runtime = runtime_from_hot_reload_source(
        engine,
        r#"
fn main(player_id: int) {
    return player_id;
}
"#,
    );
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    stage_source_update(
        &mut runtime,
        r#"
fn main(player_id: int, amount: int) {
    return amount;
}
"#,
    );
    assert_eq!(
        runtime.call(
            "main",
            &[Value::Int(7)],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(7))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged required parameter rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(
        report.errors[0].code,
        "reload.function.required_added_parameters"
    );
    let HotReloadErrorKind::AddedFunctionParametersWithoutDefaults { function, added } =
        &report.errors[0].error.kind
    else {
        panic!("expected added required parameters");
    };
    assert_eq!(function, "main");
    assert_eq!(added, &vec!["amount".to_owned()]);
    assert_eq!(
        runtime.call(
            "main",
            &[Value::Int(7)],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(7))
    );
}

#[test]
fn runtime_stages_source_file_native_effect_rejection_until_safe_point() {
    let old_engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::reward::grant", NativeFunctionId::new(22))
                .effects(EffectSet::host_read()),
            |_| Ok(Value::Null),
        )
        .build()
        .expect("old engine should build");
    let initial = hot_reload_initial_from_source(&old_engine, "fn main() { return 1; }");
    let new_engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::reward::grant", NativeFunctionId::new(22))
                .effects(EffectSet::host_write()),
            |_| Ok(Value::Null),
        )
        .build()
        .expect("new engine should build");
    let mut runtime = Runtime::from_hot_reload_version(new_engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    stage_source_update(&mut runtime, "fn main() { return 2; }");
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(1))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged native effect ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.function.effects_changed");
    let HotReloadErrorKind::ChangedFunctionEffects {
        function,
        old,
        new,
        source_span,
    } = &report.errors[0].error.kind
    else {
        panic!("expected changed native function effects");
    };
    assert_eq!(function, "game::reward::grant");
    assert!(old.reads_host);
    assert!(!old.writes_host);
    assert!(new.reads_host);
    assert!(new.writes_host);
    assert!(source_span.is_none());
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(1))
    );
}

#[test]
fn runtime_stages_source_file_native_access_rejection_until_safe_point() {
    let old_engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::reward::grant", NativeFunctionId::new(22)).access(
                FunctionAccess::public()
                    .reflect_callable(true)
                    .require_permission("reward.read"),
            ),
            |_| Ok(Value::Null),
        )
        .build()
        .expect("old engine should build");
    let initial = hot_reload_initial_from_source(&old_engine, "fn main() { return 1; }");
    let new_engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::reward::grant", NativeFunctionId::new(22)).access(
                FunctionAccess::public()
                    .reflect_callable(true)
                    .require_permission("reward.write"),
            ),
            |_| Ok(Value::Null),
        )
        .build()
        .expect("new engine should build");
    let mut runtime = Runtime::from_hot_reload_version(new_engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    stage_source_update(&mut runtime, "fn main() { return 2; }");
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(1))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged native access ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.function.access_changed");
    let HotReloadErrorKind::ChangedFunctionAccess {
        function,
        old,
        new,
        source_span,
    } = &report.errors[0].error.kind
    else {
        panic!("expected changed native function access");
    };
    assert_eq!(function, "game::reward::grant");
    assert_eq!(old.required_permissions, vec!["reward.read"]);
    assert_eq!(new.required_permissions, vec!["reward.write"]);
    assert!(old.callable);
    assert!(new.callable);
    assert!(source_span.is_none());
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(1))
    );
}

#[test]
fn runtime_stages_source_file_native_parameter_rejection_until_safe_point() {
    let old_engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::reward::grant", NativeFunctionId::new(22))
                .param("amount", TypeHint::Int),
            |_| Ok(Value::Null),
        )
        .build()
        .expect("old engine should build");
    let initial = hot_reload_initial_from_source(&old_engine, "fn main() { return 1; }");
    let new_engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::reward::grant", NativeFunctionId::new(22))
                .param("amount", TypeHint::Float),
            |_| Ok(Value::Null),
        )
        .build()
        .expect("new engine should build");
    let mut runtime = Runtime::from_hot_reload_version(new_engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    stage_source_update(&mut runtime, "fn main() { return 2; }");
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(1))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged native parameter ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(
        report.errors[0].code,
        "reload.function.parameter_abi_changed"
    );
    let HotReloadErrorKind::ChangedFunctionParameterAbi {
        function,
        old,
        new,
        source_span,
    } = &report.errors[0].error.kind
    else {
        panic!("expected changed native function parameter ABI");
    };
    assert_eq!(function, "game::reward::grant");
    assert_eq!(old.len(), 1);
    assert_eq!(old[0].name, "amount");
    assert_eq!(old[0].type_hint.as_deref(), Some("int"));
    assert_eq!(new.len(), 1);
    assert_eq!(new[0].name, "amount");
    assert_eq!(new[0].type_hint.as_deref(), Some("float"));
    assert!(source_span.is_none());
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(1))
    );
}

#[test]
fn runtime_stages_source_file_native_return_rejection_until_safe_point() {
    let old_engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::reward::grant", NativeFunctionId::new(22))
                .returns(TypeHint::Int),
            |_| Ok(Value::Null),
        )
        .build()
        .expect("old engine should build");
    let initial = hot_reload_initial_from_source(&old_engine, "fn main() { return 1; }");
    let new_engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::reward::grant", NativeFunctionId::new(22))
                .returns(TypeHint::Float),
            |_| Ok(Value::Null),
        )
        .build()
        .expect("new engine should build");
    let mut runtime = Runtime::from_hot_reload_version(new_engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    stage_source_update(&mut runtime, "fn main() { return 2; }");
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(1))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged native return ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.function.return_abi_changed");
    let HotReloadErrorKind::ChangedFunctionReturnAbi {
        function,
        old,
        new,
        source_span,
    } = &report.errors[0].error.kind
    else {
        panic!("expected changed native function return ABI");
    };
    assert_eq!(function, "game::reward::grant");
    assert_eq!(old.as_deref(), Some("int"));
    assert_eq!(new.as_deref(), Some("float"));
    assert!(source_span.is_none());
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(1))
    );
}

#[test]
fn runtime_stages_source_file_method_effect_rejection_until_safe_point() {
    let player_key = TypeKey::new(TypeId::new(1), "Player");
    let old_engine = Engine::builder()
        .register_type(
            TypeDesc::new(player_key.clone())
                .host_type(HostTypeId::new(1))
                .method(
                    MethodDesc::new(HostMethodId::new(9), "grant_exp")
                        .effects(MethodEffectSet::host_read()),
                ),
        )
        .build()
        .expect("old engine should build");
    let initial = hot_reload_initial_from_source(&old_engine, "fn main() { return 1; }");
    let new_engine = Engine::builder()
        .register_type(
            TypeDesc::new(player_key)
                .host_type(HostTypeId::new(1))
                .method(
                    MethodDesc::new(HostMethodId::new(9), "grant_exp")
                        .effects(MethodEffectSet::host_write()),
                ),
        )
        .build()
        .expect("new engine should build");
    let mut runtime = Runtime::from_hot_reload_version(new_engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    stage_source_update(&mut runtime, "fn main() { return 2; }");
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(1))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged method effect ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.method.effects_changed");
    let HotReloadErrorKind::ChangedMethodEffects {
        type_name,
        method,
        old,
        new,
        source_span,
    } = &report.errors[0].error.kind
    else {
        panic!("expected changed host method effects");
    };
    assert_eq!(type_name, "Player");
    assert_eq!(method, "grant_exp");
    assert!(old.reads_host);
    assert!(!old.writes_host);
    assert!(new.reads_host);
    assert!(new.writes_host);
    assert!(source_span.is_none());
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(1))
    );
}

#[test]
fn runtime_stages_source_file_method_access_rejection_until_safe_point() {
    let player_key = TypeKey::new(TypeId::new(1), "Player");
    let old_engine = Engine::builder()
        .register_type(
            TypeDesc::new(player_key.clone())
                .host_type(HostTypeId::new(1))
                .method(
                    MethodDesc::new(HostMethodId::new(9), "grant_exp").access(
                        MethodAccess::new()
                            .reflect_callable(true)
                            .require_permission("player.read"),
                    ),
                ),
        )
        .build()
        .expect("old engine should build");
    let initial = hot_reload_initial_from_source(&old_engine, "fn main() { return 1; }");
    let new_engine = Engine::builder()
        .register_type(
            TypeDesc::new(player_key)
                .host_type(HostTypeId::new(1))
                .method(
                    MethodDesc::new(HostMethodId::new(9), "grant_exp").access(
                        MethodAccess::new()
                            .reflect_callable(false)
                            .require_permission("player.read"),
                    ),
                ),
        )
        .build()
        .expect("new engine should build");
    let mut runtime = Runtime::from_hot_reload_version(new_engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    stage_source_update(&mut runtime, "fn main() { return 2; }");
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(1))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged method access ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.method.access_changed");
    let HotReloadErrorKind::ChangedMethodAccess {
        type_name,
        method,
        old,
        new,
        source_span,
    } = &report.errors[0].error.kind
    else {
        panic!("expected changed host method access");
    };
    assert_eq!(type_name, "Player");
    assert_eq!(method, "grant_exp");
    assert_eq!(old.required_permissions, vec!["player.read"]);
    assert_eq!(new.required_permissions, vec!["player.read"]);
    assert!(old.callable);
    assert!(!new.callable);
    assert!(source_span.is_none());
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(1))
    );
}

#[test]
fn runtime_stages_source_file_method_parameter_rejection_until_safe_point() {
    let player_key = TypeKey::new(TypeId::new(1), "Player");
    let old_engine = Engine::builder()
        .register_type(
            TypeDesc::new(player_key.clone())
                .host_type(HostTypeId::new(1))
                .method(
                    MethodDesc::new(HostMethodId::new(9), "grant_exp")
                        .param(MethodParamDesc::new("amount").type_hint("int")),
                ),
        )
        .build()
        .expect("old engine should build");
    let initial = hot_reload_initial_from_source(&old_engine, "fn main() { return 1; }");
    let new_engine = Engine::builder()
        .register_type(
            TypeDesc::new(player_key)
                .host_type(HostTypeId::new(1))
                .method(
                    MethodDesc::new(HostMethodId::new(9), "grant_exp")
                        .param(MethodParamDesc::new("amount").type_hint("float")),
                ),
        )
        .build()
        .expect("new engine should build");
    let mut runtime = Runtime::from_hot_reload_version(new_engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    stage_source_update(&mut runtime, "fn main() { return 2; }");
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(1))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged method parameter ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.method.parameter_abi_changed");
    let HotReloadErrorKind::ChangedMethodParameterAbi {
        type_name,
        method,
        old,
        new,
        source_span,
    } = &report.errors[0].error.kind
    else {
        panic!("expected changed host method parameter ABI");
    };
    assert_eq!(type_name, "Player");
    assert_eq!(method, "grant_exp");
    assert_eq!(old.len(), 1);
    assert_eq!(old[0].name, "amount");
    assert_eq!(old[0].type_hint.as_deref(), Some("int"));
    assert_eq!(new.len(), 1);
    assert_eq!(new[0].name, "amount");
    assert_eq!(new[0].type_hint.as_deref(), Some("float"));
    assert!(source_span.is_none());
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(1))
    );
}

#[test]
fn runtime_stages_source_file_method_return_rejection_until_safe_point() {
    let player_key = TypeKey::new(TypeId::new(1), "Player");
    let old_engine = Engine::builder()
        .register_type(
            TypeDesc::new(player_key.clone())
                .host_type(HostTypeId::new(1))
                .method(MethodDesc::new(HostMethodId::new(9), "grant_exp").return_type("int")),
        )
        .build()
        .expect("old engine should build");
    let initial = hot_reload_initial_from_source(&old_engine, "fn main() { return 1; }");
    let new_engine = Engine::builder()
        .register_type(
            TypeDesc::new(player_key)
                .host_type(HostTypeId::new(1))
                .method(MethodDesc::new(HostMethodId::new(9), "grant_exp").return_type("null")),
        )
        .build()
        .expect("new engine should build");
    let mut runtime = Runtime::from_hot_reload_version(new_engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    stage_source_update(&mut runtime, "fn main() { return 2; }");
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(1))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged method return ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.method.return_abi_changed");
    let HotReloadErrorKind::ChangedMethodReturnAbi {
        type_name,
        method,
        old,
        new,
        source_span,
    } = &report.errors[0].error.kind
    else {
        panic!("expected changed host method return ABI");
    };
    assert_eq!(type_name, "Player");
    assert_eq!(method, "grant_exp");
    assert_eq!(old.as_deref(), Some("int"));
    assert_eq!(new.as_deref(), Some("null"));
    assert!(source_span.is_none());
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(1))
    );
}

#[test]
fn runtime_stages_file_hot_reload_rejection_until_safe_point() {
    let engine = Engine::builder()
        .hot_reload_policy(HotReloadPolicy::locked_down())
        .build()
        .expect("engine should build");
    let mut runtime = runtime_from_hot_reload_source(engine, "fn main() { return 1; }");
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    stage_source_update(
        &mut runtime,
        r#"
fn helper() {
    return 2;
}

fn main() {
    return helper();
}
"#,
    );
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(1))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert!(matches!(
        report.errors[0].error.kind,
        HotReloadErrorKind::NewFunctionDenied { ref function }
            if function == "helper"
    ));
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(1))
    );
}

#[test]
fn runtime_stages_source_file_top_level_effect_rejection_until_safe_point() {
    let engine = Engine::builder().build().expect("engine should build");
    let mut runtime = runtime_from_hot_reload_source(engine, "fn main() { return 1; }");
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    stage_source_update(
        &mut runtime,
        r#"
const BAD = register_event("monster.kill");

fn main() {
    return 2;
}
"#,
    );
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(1))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged compile rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.compile");
    assert!(
        report.errors[0]
            .source_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.as_deref() == Some("hir::top_level_side_effect"))
    );
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(1))
    );
}

#[test]
fn runtime_returns_hot_reload_file_source_errors_immediately() {
    let root = unique_test_dir("runtime_stage_file_source_error");
    std::fs::create_dir_all(&root).expect("create temp source dir");
    let path = root.join("main.vela");
    std::fs::write(&path, "fn main() { return 1; }").expect("write initial source");
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_file(&path)
        .expect("initial hot reload file compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let missing = root.join("missing.vela");

    let error = runtime
        .stage_hot_reload_update_file(&missing)
        .expect("runtime should be hot-reload enabled")
        .expect_err("missing source should not stage a hot reload report");

    assert!(matches!(
        error.kind,
        EngineHotReloadSourceErrorKind::Source(crate::source::EngineSourceError {
            kind: EngineSourceErrorKind::Io { .. }
        })
    ));
    assert!(
        !runtime
            .has_pending_hot_update()
            .expect("source error should not stage an update")
    );
}

#[test]
fn runtime_compiles_hot_reload_changed_file_from_active_version() {
    let root = unique_test_dir("runtime_hot_reload_changed_file");
    let reward_file = write_reward_modules(&root, "return grant();", 2);
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    write_reward_module(&reward_file, 6);
    let update = runtime
        .compile_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("changed file update should compile");
    let report = runtime
        .apply_hot_update(update)
        .expect("runtime should apply changed file update");

    assert!(report.accepted);
    assert_eq!(report.changed_functions, vec!["game::reward::grant"]);
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(6))
    );
}

#[test]
fn runtime_stages_hot_reload_changed_file_until_check_reload_safe_point() {
    let root = unique_test_dir("runtime_stage_hot_reload_changed_file");
    let reward_file = write_reward_modules(&root, "return grant();", 2);
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    write_reward_module(&reward_file, 6);
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("changed file update should stage");
    assert!(
        runtime
            .has_pending_hot_update()
            .expect("changed file update should be pending")
    );
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged changed-file report");

    assert!(report.accepted);
    assert_eq!(report.changed_functions, vec!["game::reward::grant"]);
    assert!(
        !runtime
            .has_pending_hot_update()
            .expect("safe point should consume changed-file update")
    );
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(6))
    );
}

#[test]
fn runtime_stages_changed_file_hot_reload_rejection_until_safe_point() {
    let root = unique_test_dir("runtime_stage_changed_file_rejection");
    let reward_file = write_reward_modules(&root, "return grant();", 2);
    let engine = Engine::builder()
        .hot_reload_policy(HotReloadPolicy::locked_down())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    write_reward_module_with_helper(&reward_file, 6);
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("hot reload rejection should be staged");
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert!(matches!(
        report.errors[0].error.kind,
        HotReloadErrorKind::NewFunctionDenied { ref function }
            if function == "game::reward::helper"
    ));
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );
}

#[test]
fn runtime_stages_changed_file_return_abi_rejection_until_safe_point() {
    let root = unique_test_dir("runtime_stage_changed_file_return_abi");
    let reward_file = write_typed_reward_modules(&root, "return grant();", "int", "2");
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    write_typed_reward_module(&reward_file, "float", "6.0");
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("changed-file return ABI rejection should be staged");
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged changed-file return ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.function.return_abi_changed");
    let HotReloadErrorKind::ChangedFunctionReturnAbi {
        function,
        old,
        new,
        source_span,
    } = &report.errors[0].error.kind
    else {
        panic!("expected changed function return ABI");
    };
    assert_eq!(function, "game::reward::grant");
    assert_eq!(old.as_deref(), Some("int"));
    assert_eq!(new.as_deref(), Some("float"));
    assert!(source_span.is_some());
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );
}

#[test]
fn runtime_stages_changed_file_required_parameter_rejection_until_safe_point() {
    let root = unique_test_dir("runtime_stage_changed_file_required_parameter");
    let reward_file = write_typed_reward_modules(&root, "return 2;", "int", "2");
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    write_reward_module_with_signature(&reward_file, "(amount: int) -> int", "amount");
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("changed-file required parameter rejection should be staged");
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged changed-file required parameter rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(
        report.errors[0].code,
        "reload.function.required_added_parameters"
    );
    let HotReloadErrorKind::AddedFunctionParametersWithoutDefaults { function, added } =
        &report.errors[0].error.kind
    else {
        panic!("expected added required parameters");
    };
    assert_eq!(function, "game::reward::grant");
    assert_eq!(added, &vec!["amount".to_owned()]);
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );
}

#[test]
fn runtime_stages_changed_file_script_function_access_rejection_until_safe_point() {
    let root = unique_test_dir("runtime_stage_changed_file_script_access");
    let reward_file = write_reward_modules(&root, "return grant();", 2);
    let main_file = root.join("game").join("main.vela");
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    std::fs::write(
        &main_file,
        r#"
fn main() {
    return 3;
}
"#,
    )
    .expect("write main without reward import");
    std::fs::write(
        &reward_file,
        r#"
fn grant() {
    return 6;
}
"#,
    )
    .expect("write reward without public export");
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("changed-file script function access ABI rejection should be staged");
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged changed-file script function access ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.function.access_changed");
    assert_eq!(
        report.errors[0].repair_hint.as_deref(),
        Some("preserve reflective access metadata or require host approval before reloading")
    );
    let HotReloadErrorKind::ChangedFunctionAccess {
        function,
        old,
        new,
        source_span,
    } = &report.errors[0].error.kind
    else {
        panic!("expected changed function access ABI");
    };
    assert_eq!(function, "game::reward::grant");
    assert!(old.public);
    assert!(!new.public);
    assert_eq!(old.required_permissions, new.required_permissions);
    assert!(source_span.is_some());
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );
}

#[test]
fn runtime_stages_changed_file_native_effect_rejection_until_safe_point() {
    let root = unique_test_dir("runtime_stage_changed_file_native_effect");
    let reward_file = write_reward_modules(&root, "return grant();", 2);
    let old_engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::native::grant_bonus", NativeFunctionId::new(22))
                .effects(EffectSet::host_read()),
            |_| Ok(Value::Null),
        )
        .build()
        .expect("old engine should build");
    let initial = old_engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let new_engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::native::grant_bonus", NativeFunctionId::new(22))
                .effects(EffectSet::host_write()),
            |_| Ok(Value::Null),
        )
        .build()
        .expect("new engine should build");
    let mut runtime = Runtime::from_hot_reload_version(new_engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    write_reward_module(&reward_file, 6);
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("changed-file native effect ABI rejection should be staged");
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged changed-file native effect ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.function.effects_changed");
    let HotReloadErrorKind::ChangedFunctionEffects {
        function,
        old,
        new,
        source_span,
    } = &report.errors[0].error.kind
    else {
        panic!("expected changed native function effects");
    };
    assert_eq!(function, "game::native::grant_bonus");
    assert!(old.reads_host);
    assert!(!old.writes_host);
    assert!(new.reads_host);
    assert!(new.writes_host);
    assert!(source_span.is_none());
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );
}

#[test]
fn runtime_stages_changed_file_native_access_rejection_until_safe_point() {
    let root = unique_test_dir("runtime_stage_changed_file_native_access");
    let reward_file = write_reward_modules(&root, "return grant();", 2);
    let old_engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::native::grant_bonus", NativeFunctionId::new(22)).access(
                FunctionAccess::public()
                    .reflect_callable(true)
                    .require_permission("reward.read"),
            ),
            |_| Ok(Value::Null),
        )
        .build()
        .expect("old engine should build");
    let initial = old_engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let new_engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::native::grant_bonus", NativeFunctionId::new(22)).access(
                FunctionAccess::public()
                    .reflect_callable(true)
                    .require_permission("reward.write"),
            ),
            |_| Ok(Value::Null),
        )
        .build()
        .expect("new engine should build");
    let mut runtime = Runtime::from_hot_reload_version(new_engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    write_reward_module(&reward_file, 6);
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("changed-file native access ABI rejection should be staged");
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged changed-file native access ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.function.access_changed");
    let HotReloadErrorKind::ChangedFunctionAccess {
        function,
        old,
        new,
        source_span,
    } = &report.errors[0].error.kind
    else {
        panic!("expected changed native function access");
    };
    assert_eq!(function, "game::native::grant_bonus");
    assert_eq!(old.required_permissions, vec!["reward.read"]);
    assert_eq!(new.required_permissions, vec!["reward.write"]);
    assert!(old.callable);
    assert!(new.callable);
    assert!(source_span.is_none());
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );
}

#[test]
fn runtime_stages_changed_file_native_parameter_rejection_until_safe_point() {
    let root = unique_test_dir("runtime_stage_changed_file_native_parameter");
    let reward_file = write_reward_modules(&root, "return grant();", 2);
    let old_engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::native::grant_bonus", NativeFunctionId::new(22))
                .param("amount", TypeHint::Int),
            |_| Ok(Value::Null),
        )
        .build()
        .expect("old engine should build");
    let initial = old_engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let new_engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::native::grant_bonus", NativeFunctionId::new(22))
                .param("amount", TypeHint::Float),
            |_| Ok(Value::Null),
        )
        .build()
        .expect("new engine should build");
    let mut runtime = Runtime::from_hot_reload_version(new_engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    write_reward_module(&reward_file, 6);
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("changed-file native parameter ABI rejection should be staged");
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged changed-file native parameter ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(
        report.errors[0].code,
        "reload.function.parameter_abi_changed"
    );
    let HotReloadErrorKind::ChangedFunctionParameterAbi {
        function,
        old,
        new,
        source_span,
    } = &report.errors[0].error.kind
    else {
        panic!("expected changed native function parameter ABI");
    };
    assert_eq!(function, "game::native::grant_bonus");
    assert_eq!(old.len(), 1);
    assert_eq!(old[0].name, "amount");
    assert_eq!(old[0].type_hint.as_deref(), Some("int"));
    assert_eq!(new.len(), 1);
    assert_eq!(new[0].name, "amount");
    assert_eq!(new[0].type_hint.as_deref(), Some("float"));
    assert!(source_span.is_none());
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );
}

#[test]
fn runtime_stages_changed_file_native_return_rejection_until_safe_point() {
    let root = unique_test_dir("runtime_stage_changed_file_native_return");
    let reward_file = write_reward_modules(&root, "return grant();", 2);
    let old_engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::native::grant_bonus", NativeFunctionId::new(22))
                .returns(TypeHint::Int),
            |_| Ok(Value::Null),
        )
        .build()
        .expect("old engine should build");
    let initial = old_engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let new_engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::native::grant_bonus", NativeFunctionId::new(22))
                .returns(TypeHint::Float),
            |_| Ok(Value::Null),
        )
        .build()
        .expect("new engine should build");
    let mut runtime = Runtime::from_hot_reload_version(new_engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    write_reward_module(&reward_file, 6);
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("changed-file native return ABI rejection should be staged");
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged changed-file native return ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.function.return_abi_changed");
    let HotReloadErrorKind::ChangedFunctionReturnAbi {
        function,
        old,
        new,
        source_span,
    } = &report.errors[0].error.kind
    else {
        panic!("expected changed native function return ABI");
    };
    assert_eq!(function, "game::native::grant_bonus");
    assert_eq!(old.as_deref(), Some("int"));
    assert_eq!(new.as_deref(), Some("float"));
    assert!(source_span.is_none());
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );
}

#[test]
fn runtime_stages_changed_file_method_effect_rejection_until_safe_point() {
    let kind = changed_file_method_rejection_kind(
        "runtime_stage_changed_file_method_effect",
        MethodDesc::new(HostMethodId::new(9), "grant_exp").effects(MethodEffectSet::host_read()),
        MethodDesc::new(HostMethodId::new(9), "grant_exp").effects(MethodEffectSet::host_write()),
        "reload.method.effects_changed",
    );

    let HotReloadErrorKind::ChangedMethodEffects {
        type_name,
        method,
        old,
        new,
        source_span,
    } = kind
    else {
        panic!("expected changed host method effects");
    };
    assert_eq!(type_name, "Player");
    assert_eq!(method, "grant_exp");
    assert!(old.reads_host);
    assert!(!old.writes_host);
    assert!(new.reads_host);
    assert!(new.writes_host);
    assert!(source_span.is_none());
}

#[test]
fn runtime_stages_changed_file_method_access_rejection_until_safe_point() {
    let kind = changed_file_method_rejection_kind(
        "runtime_stage_changed_file_method_access",
        MethodDesc::new(HostMethodId::new(9), "grant_exp").access(
            MethodAccess::new()
                .reflect_callable(true)
                .require_permission("player.read"),
        ),
        MethodDesc::new(HostMethodId::new(9), "grant_exp").access(
            MethodAccess::new()
                .reflect_callable(false)
                .require_permission("player.read"),
        ),
        "reload.method.access_changed",
    );

    let HotReloadErrorKind::ChangedMethodAccess {
        type_name,
        method,
        old,
        new,
        source_span,
    } = kind
    else {
        panic!("expected changed host method access");
    };
    assert_eq!(type_name, "Player");
    assert_eq!(method, "grant_exp");
    assert_eq!(old.required_permissions, vec!["player.read"]);
    assert_eq!(new.required_permissions, vec!["player.read"]);
    assert!(old.callable);
    assert!(!new.callable);
    assert!(source_span.is_none());
}

#[test]
fn runtime_stages_changed_file_method_parameter_rejection_until_safe_point() {
    let kind = changed_file_method_rejection_kind(
        "runtime_stage_changed_file_method_parameter",
        MethodDesc::new(HostMethodId::new(9), "grant_exp")
            .param(MethodParamDesc::new("amount").type_hint("int")),
        MethodDesc::new(HostMethodId::new(9), "grant_exp")
            .param(MethodParamDesc::new("amount").type_hint("float")),
        "reload.method.parameter_abi_changed",
    );

    let HotReloadErrorKind::ChangedMethodParameterAbi {
        type_name,
        method,
        old,
        new,
        source_span,
    } = kind
    else {
        panic!("expected changed host method parameter ABI");
    };
    assert_eq!(type_name, "Player");
    assert_eq!(method, "grant_exp");
    assert_eq!(old.len(), 1);
    assert_eq!(old[0].name, "amount");
    assert_eq!(old[0].type_hint.as_deref(), Some("int"));
    assert_eq!(new.len(), 1);
    assert_eq!(new[0].name, "amount");
    assert_eq!(new[0].type_hint.as_deref(), Some("float"));
    assert!(source_span.is_none());
}

#[test]
fn runtime_stages_changed_file_method_return_rejection_until_safe_point() {
    let kind = changed_file_method_rejection_kind(
        "runtime_stage_changed_file_method_return",
        MethodDesc::new(HostMethodId::new(9), "grant_exp").return_type("int"),
        MethodDesc::new(HostMethodId::new(9), "grant_exp").return_type("null"),
        "reload.method.return_abi_changed",
    );

    let HotReloadErrorKind::ChangedMethodReturnAbi {
        type_name,
        method,
        old,
        new,
        source_span,
    } = kind
    else {
        panic!("expected changed host method return ABI");
    };
    assert_eq!(type_name, "Player");
    assert_eq!(method, "grant_exp");
    assert_eq!(old.as_deref(), Some("int"));
    assert_eq!(new.as_deref(), Some("null"));
    assert!(source_span.is_none());
}

#[test]
fn runtime_stages_changed_file_defaulted_schema_addition_until_safe_point() {
    let root = unique_test_dir("runtime_stage_changed_file_defaulted_schema_addition");
    let reward_file = write_schema_reward_modules(&root, 2, StructCountField::Absent);
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    write_schema_reward_module(&reward_file, 6, StructCountField::Defaulted);
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("changed-file defaulted schema addition should be staged");
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged changed-file schema addition report");

    assert!(report.accepted);
    assert_eq!(report.changed_functions, vec!["game::reward::grant"]);
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(6))
    );
}

#[test]
fn runtime_stages_changed_file_required_schema_field_rejection_until_safe_point() {
    let root = unique_test_dir("runtime_stage_changed_file_required_schema_field_rejection");
    let reward_file = write_schema_reward_modules(&root, 2, StructCountField::Absent);
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    write_schema_reward_module(&reward_file, 6, StructCountField::Required);
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("changed-file schema field rejection should be staged");
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged changed-file schema field rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.schema.abi_changed");
    assert_eq!(
        report.errors[0].target.as_deref(),
        Some("game::reward::Reward")
    );
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );
}

#[test]
fn runtime_stages_changed_file_schema_field_type_rejection_until_safe_point() {
    let root = unique_test_dir("runtime_stage_changed_file_schema_field_type_rejection");
    let reward_file = write_schema_reward_modules(&root, 2, StructCountField::Required);
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    write_schema_reward_module(&reward_file, 6, StructCountField::Float);
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("changed-file schema field type rejection should be staged");
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged changed-file schema field type rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.schema.abi_changed");
    assert_eq!(
        report.errors[0].target.as_deref(),
        Some("game::reward::Reward")
    );
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );
}

#[test]
fn runtime_stages_changed_file_defaulted_enum_variant_field_addition_until_safe_point() {
    let root = unique_test_dir("runtime_stage_changed_file_defaulted_enum_variant_field_addition");
    let reward_file = write_enum_reward_modules(&root, 2, EnumVariantCountField::Absent);
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    write_enum_reward_module(&reward_file, 6, EnumVariantCountField::Defaulted);
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("changed-file defaulted enum variant field addition should be staged");
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged changed-file enum variant field addition report");

    assert!(report.accepted);
    assert_eq!(report.changed_functions, vec!["game::reward::grant"]);
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(6))
    );
}

#[test]
fn runtime_stages_changed_file_required_enum_variant_field_rejection_until_safe_point() {
    let root = unique_test_dir("runtime_stage_changed_file_required_enum_variant_field_rejection");
    let reward_file = write_enum_reward_modules(&root, 2, EnumVariantCountField::Absent);
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    write_enum_reward_module(&reward_file, 6, EnumVariantCountField::Required);
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("changed-file enum variant field rejection should be staged");
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged changed-file enum variant field rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.schema.abi_changed");
    assert_eq!(
        report.errors[0].target.as_deref(),
        Some("game::reward::QuestProgress")
    );
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );
}

#[test]
fn runtime_stages_changed_file_compile_rejection_until_safe_point() {
    let root = unique_test_dir("runtime_stage_changed_file_compile_rejection");
    let reward_file = write_reward_modules(&root, "return grant();", 2);
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    std::fs::write(
        &reward_file,
        r#"
const BAD = register_event("monster.kill");

pub fn grant() {
    return 6;
}
"#,
    )
    .expect("write side-effecting changed file");
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("compile rejection should be staged as a hot reload report");
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged changed-file compile rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.compile");
    assert!(
        report.errors[0]
            .source_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.as_deref() == Some("hir::top_level_side_effect"))
    );
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );
}

#[test]
fn runtime_returns_hot_reload_changed_file_source_errors_immediately() {
    let root = unique_test_dir("runtime_stage_changed_file_source_error");
    let _reward_file = write_reward_modules(&root, "return grant();", 2);
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let changed = root.join("game").join("reward.txt");
    std::fs::write(&changed, "not a vela source file").expect("write non-source file");

    let error = runtime
        .stage_hot_reload_update_changed_file(&root, &changed)
        .expect("runtime should be hot-reload enabled")
        .expect_err("invalid changed-file path should not stage a hot reload report");

    assert!(matches!(
        error.kind,
        EngineHotReloadSourceErrorKind::Source(crate::source::EngineSourceError {
            kind: EngineSourceErrorKind::InvalidSourcePath { .. }
        })
    ));
    assert!(
        !runtime
            .has_pending_hot_update()
            .expect("source error should not stage an update")
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

struct TestDir(std::path::PathBuf);

impl TestDir {
    fn join(&self, path: impl AsRef<std::path::Path>) -> std::path::PathBuf {
        self.0.join(path)
    }
}

impl AsRef<std::path::Path> for TestDir {
    fn as_ref(&self) -> &std::path::Path {
        &self.0
    }
}

impl std::ops::Deref for TestDir {
    type Target = std::path::Path;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
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

fn runtime_from_hot_reload_source(engine: Engine, source: &str) -> Runtime {
    let initial = hot_reload_initial_from_source(&engine, source);
    Runtime::from_hot_reload_version(engine, initial)
}

fn hot_reload_initial_from_source(
    engine: &Engine,
    source: &str,
) -> vela_hot_reload::version::ProgramVersion {
    engine
        .compile_hot_reload_initial(SourceId::new(1), source)
        .expect("initial hot reload source compile")
}

fn stage_source_update(runtime: &mut Runtime, source: &str) {
    let update = runtime
        .compile_hot_reload_update(SourceId::new(2), source)
        .expect("runtime should be hot-reload enabled");
    runtime
        .stage_hot_update_result(update)
        .expect("source update should stage");
}

fn write_reward_modules(
    root: &std::path::Path,
    main_return: &str,
    reward: i64,
) -> std::path::PathBuf {
    let game_dir = root.join("game");
    std::fs::create_dir_all(&game_dir).expect("create module dir");
    std::fs::write(
        game_dir.join("main.vela"),
        format!(
            r#"
use game::reward::grant

fn main() {{
    {main_return}
}}
"#
        ),
    )
    .expect("write main module");
    let reward_file = game_dir.join("reward.vela");
    write_reward_module(&reward_file, reward);
    reward_file
}

fn write_reward_module(path: &std::path::Path, reward: i64) {
    std::fs::write(
        path,
        format!(
            r#"
pub fn grant() {{
    return {reward};
}}
"#
        ),
    )
    .expect("write reward module");
}

fn write_schema_reward_modules(
    root: &std::path::Path,
    reward: i64,
    count_field: StructCountField,
) -> std::path::PathBuf {
    let game_dir = root.join("game");
    std::fs::create_dir_all(&game_dir).expect("create module dir");
    std::fs::write(
        game_dir.join("main.vela"),
        r#"
use game::reward::grant

fn main() {
    return grant();
}
"#,
    )
    .expect("write main module");
    let reward_file = game_dir.join("reward.vela");
    write_schema_reward_module(&reward_file, reward, count_field);
    reward_file
}

#[derive(Clone, Copy)]
enum StructCountField {
    Absent,
    Defaulted,
    Required,
    Float,
}

impl StructCountField {
    const fn source(self) -> &'static str {
        match self {
            Self::Absent => "",
            Self::Defaulted => "    count: int = 1\n",
            Self::Required => "    count: int\n",
            Self::Float => "    count: float\n",
        }
    }
}

fn write_schema_reward_module(path: &std::path::Path, reward: i64, count_field: StructCountField) {
    let count_field = count_field.source();
    std::fs::write(
        path,
        format!(
            r#"
struct Reward {{
    item_id: string
{count_field}}}

pub fn grant() {{
    return {reward};
}}
"#
        ),
    )
    .expect("write schema reward module");
}

fn write_enum_reward_modules(
    root: &std::path::Path,
    reward: i64,
    count_field: EnumVariantCountField,
) -> std::path::PathBuf {
    let game_dir = root.join("game");
    std::fs::create_dir_all(&game_dir).expect("create module dir");
    std::fs::write(
        game_dir.join("main.vela"),
        r#"
use game::reward::grant

fn main() {
    return grant();
}
"#,
    )
    .expect("write main module");
    let reward_file = game_dir.join("reward.vela");
    write_enum_reward_module(&reward_file, reward, count_field);
    reward_file
}

#[derive(Clone, Copy)]
enum EnumVariantCountField {
    Absent,
    Defaulted,
    Required,
}

impl EnumVariantCountField {
    const fn source(self) -> &'static str {
        match self {
            Self::Absent => "",
            Self::Defaulted => "        count: int = 0\n",
            Self::Required => "        count: int\n",
        }
    }
}

fn write_enum_reward_module(
    path: &std::path::Path,
    reward: i64,
    count_field: EnumVariantCountField,
) {
    let count_field = count_field.source();
    std::fs::write(
        path,
        format!(
            r#"
enum QuestProgress {{
    Active {{
        quest_id: string
{count_field}    }}
}}

pub fn grant() {{
    return {reward};
}}
"#
        ),
    )
    .expect("write enum reward module");
}

fn dir_native_rejection_kind(
    test_name: &str,
    old_desc: NativeFunctionDesc,
    new_desc: NativeFunctionDesc,
    expected_code: &str,
) -> HotReloadErrorKind {
    let root = unique_test_dir(test_name);
    let reward_file = write_reward_modules(&root, "return grant();", 2);
    let old_engine = Engine::builder()
        .register_native_fn(old_desc, |_| Ok(Value::Null))
        .build()
        .expect("old engine should build");
    let initial = old_engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let new_engine = Engine::builder()
        .register_native_fn(new_desc, |_| Ok(Value::Null))
        .build()
        .expect("new engine should build");
    let mut runtime = Runtime::from_hot_reload_version(new_engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    write_reward_module(&reward_file, 6);
    runtime
        .stage_hot_reload_update_dir(&root)
        .expect("runtime should be hot-reload enabled")
        .expect("dir native descriptor ABI rejection should be staged");
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged dir native descriptor ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, expected_code);
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );
    report.errors[0].error.kind.clone()
}

fn dir_method_rejection_kind(
    test_name: &str,
    old_method: MethodDesc,
    new_method: MethodDesc,
    expected_code: &str,
) -> HotReloadErrorKind {
    let root = unique_test_dir(test_name);
    let reward_file = write_reward_modules(&root, "return grant();", 2);
    let old_engine = Engine::builder()
        .register_type(type_with_reload_method(old_method))
        .build()
        .expect("old engine should build");
    let initial = old_engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let new_engine = Engine::builder()
        .register_type(type_with_reload_method(new_method))
        .build()
        .expect("new engine should build");
    let mut runtime = Runtime::from_hot_reload_version(new_engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    write_reward_module(&reward_file, 6);
    runtime
        .stage_hot_reload_update_dir(&root)
        .expect("runtime should be hot-reload enabled")
        .expect("dir method ABI rejection should be staged");
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged dir method ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, expected_code);
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );
    report.errors[0].error.kind.clone()
}

fn changed_file_method_rejection_kind(
    test_name: &str,
    old_method: MethodDesc,
    new_method: MethodDesc,
    expected_code: &str,
) -> HotReloadErrorKind {
    let root = unique_test_dir(test_name);
    let reward_file = write_reward_modules(&root, "return grant();", 2);
    let old_engine = Engine::builder()
        .register_type(type_with_reload_method(old_method))
        .build()
        .expect("old engine should build");
    let initial = old_engine
        .compile_hot_reload_initial_dir(&root)
        .expect("initial hot reload dir compile");
    let new_engine = Engine::builder()
        .register_type(type_with_reload_method(new_method))
        .build()
        .expect("new engine should build");
    let mut runtime = Runtime::from_hot_reload_version(new_engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    write_reward_module(&reward_file, 6);
    runtime
        .stage_hot_reload_update_changed_file(&root, &reward_file)
        .expect("runtime should be hot-reload enabled")
        .expect("changed-file method ABI rejection should be staged");
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged changed-file method ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, expected_code);
    assert_eq!(
        runtime.call(
            "game::main::main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(Value::Int(2))
    );
    report.errors[0].error.kind.clone()
}

fn type_with_reload_method(method: MethodDesc) -> TypeDesc {
    TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
        .host_type(HostTypeId::new(1))
        .method(method)
}

fn write_typed_reward_modules(
    root: &std::path::Path,
    main_return: &str,
    return_type: &str,
    reward_expr: &str,
) -> std::path::PathBuf {
    let game_dir = root.join("game");
    std::fs::create_dir_all(&game_dir).expect("create module dir");
    std::fs::write(
        game_dir.join("main.vela"),
        format!(
            r#"
use game::reward::grant

fn main() {{
    {main_return}
}}
"#
        ),
    )
    .expect("write main module");
    let reward_file = game_dir.join("reward.vela");
    write_typed_reward_module(&reward_file, return_type, reward_expr);
    reward_file
}

fn write_typed_reward_module(path: &std::path::Path, return_type: &str, reward_expr: &str) {
    write_reward_module_with_signature(path, &format!("() -> {return_type}"), reward_expr);
}

fn write_reward_module_with_signature(path: &std::path::Path, signature: &str, reward_expr: &str) {
    std::fs::write(
        path,
        format!(
            r#"
pub fn grant{signature} {{
    return {reward_expr};
}}
"#
        ),
    )
    .expect("write reward module with signature");
}

fn write_reward_module_with_helper(path: &std::path::Path, reward: i64) {
    std::fs::write(
        path,
        format!(
            r#"
pub fn grant() {{
    return {reward};
}}

fn helper() {{
    return 1;
}}
"#
        ),
    )
    .expect("write reward module with helper");
}
