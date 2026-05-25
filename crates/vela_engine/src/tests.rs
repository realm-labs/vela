use std::collections::BTreeMap;
use std::sync::Arc;
use vela_bytecode::compiler::{compile_program_source, compile_program_source_with_options};
use vela_common::{FieldId, HostMethodId, HostObjectId, HostTypeId, SourceId, TypeId};
use vela_host::{HostPath, HostRef, HostValue, MockStateAdapter, PatchOp, PatchTx};
use vela_hot_reload::{HotReloadErrorKind, HotReloadPolicy, HotReloadRuntime};
use vela_reflect::{
    FieldAccess, FieldDesc, MethodAccess, MethodDesc, MethodEffectSet, ReflectPermission,
    ReflectPermissionSet, SchemaHash, TypeDesc, TypeKey,
};
use vela_vm::{ExecutionBudgetKind, VmError, VmResult};
use vela_vm::{HostExecution, Value, VmErrorKind};

use crate::{
    CONTEXT_TIME_PERMISSION, CONTROLLED_RANDOM_PERMISSION, CTX_NOW_FUNCTION_ID,
    CTX_TICK_FUNCTION_ID, CallOptions, EffectSet, Engine, EngineErrorKind, EngineSourceErrorKind,
    FunctionAccess, MATH_RANDOM_FUNCTION_ID, NativeCallContext, NativeFunctionDesc,
    NativeFunctionId, NativeMethodDesc, ScriptArgsExt, TypeHint,
};
use crate::{FromScriptArg, IntoScriptArg};

#[test]
fn engine_installs_registered_native_functions_into_vm() {
    let engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game.add", NativeFunctionId::new(1))
                .param("lhs", TypeHint::Int)
                .param("rhs", TypeHint::Int)
                .returns(TypeHint::Int)
                .effects(EffectSet::pure())
                .access(FunctionAccess::public())
                .docs("Adds two integers."),
            |args| {
                let [Value::Int(lhs), Value::Int(rhs)] = args else {
                    return Ok(Value::Null);
                };
                Ok(Value::Int(lhs + rhs))
            },
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    return game.add(2, 3);
}
"#,
    )
    .expect("program should compile");

    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(Value::Int(5))
    );
}

#[test]
fn engine_registers_typed_native_functions() {
    let engine = Engine::builder()
        .register_typed_native_fn::<(i64, i64), _>(
            NativeFunctionDesc::new("game.add", NativeFunctionId::new(101))
                .param("lhs", TypeHint::Int)
                .param("rhs", TypeHint::Int)
                .returns(TypeHint::Int),
            |lhs: i64, rhs: i64| lhs + rhs,
        )
        .register_typed_native_fn::<(), _>(
            NativeFunctionDesc::new("game.label", NativeFunctionId::new(102))
                .returns(TypeHint::String),
            || "typed",
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    return game.add(2, 3) + game.label().len();
}
"#,
    )
    .expect("program should compile");

    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(Value::Int(10)),
    );
}

#[test]
fn typed_native_functions_accept_four_script_args() {
    let engine = Engine::builder()
        .register_typed_native_fn::<(i64, i64, i64, i64), _>(
            NativeFunctionDesc::new("game.sum4", NativeFunctionId::new(221))
                .param("a", TypeHint::Int)
                .param("b", TypeHint::Int)
                .param("c", TypeHint::Int)
                .param("d", TypeHint::Int)
                .returns(TypeHint::Int),
            |a: i64, b: i64, c: i64, d: i64| a + b + c + d,
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    return game.sum4(1, 2, 3, 4);
}
"#,
    )
    .expect("program should compile");

    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(Value::Int(10)),
    );
}

#[test]
fn typed_native_functions_accept_optional_values() {
    let engine = Engine::builder()
        .register_typed_native_fn::<(Option<i64>,), _>(
            NativeFunctionDesc::new("game.option_bonus", NativeFunctionId::new(108))
                .param("bonus", TypeHint::Any)
                .returns(TypeHint::Int),
            |bonus: Option<i64>| bonus.unwrap_or(7),
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    return game.option_bonus(null) + game.option_bonus(5);
}
"#,
    )
    .expect("program should compile");

    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(Value::Int(12)),
    );
}

#[test]
fn typed_native_functions_return_dynamic_result_values() {
    let engine = Engine::builder()
        .register_typed_native_fn::<(bool,), _>(
            NativeFunctionDesc::new("game.checked_bonus", NativeFunctionId::new(109))
                .param("ok", TypeHint::Bool)
                .returns(TypeHint::Any),
            |ok: bool| -> std::result::Result<i64, String> {
                if ok { Ok(11) } else { Err("denied".to_owned()) }
            },
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    return game.checked_bonus(false);
}
"#,
    )
    .expect("program should compile");

    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(Value::Enum {
            enum_name: "Result".to_owned(),
            variant: "Err".to_owned(),
            fields: [("0".to_owned(), Value::String("denied".to_owned()))].into(),
        }),
    );
}

#[test]
fn typed_native_functions_report_arity_and_type_errors() {
    let engine = Engine::builder()
        .register_typed_native_fn::<(i64, i64), _>(
            NativeFunctionDesc::new("game.add", NativeFunctionId::new(103)),
            |lhs: i64, rhs: i64| lhs + rhs,
        )
        .build()
        .expect("engine should build");
    let function = engine
        .native_function_by_name("game.add")
        .expect("typed native should be registered");

    assert!(matches!(
        (function.function)(&[Value::Int(1)]),
        Err(VmError {
            kind: VmErrorKind::ArityMismatch {
                expected: 2,
                actual: 1,
                ..
            },
            ..
        })
    ));
    assert!(matches!(
        (function.function)(&[Value::String("x".to_owned()), Value::Int(1)]),
        Err(VmError {
            kind: VmErrorKind::TypeMismatch { operation: "int" },
            ..
        })
    ));
}

#[test]
fn script_arg_conversions_support_optional_values() {
    assert_eq!(Option::<i64>::from_script_arg(&Value::Null), Ok(None));
    assert_eq!(Option::<i64>::from_script_arg(&Value::Int(3)), Ok(Some(3)));
    assert_eq!(
        Some("reward").into_script_arg(),
        Value::String("reward".to_owned())
    );
    assert_eq!(Option::<i64>::None.into_script_arg(), Value::Null);
    assert_eq!(
        crate::args![Some(2_i64), Option::<i64>::None],
        vec![Value::Int(2), Value::Null],
    );
    assert!(matches!(
        Option::<i64>::from_script_arg(&Value::String("bad".to_owned())),
        Err(VmError {
            kind: VmErrorKind::TypeMismatch { operation: "int" },
            ..
        })
    ));
}

#[test]
fn script_arg_conversions_support_result_values() {
    let ok_value = std::result::Result::<i64, String>::Ok(4).into_script_arg();
    let err_value = std::result::Result::<i64, String>::Err("bad".to_owned()).into_script_arg();

    assert_eq!(
        std::result::Result::<i64, String>::from_script_arg(&ok_value),
        Ok(Ok(4)),
    );
    assert_eq!(
        std::result::Result::<i64, String>::from_script_arg(&err_value),
        Ok(Err("bad".to_owned())),
    );
    assert_eq!(
        crate::args![std::result::Result::<i64, String>::Err(
            "missing".to_owned()
        )],
        vec![Value::Enum {
            enum_name: "Result".to_owned(),
            variant: "Err".to_owned(),
            fields: [("0".to_owned(), Value::String("missing".to_owned()))].into(),
        }],
    );
    assert!(matches!(
        std::result::Result::<i64, String>::from_script_arg(&Value::Enum {
            enum_name: "Result".to_owned(),
            variant: "Ok".to_owned(),
            fields: [("0".to_owned(), Value::String("bad".to_owned()))].into(),
        }),
        Err(VmError {
            kind: VmErrorKind::TypeMismatch { operation: "int" },
            ..
        })
    ));
    assert!(matches!(
        std::result::Result::<i64, String>::from_script_arg(&Value::Enum {
            enum_name: "Result".to_owned(),
            variant: "Unknown".to_owned(),
            fields: [("0".to_owned(), Value::Int(1))].into(),
        }),
        Err(VmError {
            kind: VmErrorKind::TypeMismatch {
                operation: "result",
            },
            ..
        })
    ));
}

#[test]
fn engine_registers_typed_host_native_functions() {
    let engine = Engine::builder()
        .grant_permission("player.write")
        .register_typed_host_native_fn::<(HostRef, i64), _>(
            NativeFunctionDesc::new("game.typed_host_set_level", NativeFunctionId::new(106))
                .param(
                    "player",
                    TypeHint::Host(TypeKey::new(TypeId::new(1), "Player")),
                )
                .param("level", TypeHint::Int)
                .returns(TypeHint::Null)
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public().require_permission("player.write")),
            typed_host_set_level,
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    game.typed_host_set_level(player, 19);
    return 1;
}
"#,
    )
    .expect("program should compile");
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
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
            &mut host,
        ),
        Ok(Value::Int(1)),
    );
    assert_eq!(
        tx.patches()[0].path,
        HostPath::new(host_ref).field(FieldId::new(1)),
    );
    assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Int(19)));
}

#[test]
fn typed_host_native_conversion_errors_before_patch() {
    let engine = Engine::builder()
        .grant_permission("player.write")
        .register_typed_host_native_fn::<(HostRef, i64), _>(
            NativeFunctionDesc::new("game.typed_host_set_level", NativeFunctionId::new(107))
                .access(FunctionAccess::public().require_permission("player.write")),
            typed_host_set_level,
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    game.typed_host_set_level("not a host", 19);
    return 1;
}
"#,
    )
    .expect("program should compile");
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert!(matches!(
        engine
            .into_vm()
            .run_program_with_host(&program, "main", &[], &mut host),
        Err(VmError {
            kind: VmErrorKind::TypeMismatch {
                operation: "host ref",
            },
            ..
        })
    ));
    assert!(tx.patches().is_empty());
}

fn typed_host_set_level(host: &mut HostExecution<'_>, player: HostRef, level: i64) -> VmResult<()> {
    host.tx.set_path(
        HostPath::new(player).field(FieldId::new(1)),
        HostValue::Int(level),
        None,
    )?;
    Ok(())
}

#[test]
fn engine_registers_four_arg_typed_host_native_functions() {
    let engine = Engine::builder()
        .grant_permission("player.write")
        .register_typed_host_native_fn::<(HostRef, i64, i64, i64), _>(
            NativeFunctionDesc::new("game.typed_host_sum_level", NativeFunctionId::new(222))
                .param(
                    "player",
                    TypeHint::Host(TypeKey::new(TypeId::new(1), "Player")),
                )
                .param("a", TypeHint::Int)
                .param("b", TypeHint::Int)
                .param("c", TypeHint::Int)
                .returns(TypeHint::Int)
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public().require_permission("player.write")),
            typed_host_sum_level,
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    return game.typed_host_sum_level(player, 2, 3, 4);
}
"#,
    )
    .expect("program should compile");
    let player = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine.into_vm().run_program_with_host(
            &program,
            "main",
            &[Value::HostRef(player)],
            &mut host
        ),
        Ok(Value::Int(9)),
    );
    assert_eq!(
        tx.patches()[0].path,
        HostPath::new(player).field(FieldId::new(1)),
    );
    assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Int(9)));
}

fn typed_host_sum_level(
    host: &mut HostExecution<'_>,
    player: HostRef,
    a: i64,
    b: i64,
    c: i64,
) -> VmResult<i64> {
    let level = a + b + c;
    host.tx.set_path(
        HostPath::new(player).field(FieldId::new(1)),
        HostValue::Int(level),
        None,
    )?;
    Ok(level)
}

#[test]
fn engine_registers_typed_context_host_native_functions() {
    let engine = Engine::builder()
        .grant_permission("player.write")
        .register_typed_context_host_native_fn::<(HostRef, i64), _>(
            NativeFunctionDesc::new("game.typed_set_level", NativeFunctionId::new(104))
                .param(
                    "player",
                    TypeHint::Host(TypeKey::new(TypeId::new(1), "Player")),
                )
                .param("level", TypeHint::Int)
                .returns(TypeHint::Bool)
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public().require_permission("player.write")),
            typed_set_level,
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    return game.typed_set_level(player, 17);
}
"#,
    )
    .expect("program should compile");
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
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
            &mut host,
        ),
        Ok(Value::Bool(true)),
    );
    assert_eq!(
        tx.patches()[0].path,
        HostPath::new(host_ref).field(FieldId::new(1)),
    );
    assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Int(17)));
}

#[test]
fn typed_context_host_native_conversion_errors_before_patch() {
    let engine = Engine::builder()
        .grant_permission("player.write")
        .register_typed_context_host_native_fn::<(HostRef, i64), _>(
            NativeFunctionDesc::new("game.typed_set_level", NativeFunctionId::new(105))
                .access(FunctionAccess::public().require_permission("player.write")),
            typed_set_level,
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    game.typed_set_level("not a host", 17);
    return 1;
}
"#,
    )
    .expect("program should compile");
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert!(matches!(
        engine
            .into_vm()
            .run_program_with_host(&program, "main", &[], &mut host),
        Err(VmError {
            kind: VmErrorKind::TypeMismatch {
                operation: "host ref",
            },
            ..
        })
    ));
    assert!(tx.patches().is_empty());
}

fn typed_set_level(
    ctx: &mut NativeCallContext<'_, '_>,
    player: HostRef,
    level: i64,
) -> VmResult<bool> {
    ctx.charge_instructions(10)?;
    let has_permission = ctx.has_permission("player.write");
    ctx.tx().set_path(
        HostPath::new(player).field(FieldId::new(1)),
        HostValue::Int(level),
        None,
    )?;
    Ok(has_permission)
}

#[test]
fn engine_registers_four_arg_typed_context_host_native_functions() {
    let engine = Engine::builder()
        .grant_permission("player.write")
        .register_typed_context_host_native_fn::<(HostRef, i64, i64, i64), _>(
            NativeFunctionDesc::new("game.typed_context_sum_level", NativeFunctionId::new(223))
                .param(
                    "player",
                    TypeHint::Host(TypeKey::new(TypeId::new(1), "Player")),
                )
                .param("a", TypeHint::Int)
                .param("b", TypeHint::Int)
                .param("c", TypeHint::Int)
                .returns(TypeHint::Int)
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public().require_permission("player.write")),
            typed_context_sum_level,
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    return game.typed_context_sum_level(player, 5, 6, 7);
}
"#,
    )
    .expect("program should compile");
    let player = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine.into_vm().run_program_with_host(
            &program,
            "main",
            &[Value::HostRef(player)],
            &mut host
        ),
        Ok(Value::Int(18)),
    );
    assert_eq!(
        tx.patches()[0].path,
        HostPath::new(player).field(FieldId::new(1)),
    );
    assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Int(18)));
}

fn typed_context_sum_level(
    ctx: &mut NativeCallContext<'_, '_>,
    player: HostRef,
    a: i64,
    b: i64,
    c: i64,
) -> VmResult<i64> {
    ctx.charge_instructions(1)?;
    let level = a + b + c;
    ctx.tx().set_path(
        HostPath::new(player).field(FieldId::new(1)),
        HostValue::Int(level),
        None,
    )?;
    Ok(level)
}

#[test]
fn engine_registers_native_function_reflection_metadata() {
    let engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game.add", NativeFunctionId::new(21))
                .param("lhs", TypeHint::Int)
                .param("rhs", TypeHint::Int)
                .returns(TypeHint::Int)
                .effects(EffectSet::host_read())
                .access(
                    FunctionAccess::public()
                        .reflect_callable(true)
                        .require_permission("game.add"),
                )
                .docs("Adds two integers."),
            |_| Ok(Value::Int(0)),
        )
        .build()
        .expect("engine should build");

    let registry = engine.registry();
    let module = registry
        .module_by_name("game")
        .expect("native module metadata");
    assert_eq!(module.exports.len(), 1);
    assert_eq!(module.exports[0].name, "game.add");

    let function = registry
        .function_by_name("game.add")
        .expect("native function metadata");
    assert_eq!(function.name, "game.add");
    assert_eq!(function.module.as_deref(), Some("game"));
    assert!(function.public);
    assert_eq!(function.params.len(), 2);
    assert_eq!(function.params[0].name, "lhs");
    assert_eq!(function.params[0].type_hint.as_deref(), Some("int"));
    assert_eq!(function.params[1].name, "rhs");
    assert_eq!(function.params[1].type_hint.as_deref(), Some("int"));
    assert_eq!(function.return_type.as_deref(), Some("int"));
    assert!(function.effects.reads_host);
    assert!(!function.effects.writes_host);
    assert!(function.access.reflect_visible);
    assert_eq!(
        function.access.required_permissions(),
        &["game.add".to_owned()]
    );
    assert_eq!(function.docs.as_deref(), Some("Adds two integers."));
}

#[test]
fn engine_compile_file_uses_engine_compiler_options() {
    let root = unique_test_dir("compile_file");
    std::fs::create_dir_all(&root).expect("create temp source dir");
    let source = root.join("main.lang");
    std::fs::write(
        &source,
        r#"
fn main(player: Player) {
    player.grant_exp(7);
    return 1;
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
        Ok(Value::Int(1))
    );
    assert_eq!(
        tx.patches()[0].op,
        PatchOp::CallHostMethod {
            method,
            args: vec![HostValue::Int(7)]
        }
    );
    std::fs::remove_dir_all(root).expect("clean temp source dir");
}

#[test]
fn engine_compile_dir_loads_lang_modules_deterministically() {
    let root = unique_test_dir("compile_dir");
    let game_dir = root.join("game");
    std::fs::create_dir_all(&game_dir).expect("create module dir");
    std::fs::write(
        game_dir.join("main.lang"),
        r#"
use game.reward.grant

fn main() {
    return grant() + game.config.BONUS;
}
"#,
    )
    .expect("write main module");
    std::fs::write(
        game_dir.join("reward.lang"),
        r#"
pub fn grant() {
    return 4;
}
"#,
    )
    .expect("write reward module");
    std::fs::write(
        game_dir.join("config.lang"),
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
fn engine_compile_file_reports_io_errors() {
    let root = unique_test_dir("missing_file");
    let path = root.join("missing.lang");
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

#[test]
fn engine_installs_registered_host_native_functions_into_vm() {
    let engine = Engine::builder()
        .grant_permission("player.write")
        .register_host_native_fn(
            NativeFunctionDesc::new("game.set_level", NativeFunctionId::new(2))
                .param(
                    "player",
                    TypeHint::Host(TypeKey::new(TypeId::new(1), "Player")),
                )
                .param("level", TypeHint::Int)
                .returns(TypeHint::Null)
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public().require_permission("player.write")),
            |args, host| {
                let [Value::HostRef(player), Value::Int(level)] = args else {
                    return Ok(Value::Null);
                };
                host.tx.set_path(
                    HostPath::new(*player).field(FieldId::new(1)),
                    HostValue::Int(*level),
                    None,
                )?;
                Ok(Value::Null)
            },
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    game.set_level(player, 9);
    return 1;
}
"#,
    )
    .expect("program should compile");
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
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
        Ok(Value::Int(1))
    );
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(
        tx.patches()[0].path,
        HostPath::new(host_ref).field(FieldId::new(1))
    );
    assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Int(9)));
}

#[test]
fn engine_installs_context_host_native_functions_into_vm() {
    let engine = Engine::builder()
        .grant_permission("player.write")
        .register_context_host_native_fn(
            NativeFunctionDesc::new("game.context_set_level", NativeFunctionId::new(23))
                .param(
                    "player",
                    TypeHint::Host(TypeKey::new(TypeId::new(1), "Player")),
                )
                .param("level", TypeHint::Int)
                .returns(TypeHint::Bool)
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public().require_permission("player.write")),
            |args, ctx| {
                let [Value::HostRef(player), Value::Int(level)] = args else {
                    return Ok(Value::Bool(false));
                };
                assert!(ctx.has_permission("player.write"));
                assert!(
                    ctx.engine()
                        .native_function_by_name("game.context_set_level")
                        .is_none()
                );
                assert!(
                    ctx.engine()
                        .context_host_native_function_by_name("game.context_set_level")
                        .is_some()
                );
                ctx.tx().set_path(
                    HostPath::new(*player).field(FieldId::new(1)),
                    HostValue::Int(*level),
                    None,
                )?;
                Ok(Value::Bool(true))
            },
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    return game.context_set_level(player, 11);
}
"#,
    )
    .expect("program should compile");
    let registry = engine.registry();
    let function = registry
        .function_by_name("game.context_set_level")
        .expect("context host native metadata");
    assert_eq!(function.id, NativeFunctionId::new(23));
    assert!(function.effects.writes_host);
    assert_eq!(
        function.access.required_permissions(),
        &["player.write".to_owned()]
    );
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
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
        Ok(Value::Bool(true))
    );
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(
        tx.patches()[0].path,
        HostPath::new(host_ref).field(FieldId::new(1))
    );
    assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Int(11)));
}

#[test]
fn context_host_native_can_charge_execution_budget_before_patching() {
    let engine = Engine::builder()
        .register_context_host_native_fn(
            NativeFunctionDesc::new("game.expensive_set_level", NativeFunctionId::new(24))
                .param(
                    "player",
                    TypeHint::Host(TypeKey::new(TypeId::new(1), "Player")),
                )
                .param("level", TypeHint::Int)
                .returns(TypeHint::Null)
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public()),
            |args, ctx| {
                ctx.charge_instructions(100)?;
                let player = args.required::<HostRef>(0)?;
                let level = args.required::<i64>(1)?;
                ctx.tx().set_path(
                    HostPath::new(player).field(FieldId::new(1)),
                    HostValue::Int(level),
                    None,
                )?;
                Ok(Value::Null)
            },
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    game.expensive_set_level(player, 13);
    return 1;
}
"#,
    )
    .expect("program should compile");
    let mut runtime = crate::Runtime::new(engine, program);
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    let error = runtime
        .call(
            "main",
            &[Value::HostRef(host_ref)],
            CallOptions::new(50, usize::MAX, usize::MAX, usize::MAX),
            &mut adapter,
            &mut tx,
        )
        .expect_err("native budget charge should fail");

    assert_eq!(
        error.kind,
        VmErrorKind::BudgetExceeded {
            budget: ExecutionBudgetKind::Instructions,
            limit: 50
        }
    );
    assert!(tx.patches().is_empty());
}

#[test]
fn args_macro_converts_rust_values_and_host_refs() {
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 7);
    let mut map = BTreeMap::new();
    map.insert("key", 9);

    let args = crate::args![(), true, 5, 2.5_f64, "title", ["a", "b"], map, host_ref,];

    assert_eq!(
        args,
        vec![
            Value::Null,
            Value::Bool(true),
            Value::Int(5),
            Value::Float(2.5),
            Value::String("title".to_owned()),
            Value::Array(vec![
                Value::String("a".to_owned()),
                Value::String("b".to_owned())
            ]),
            Value::Map([("key".to_owned(), Value::Int(9))].into()),
            Value::HostRef(host_ref),
        ]
    );
    assert_eq!(crate::args!(), Vec::<Value>::new());
    assert_eq!(crate::host!(1, 42, 7), Value::HostRef(host_ref));
}

#[test]
fn script_arg_conversions_extract_owned_rust_values() {
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 7);
    let args = crate::args![
        true,
        5,
        2.5_f64,
        "title",
        [1, 2, 3],
        BTreeMap::from([("key", "value")]),
        host_ref,
    ];

    assert!(args.required::<bool>(0).expect("bool arg"));
    assert_eq!(args.required::<i64>(1), Ok(5));
    assert_eq!(args.required::<f64>(2), Ok(2.5));
    assert_eq!(args.required::<String>(3), Ok("title".to_owned()));
    assert_eq!(args.required::<Vec<i64>>(4), Ok(vec![1, 2, 3]));
    assert_eq!(
        args.required::<BTreeMap<String, String>>(5),
        Ok(BTreeMap::from([("key".to_owned(), "value".to_owned())]))
    );
    assert_eq!(args.required::<HostRef>(6), Ok(host_ref));

    assert!(matches!(
        args.required::<HostRef>(1),
        Err(VmError {
            kind: VmErrorKind::TypeMismatch {
                operation: "host ref"
            },
            source_span: None,
            ..
        })
    ));
    assert!(matches!(
        args.required::<i64>(9),
        Err(VmError {
            kind: VmErrorKind::ArityMismatch {
                name,
                expected: 10,
                actual: 7,
            },
            source_span: None,
            ..
        }) if name == "native argument conversion"
    ));
}

#[test]
fn runtime_call_accepts_args_and_host_macros() {
    let method = HostMethodId::new(23);
    let engine = Engine::builder()
        .register_type(
            player_type(TypeId::new(1), HostTypeId::new(1))
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
    let mut runtime = crate::Runtime::new(engine, program);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let args = crate::args![crate::host!(1, 42, 1), 12];

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
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(
        tx.patches()[0].op,
        PatchOp::CallHostMethod {
            method,
            args: vec![HostValue::Int(12)]
        }
    );
}

#[test]
fn runtime_call_records_host_patches_without_applying() {
    let method = HostMethodId::new(23);
    let engine = Engine::builder()
        .register_type(
            player_type(TypeId::new(1), HostTypeId::new(1))
                .method(MethodDesc::new(method, "grant_exp")),
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    player.grant_exp(12);
    return "done";
}
"#,
        &engine.compiler_options(),
    )
    .expect("program should compile");
    let mut runtime = crate::Runtime::new(engine, program);
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    let result = runtime
        .call(
            "main",
            &[Value::HostRef(host_ref)],
            CallOptions::gameplay(),
            &mut adapter,
            &mut tx,
        )
        .expect("runtime call should run");

    assert_eq!(result, Value::String("done".to_owned()));
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(
        tx.patches()[0].op,
        PatchOp::CallHostMethod {
            method,
            args: vec![HostValue::Int(12)]
        }
    );
    assert!(
        adapter.method_calls().is_empty(),
        "runtime call must not apply patches"
    );
}

#[test]
fn runtime_call_enforces_call_options_budget() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    let total = 0;
    for value in 1..=100 {
        total += value;
    }
    return total;
}
"#,
    )
    .expect("program should compile");
    let mut runtime = crate::Runtime::new(engine, program);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    let error = runtime
        .call(
            "main",
            &[],
            CallOptions::new(4, usize::MAX, usize::MAX, usize::MAX),
            &mut adapter,
            &mut tx,
        )
        .expect_err("runtime call should exhaust instruction budget");

    assert_eq!(
        error,
        VmError {
            kind: VmErrorKind::BudgetExceeded {
                budget: ExecutionBudgetKind::Instructions,
                limit: 4
            },
            source_span: None,
            call_stack: Arc::from([vela_vm::VmStackFrame {
                function: "main".to_owned(),
                call_site: None,
            }]),
        }
    );
    assert!(tx.patches().is_empty());
}

#[test]
fn engine_denies_native_calls_missing_required_permission() {
    let engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game.secret", NativeFunctionId::new(3))
                .returns(TypeHint::Int)
                .access(FunctionAccess::public().require_permission("game.secret")),
            |_| Ok(Value::Int(99)),
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    return game.secret();
}
"#,
    )
    .expect("program should compile");

    assert!(matches!(
        engine.into_vm().run_program(&program, "main", &[]),
        Err(error) if error.kind == VmErrorKind::PermissionDenied {
            native: "game.secret".to_owned(),
            permission: "game.secret".to_owned(),
        }
    ));
}

#[test]
fn engine_controlled_random_requires_permission() {
    let engine = Engine::builder()
        .with_controlled_random(7)
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    return math.random(1, 6);
}
"#,
    )
    .expect("program should compile");

    assert!(matches!(
        engine.into_vm().run_program(&program, "main", &[]),
        Err(error) if error.kind == VmErrorKind::PermissionDenied {
            native: "math.random".to_owned(),
            permission: CONTROLLED_RANDOM_PERMISSION.to_owned(),
        }
    ));
}

#[test]
fn engine_controlled_random_is_seeded_and_bounded() {
    let source = r#"
fn main() {
    let first = math.random(1, 6);
    let second = math.random(10, 12);
    if first >= 1 && first <= 6 && second >= 10 && second <= 12 {
        return first * 100 + second;
    }
    return 0;
}
"#;
    let program = compile_program_source(SourceId::new(1), source).expect("program should compile");
    let first_engine = Engine::builder()
        .grant_permission(CONTROLLED_RANDOM_PERMISSION)
        .with_controlled_random(42)
        .build()
        .expect("first engine should build");
    let second_engine = Engine::builder()
        .grant_permission(CONTROLLED_RANDOM_PERMISSION)
        .with_controlled_random(42)
        .build()
        .expect("second engine should build");

    let first = first_engine
        .into_vm()
        .run_program(&program, "main", &[])
        .expect("first random run should succeed");
    let second = second_engine
        .into_vm()
        .run_program(&program, "main", &[])
        .expect("second random run should succeed");

    assert_eq!(first, second);
    assert_ne!(first, Value::Int(0));
}

#[test]
fn engine_controlled_random_registers_metadata() {
    let engine = Engine::builder()
        .with_controlled_random(1)
        .build()
        .expect("engine should build");

    let registry = engine.registry();
    let function = registry
        .function_by_name("math.random")
        .expect("math.random metadata");
    assert_eq!(function.id, MATH_RANDOM_FUNCTION_ID);
    assert_eq!(function.module.as_deref(), Some("math"));
    assert_eq!(function.params.len(), 2);
    assert_eq!(function.params[0].type_hint.as_deref(), Some("int"));
    assert_eq!(function.params[1].type_hint.as_deref(), Some("int"));
    assert_eq!(function.return_type.as_deref(), Some("int"));
    assert_eq!(
        function.access.required_permissions(),
        &[CONTROLLED_RANDOM_PERMISSION.to_owned()]
    );
}

#[test]
fn engine_context_clock_requires_permission() {
    let engine = Engine::builder()
        .with_context_clock(1_700_000_000, 42)
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    return ctx.now();
}
"#,
    )
    .expect("program should compile");

    assert!(matches!(
        engine.into_vm().run_program(&program, "main", &[]),
        Err(error) if error.kind == VmErrorKind::PermissionDenied {
            native: "ctx.now".to_owned(),
            permission: CONTEXT_TIME_PERMISSION.to_owned(),
        }
    ));
}

#[test]
fn engine_context_clock_returns_configured_values() {
    let engine = Engine::builder()
        .grant_permission(CONTEXT_TIME_PERMISSION)
        .with_context_clock(1_700_000_000, 42)
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    return ctx.now() + ctx.tick();
}
"#,
    )
    .expect("program should compile");

    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(Value::Int(1_700_000_042))
    );
}

#[test]
fn engine_context_clock_registers_metadata() {
    let engine = Engine::builder()
        .with_context_clock(1, 2)
        .build()
        .expect("engine should build");

    let registry = engine.registry();
    let now = registry
        .function_by_name("ctx.now")
        .expect("ctx.now metadata");
    let tick = registry
        .function_by_name("ctx.tick")
        .expect("ctx.tick metadata");

    assert_eq!(now.id, CTX_NOW_FUNCTION_ID);
    assert_eq!(now.module.as_deref(), Some("ctx"));
    assert!(now.params.is_empty());
    assert_eq!(now.return_type.as_deref(), Some("int"));
    assert_eq!(
        now.access.required_permissions(),
        &[CONTEXT_TIME_PERMISSION.to_owned()]
    );
    assert_eq!(tick.id, CTX_TICK_FUNCTION_ID);
    assert_eq!(tick.module.as_deref(), Some("ctx"));
    assert!(tick.params.is_empty());
    assert_eq!(tick.return_type.as_deref(), Some("int"));
    assert_eq!(
        tick.access.required_permissions(),
        &[CONTEXT_TIME_PERMISSION.to_owned()]
    );
}

#[test]
fn engine_denies_host_native_before_recording_patches() {
    let engine = Engine::builder()
        .register_host_native_fn(
            NativeFunctionDesc::new("game.set_level", NativeFunctionId::new(4))
                .param(
                    "player",
                    TypeHint::Host(TypeKey::new(TypeId::new(1), "Player")),
                )
                .param("level", TypeHint::Int)
                .returns(TypeHint::Null)
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public().require_permission("player.write")),
            |args, host| {
                let [Value::HostRef(player), Value::Int(level)] = args else {
                    return Ok(Value::Null);
                };
                host.tx.set_path(
                    HostPath::new(*player).field(FieldId::new(1)),
                    HostValue::Int(*level),
                    None,
                )?;
                Ok(Value::Null)
            },
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    game.set_level(player, 9);
    return 1;
}
"#,
    )
    .expect("program should compile");
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert!(matches!(
        engine
            .into_vm()
            .run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host),
        Err(error) if error.kind == VmErrorKind::PermissionDenied {
            native: "game.set_level".to_owned(),
            permission: "player.write".to_owned(),
        }
    ));
    assert!(tx.patches().is_empty());
}

#[test]
fn engine_installs_permissioned_reflection_natives() {
    let engine = Engine::builder()
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .reflection_permissions(
            ReflectPermissionSet::read_only().with(ReflectPermission::InspectHostPath),
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    if reflect.name(player) == "Player" && reflect.get(player, "level") == 7 {
        reflect.set(player, "level", 8);
    }
    return 0;
}
"#,
    )
    .expect("program should compile");
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    adapter.insert_value(
        HostPath::new(host_ref).field(FieldId::new(1)),
        HostValue::Int(7),
    );
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert!(matches!(
        engine
            .into_vm()
            .run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host),
        Err(error) if error.kind == VmErrorKind::Reflect(vela_reflect::ReflectErrorKind::PermissionDenied {
            permission: ReflectPermission::WriteValueFields
        })
    ));
    assert!(tx.patches().is_empty());
}

#[test]
fn engine_granted_permissions_unlock_reflection_metadata_lists() {
    let engine = Engine::builder()
        .register_type(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
                .host_type(HostTypeId::new(1))
                .field(
                    FieldDesc::new(FieldId::new(1), "secret_level")
                        .access(FieldAccess::new().require_permission("player.inspect")),
                ),
        )
        .register_native_fn(
            NativeFunctionDesc::new("game.secret_bonus", NativeFunctionId::new(77))
                .returns(TypeHint::Int)
                .access(
                    FunctionAccess::public()
                        .reflect_callable(true)
                        .require_permission("game.inspect"),
                ),
            |_| Ok(Value::Int(5)),
        )
        .grant_permission("player.inspect")
        .grant_permission("game.inspect")
        .reflection_permissions(ReflectPermissionSet::new().with(ReflectPermission::ReadTypeInfo))
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    let fields = reflect.fields();
    let functions = reflect.functions();
    if fields.len() == 1
        && fields[0].owner == "Player"
        && fields[0].name == "secret_level"
        && functions.len() == 1
        && functions[0].name == "game.secret_bonus" {
        return 1;
    }
    return 0;
}
"#,
    )
    .expect("program should compile");
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine
            .into_vm()
            .run_program_with_host(&program, "main", &[], &mut host),
        Ok(Value::Int(1))
    );
    assert!(tx.patches().is_empty());
}

#[test]
fn engine_missing_permissions_hide_reflection_metadata_lists() {
    let engine = Engine::builder()
        .register_type(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
                .host_type(HostTypeId::new(1))
                .field(
                    FieldDesc::new(FieldId::new(1), "secret_level")
                        .access(FieldAccess::new().require_permission("player.inspect")),
                ),
        )
        .register_native_fn(
            NativeFunctionDesc::new("game.secret_bonus", NativeFunctionId::new(77))
                .returns(TypeHint::Int)
                .access(
                    FunctionAccess::public()
                        .reflect_callable(true)
                        .require_permission("game.inspect"),
                ),
            |_| Ok(Value::Int(5)),
        )
        .reflection_permissions(ReflectPermissionSet::new().with(ReflectPermission::ReadTypeInfo))
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    return reflect.fields().len() + reflect.functions().len();
}
"#,
    )
    .expect("program should compile");
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine
            .into_vm()
            .run_program_with_host(&program, "main", &[], &mut host),
        Ok(Value::Int(0))
    );
    assert!(tx.patches().is_empty());
}

#[test]
fn engine_installs_reflection_lookup_budget() {
    let engine = Engine::builder()
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .reflection_lookup_budget(1)
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    reflect.name(player);
    reflect.kind(player);
    return 1;
}
"#,
    )
    .expect("program should compile");
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert!(matches!(
        engine
            .into_vm()
            .run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host),
        Err(error) if error.kind == VmErrorKind::Reflect(vela_reflect::ReflectErrorKind::LookupBudgetExceeded {
            limit: 1
        })
    ));
    assert!(tx.patches().is_empty());
}

#[test]
fn engine_reflect_call_denies_unapproved_native_methods() {
    let method = HostMethodId::new(6);
    let owner = TypeKey::new(TypeId::new(1), "Player");
    let engine = Engine::builder()
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .register_native_method_fn(
            NativeMethodDesc::new(owner, method, "grant_exp")
                .effects(EffectSet::host_write())
                .access(
                    FunctionAccess::public()
                        .reflect_callable(true)
                        .require_permission("player.grant_exp"),
                ),
            |_, _, _| Ok(Value::Null),
        )
        .reflection_permissions(ReflectPermissionSet::all())
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    reflect.call(player, "grant_exp", 10);
    return 1;
}
"#,
    )
    .expect("program should compile");
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert!(matches!(
        engine
            .into_vm()
            .run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host),
        Err(error) if error.kind == VmErrorKind::Reflect(vela_reflect::ReflectErrorKind::MethodPermissionDenied {
            method: "grant_exp".to_owned(),
            permission: "player.grant_exp".to_owned()
        })
    ));
    assert!(tx.patches().is_empty());
}

#[test]
fn engine_reflect_call_records_approved_native_methods() {
    let method = HostMethodId::new(6);
    let owner = TypeKey::new(TypeId::new(1), "Player");
    let engine = Engine::builder()
        .grant_permission("player.grant_exp")
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .register_native_method_fn(
            NativeMethodDesc::new(owner, method, "grant_exp")
                .effects(EffectSet::host_write())
                .access(
                    FunctionAccess::public()
                        .reflect_callable(true)
                        .require_permission("player.grant_exp"),
                ),
            |_, _, _| Ok(Value::Null),
        )
        .reflection_permissions(ReflectPermissionSet::all())
        .build()
        .expect("engine should build");
    let registry = engine.registry();
    let reflected_method = registry
        .type_by_name("Player")
        .and_then(|desc| {
            desc.methods
                .iter()
                .find(|method| method.name == "grant_exp")
        })
        .expect("reflected method");
    assert!(reflected_method.access.reflect_callable);
    assert!(reflected_method.effects.writes_host);
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    reflect.call(player, "grant_exp", 10);
    return 1;
}
"#,
    )
    .expect("program should compile");
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
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
        Ok(Value::Int(1))
    );
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(
        tx.patches()[0].op,
        PatchOp::CallHostMethod {
            method,
            args: vec![HostValue::Int(10)]
        }
    );
}

#[test]
fn engine_compiler_options_lower_registered_host_methods() {
    let method = HostMethodId::new(5);
    let engine = Engine::builder()
        .register_type(
            player_type(TypeId::new(1), HostTypeId::new(1))
                .method(MethodDesc::new(method, "grant_exp")),
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    player.grant_exp(10);
    return 1;
}
"#,
        &engine.compiler_options(),
    )
    .expect("program should compile");
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
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
        Ok(Value::Int(1))
    );
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(tx.patches()[0].path, HostPath::new(host_ref));
    assert_eq!(
        tx.patches()[0].op,
        PatchOp::CallHostMethod {
            method,
            args: vec![HostValue::Int(10)],
        }
    );
}

#[test]
fn engine_compiler_options_disambiguate_host_methods_by_receiver_type() {
    let player_method = HostMethodId::new(5);
    let monster_method = HostMethodId::new(6);
    let engine = Engine::builder()
        .register_type(
            player_type(TypeId::new(1), HostTypeId::new(1))
                .method(MethodDesc::new(player_method, "grant_exp")),
        )
        .register_type(
            TypeDesc::new(TypeKey::new(TypeId::new(2), "Monster"))
                .host_type(HostTypeId::new(2))
                .method(MethodDesc::new(monster_method, "grant_exp")),
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main(player: Player, monster: Monster) {
    player.grant_exp(10);
    monster.grant_exp(3);
    return 1;
}
"#,
        &engine.compiler_options(),
    )
    .expect("program should compile");
    let player = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let monster = HostRef::new(HostTypeId::new(2), HostObjectId::new(7), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine.into_vm().run_program_with_host(
            &program,
            "main",
            &[Value::HostRef(player), Value::HostRef(monster)],
            &mut host
        ),
        Ok(Value::Int(1))
    );
    assert_eq!(tx.patches().len(), 2);
    assert_eq!(
        tx.patches()[0].op,
        PatchOp::CallHostMethod {
            method: player_method,
            args: vec![HostValue::Int(10)],
        }
    );
    assert_eq!(
        tx.patches()[1].op,
        PatchOp::CallHostMethod {
            method: monster_method,
            args: vec![HostValue::Int(3)],
        }
    );
}

#[test]
fn engine_registers_callable_native_methods_for_host_paths() {
    let method = HostMethodId::new(6);
    let owner = TypeKey::new(TypeId::new(1), "Player");
    let engine = Engine::builder()
        .grant_permission("player.grant_exp")
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .register_native_method_fn(
            NativeMethodDesc::new(owner, method, "grant_exp")
                .param("amount", TypeHint::Int)
                .returns(TypeHint::Null)
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public().require_permission("player.grant_exp"))
                .docs("Grant player experience."),
            move |receiver, args, host| {
                let [Value::Int(amount)] = args else {
                    return Ok(Value::Null);
                };
                host.tx.call_method(
                    receiver.clone(),
                    method,
                    vec![HostValue::Int(*amount)],
                    None,
                )?;
                Ok(Value::Null)
            },
        )
        .build()
        .expect("engine should build");
    let registry = engine.registry();
    let reflected_method = registry
        .type_by_name("Player")
        .and_then(|desc| {
            desc.methods
                .iter()
                .find(|method| method.name == "grant_exp")
        })
        .expect("reflected native method metadata");
    assert_eq!(
        reflected_method.docs.as_deref(),
        Some("Grant player experience.")
    );
    assert_eq!(reflected_method.params.len(), 1);
    assert_eq!(reflected_method.params[0].name, "amount");
    assert_eq!(reflected_method.params[0].type_hint.as_deref(), Some("int"));
    assert_eq!(reflected_method.return_type.as_deref(), Some("null"));
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    player.grant_exp(10);
    return 1;
}
"#,
        &engine.compiler_options(),
    )
    .expect("program should compile");
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine.call_native_method(
            method,
            &HostPath::new(host_ref),
            &[Value::Int(10)],
            &mut host,
        ),
        Ok(Value::Null)
    );
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(
        tx.patches()[0].op,
        PatchOp::CallHostMethod {
            method,
            args: vec![HostValue::Int(10)],
        }
    );

    let mut adapter = MockStateAdapter::new();
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
        Ok(Value::Int(1))
    );
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(tx.patches()[0].path, HostPath::new(host_ref));
}

#[test]
fn engine_registers_typed_callable_native_methods_for_host_paths() {
    let method = HostMethodId::new(8);
    let owner = TypeKey::new(TypeId::new(1), "Player");
    let engine = Engine::builder()
        .grant_permission("player.grant_exp")
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .register_typed_native_method_fn::<(i64,), _>(
            NativeMethodDesc::new(owner, method, "typed_grant_exp")
                .param("amount", TypeHint::Int)
                .returns(TypeHint::Int)
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public().require_permission("player.grant_exp")),
            typed_grant_exp,
        )
        .build()
        .expect("engine should build");
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine.call_native_method(
            method,
            &HostPath::new(host_ref),
            &[Value::Int(15)],
            &mut host,
        ),
        Ok(Value::Int(15))
    );
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(
        tx.patches()[0].op,
        PatchOp::CallHostMethod {
            method,
            args: vec![HostValue::Int(15)],
        }
    );
}

#[test]
fn typed_callable_native_method_conversion_errors_before_patch() {
    let method = HostMethodId::new(8);
    let owner = TypeKey::new(TypeId::new(1), "Player");
    let engine = Engine::builder()
        .grant_permission("player.grant_exp")
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .register_typed_native_method_fn::<(i64,), _>(
            NativeMethodDesc::new(owner, method, "typed_grant_exp")
                .access(FunctionAccess::public().require_permission("player.grant_exp")),
            typed_grant_exp,
        )
        .build()
        .expect("engine should build");
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert!(matches!(
        engine.call_native_method(
            method,
            &HostPath::new(host_ref),
            &[Value::String("bad".to_owned())],
            &mut host,
        ),
        Err(VmError {
            kind: VmErrorKind::TypeMismatch { operation: "int" },
            ..
        })
    ));
    assert!(tx.patches().is_empty());
}

fn typed_grant_exp(
    receiver: &HostPath,
    host: &mut HostExecution<'_>,
    amount: i64,
) -> VmResult<Option<i64>> {
    host.tx.call_method(
        receiver.clone(),
        HostMethodId::new(8),
        vec![HostValue::Int(amount)],
        None,
    )?;
    Ok(Some(amount))
}

#[test]
fn engine_registers_four_arg_typed_callable_native_methods() {
    let method = HostMethodId::new(9);
    let owner = TypeKey::new(TypeId::new(1), "Player");
    let engine = Engine::builder()
        .grant_permission("player.sum")
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .register_typed_native_method_fn::<(i64, i64, i64, i64), _>(
            NativeMethodDesc::new(owner, method, "typed_sum4")
                .param("a", TypeHint::Int)
                .param("b", TypeHint::Int)
                .param("c", TypeHint::Int)
                .param("d", TypeHint::Int)
                .returns(TypeHint::Int)
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public().require_permission("player.sum")),
            typed_sum4,
        )
        .build()
        .expect("engine should build");
    let player = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine.call_native_method(
            method,
            &HostPath::new(player),
            &[Value::Int(1), Value::Int(2), Value::Int(3), Value::Int(4)],
            &mut host,
        ),
        Ok(Value::Int(10))
    );
    assert_eq!(
        tx.patches()[0].op,
        PatchOp::CallHostMethod {
            method,
            args: vec![HostValue::Int(10)],
        }
    );
}

fn typed_sum4(
    receiver: &HostPath,
    host: &mut HostExecution<'_>,
    a: i64,
    b: i64,
    c: i64,
    d: i64,
) -> VmResult<i64> {
    let total = a + b + c + d;
    host.tx.call_method(
        receiver.clone(),
        HostMethodId::new(9),
        vec![HostValue::Int(total)],
        None,
    )?;
    Ok(total)
}

#[test]
fn engine_rejects_native_methods_for_unknown_owner_types() {
    let result = Engine::builder()
        .register_native_method_fn(
            NativeMethodDesc::new(
                TypeKey::new(TypeId::new(99), "Missing"),
                HostMethodId::new(1),
                "grant_exp",
            ),
            |_, _, _| Ok(Value::Null),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::UnknownNativeMethodOwner {
            name: "Missing".to_owned()
        }
    ));
}

#[test]
fn engine_rejects_duplicate_native_function_ids() {
    let result = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game.first", NativeFunctionId::new(10)),
            |_| Ok(Value::Null),
        )
        .register_native_fn(
            NativeFunctionDesc::new("game.second", NativeFunctionId::new(10)),
            |_| Ok(Value::Null),
        )
        .build();

    assert!(matches!(
        result.map(|_| ()),
        Err(error) if error.kind == EngineErrorKind::DuplicateNativeFunctionId { id: 10 }
    ));
}

#[test]
fn engine_rejects_duplicate_names_across_host_and_pure_natives() {
    let result = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game.same", NativeFunctionId::new(10)),
            |_| Ok(Value::Null),
        )
        .register_host_native_fn(
            NativeFunctionDesc::new("game.same", NativeFunctionId::new(11)),
            |_, _| Ok(Value::Null),
        )
        .build();

    assert!(matches!(
        result.map(|_| ()),
        Err(error) if error.kind == EngineErrorKind::DuplicateNativeFunctionName {
            name: "game.same".to_owned()
        }
    ));
}

#[test]
fn engine_rejects_duplicate_context_host_native_ids() {
    let result = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game.first", NativeFunctionId::new(30)),
            |_| Ok(Value::Null),
        )
        .register_context_host_native_fn(
            NativeFunctionDesc::new("game.second", NativeFunctionId::new(30)),
            |_, _| Ok(Value::Null),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::DuplicateNativeFunctionId { id: 30 }
    ));
}

#[test]
fn engine_rejects_duplicate_type_names() {
    let result = Engine::builder()
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .register_type(player_type(TypeId::new(2), HostTypeId::new(2)))
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::DuplicateTypeName {
            name: "Player".to_owned()
        }
    ));
}

#[test]
fn engine_rejects_duplicate_host_method_names() {
    let result = Engine::builder()
        .register_type(
            player_type(TypeId::new(1), HostTypeId::new(1))
                .method(MethodDesc::new(HostMethodId::new(1), "grant_exp"))
                .method(MethodDesc::new(HostMethodId::new(2), "grant_exp")),
        )
        .build();

    assert!(matches!(
        result,
        Err(error) if error.kind == EngineErrorKind::DuplicateHostMethodName {
            name: "grant_exp".to_owned()
        }
    ));
}

#[test]
fn engine_installs_type_registry_for_host_ref_script_impl_dispatch() {
    let engine = Engine::builder()
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource { fn bonus(self, amount) -> int; }

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
        return amount + 7;
    }
}

fn main(player) {
    return player.bonus(5);
}
"#,
    )
    .expect("program should compile");
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
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
        Ok(Value::Int(12))
    );
    assert!(tx.patches().is_empty());
}

fn player_type(type_id: TypeId, host_type_id: HostTypeId) -> TypeDesc {
    TypeDesc::new(TypeKey::new(type_id, "Player"))
        .host_type(host_type_id)
        .field(FieldDesc::new(FieldId::new(1), "level").writable(true))
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
