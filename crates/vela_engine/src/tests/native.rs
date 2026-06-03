use std::sync::Arc;

use vela_bytecode::compiler::{compile_program_source, compile_program_source_with_options};
use vela_common::{FieldId, HostMethodId, HostObjectId, HostTypeId, SourceId, TypeId};
use vela_host::adapter::ScriptStateAdapter;
use vela_host::mock::MockStateAdapter;
use vela_host::patch::PatchOp;
use vela_host::path::{HostPath, HostRef};
use vela_host::tx::PatchTx;
use vela_host::value::HostValue;
use vela_reflect::registry::TypeKey;
use vela_vm::HostExecution;
use vela_vm::budget::ExecutionBudgetKind;
use vela_vm::error::{VmError, VmErrorKind};
use vela_vm::value::Value;

use crate::args::ScriptArgsExt;
use crate::engine::Engine;
use crate::native::{EffectSet, FunctionAccess, NativeFunctionDesc, NativeFunctionId, TypeHint};
use crate::runtime::{CallOptions, Runtime};

#[test]
fn engine_installs_registered_native_functions_into_vm() {
    let engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::add", NativeFunctionId::new(1))
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
    return game::add(2, 3);
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
fn engine_compiler_options_lower_named_registered_native_arguments() {
    let engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::subtract", NativeFunctionId::new(27))
                .param("lhs", TypeHint::Int)
                .param("rhs", TypeHint::Int)
                .returns(TypeHint::Int)
                .effects(EffectSet::pure())
                .access(FunctionAccess::public()),
            |args| {
                let [Value::Int(lhs), Value::Int(rhs)] = args else {
                    return Ok(Value::Null);
                };
                Ok(Value::Int(lhs - rhs))
            },
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main() {
    return game::subtract(rhs = 3, lhs = 10);
}
"#,
        &engine.compiler_options(),
    )
    .expect("named registered native arguments should compile");

    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(Value::Int(7))
    );
}

#[test]
fn engine_compiler_options_lower_named_standard_native_arguments() {
    let engine = Engine::builder()
        .with_standard_natives()
        .build()
        .expect("engine should build with standard natives");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main() {
    return math::clamp(max = 10, value = 15, min = 1);
}
"#,
        &engine.compiler_options(),
    )
    .expect("named stdlib native arguments should compile");

    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(Value::Int(10))
    );
}

#[test]
fn engine_compiler_options_lower_named_standard_value_method_arguments() {
    let engine = Engine::builder()
        .with_standard_natives()
        .build()
        .expect("engine should build with standard natives");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main() {
    let pair = "reward:gold".split_once(separator = ":").unwrap_or(["", ""]);
    return {"gold": 4}.get_or(default = 0, key = pair[1]);
}
"#,
        &engine.compiler_options(),
    )
    .expect("named stdlib value method arguments should compile");

    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(Value::Int(4))
    );
}

#[test]
fn engine_compiler_options_lower_receiver_specific_named_standard_value_method_arguments() {
    let engine = Engine::builder()
        .with_standard_natives()
        .build()
        .expect("engine should build with standard natives");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main() {
    return "reward:gold".contains(needle = ":") && ["gold"].contains(value = "gold");
}
"#,
        &engine.compiler_options(),
    )
    .expect("receiver-specific named stdlib value method arguments should compile");

    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(Value::Bool(true))
    );
}

#[test]
fn engine_compiler_options_lower_local_receiver_named_standard_value_method_arguments() {
    let engine = Engine::builder()
        .with_standard_natives()
        .build()
        .expect("engine should build with standard natives");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main(text: string) {
    let parts = ["gold"];
    let reward = "reward:gold";
    return text.contains(needle = ":")
        && reward.contains(needle = ":")
        && parts.contains(value = "gold");
}
"#,
        &engine.compiler_options(),
    )
    .expect("local receiver named stdlib value method arguments should compile");

    assert_eq!(
        engine
            .into_vm()
            .run_program(&program, "main", &[Value::String("loot:xp".to_owned())]),
        Ok(Value::Bool(true))
    );
}

#[test]
fn engine_compiler_options_reject_ambiguous_named_standard_value_method_arguments() {
    let engine = Engine::builder()
        .with_standard_natives()
        .build()
        .expect("engine should build with standard natives");
    compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main(value) {
    return value.contains(needle = ":");
}
"#,
        &engine.compiler_options(),
    )
    .expect_err("ambiguous stdlib value method names should not accept named args");
}

#[test]
fn engine_builder_installs_standard_natives_into_runtime() {
    let engine = Engine::builder()
        .with_standard_natives()
        .build()
        .expect("engine should build with standard natives");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    let tags = set::from_array(["fire", "ice", "fire"]);
    let midpoint = math::floor(math::lerp(10, 20, 0.5));
    let range = math::round(math::distance3d(0, 0, 0, 2, 3, 6));
    let score = math::pow(2, 3);
    let root = math::round(math::sqrt(81));
    let direction = math::sign(-3);
    let approach = math::move_towards(0, 10, 4);
    return tags.len() + option::unwrap_or(option::some(midpoint), 0) + math::round(1.5) + range + score + root + direction + approach;
}
"#,
    )
    .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    let result = runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx);
    assert_eq!(result, Ok(Value::Int(46)),);
}

#[test]
fn engine_installs_registered_host_native_functions_into_vm() {
    let engine = Engine::builder()
        .grant_permission("player.write")
        .register_host_native_fn(
            NativeFunctionDesc::new("game::set_level", NativeFunctionId::new(2))
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
    game::set_level(player, 9);
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
            NativeFunctionDesc::new("game::context_set_level", NativeFunctionId::new(23))
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
                        .native_function_by_name("game::context_set_level")
                        .is_none()
                );
                assert!(
                    ctx.engine()
                        .context_host_native_function_by_name("game::context_set_level")
                        .is_some()
                );
                ctx.set_path(
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
    return game::context_set_level(player, 11);
}
"#,
    )
    .expect("program should compile");
    let registry = engine.registry();
    let function = registry
        .function_by_name("game::context_set_level")
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
fn context_host_native_read_path_observes_patch_overlay() {
    let engine = Engine::builder()
        .register_context_host_native_fn(
            NativeFunctionDesc::new("game::read_after_context_write", NativeFunctionId::new(33))
                .param(
                    "player",
                    TypeHint::Host(TypeKey::new(TypeId::new(1), "Player")),
                )
                .param("level", TypeHint::Int)
                .returns(TypeHint::Int)
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public()),
            |args, ctx| {
                let player = args.required::<HostRef>(0)?;
                let level = args.required::<i64>(1)?;
                let path = HostPath::new(player).field(FieldId::new(1));

                assert_eq!(ctx.read_path(&path, None)?, HostValue::Int(3));
                ctx.set_path(path.clone(), HostValue::Int(level), None)?;
                match ctx.read_path(&path, None)? {
                    HostValue::Int(value) => Ok(Value::Int(value)),
                    _ => Ok(Value::Null),
                }
            },
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    return game::read_after_context_write(player, 17);
}
"#,
    )
    .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let level = HostPath::new(host_ref).field(FieldId::new(1));
    let mut adapter = MockStateAdapter::new();
    adapter.insert_value(level.clone(), HostValue::Int(3));
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call(
            "main",
            &[Value::HostRef(host_ref)],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx,
        ),
        Ok(Value::Int(17))
    );
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(tx.patches()[0].path, level);
    assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Int(17)));
    assert_eq!(adapter.read_path(&level), Ok(HostValue::Int(3)));
}

#[test]
fn context_host_native_can_charge_execution_budget_before_patching() {
    let engine = Engine::builder()
        .register_context_host_native_fn(
            NativeFunctionDesc::new("game::expensive_set_level", NativeFunctionId::new(24))
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
                ctx.set_path(
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
    game::expensive_set_level(player, 13);
    return 1;
}
"#,
    )
    .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);
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
fn context_host_native_can_charge_memory_budget_before_patching() {
    let engine = Engine::builder()
        .register_context_host_native_fn(
            NativeFunctionDesc::new("game::memory_checked_set_level", NativeFunctionId::new(25))
                .param(
                    "player",
                    TypeHint::Host(TypeKey::new(TypeId::new(1), "Player")),
                )
                .param("level", TypeHint::Int)
                .returns(TypeHint::Null)
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public()),
            |args, ctx| {
                ctx.charge_memory_bytes(128)?;
                let player = args.required::<HostRef>(0)?;
                let level = args.required::<i64>(1)?;
                ctx.set_path(
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
    game::memory_checked_set_level(player, 13);
    return 1;
}
"#,
    )
    .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    let error = runtime
        .call(
            "main",
            &[Value::HostRef(host_ref)],
            CallOptions::new(u64::MAX, 64, usize::MAX, usize::MAX),
            &mut adapter,
            &mut tx,
        )
        .expect_err("native memory budget charge should fail");

    assert_eq!(
        error.kind,
        VmErrorKind::BudgetExceeded {
            budget: ExecutionBudgetKind::MemoryBytes,
            limit: 64
        }
    );
    assert!(tx.patches().is_empty());
}

#[test]
fn context_host_native_set_path_reserves_patch_budget_before_patching() {
    let engine = Engine::builder()
        .register_context_host_native_fn(
            NativeFunctionDesc::new("game::patch_checked_set_level", NativeFunctionId::new(26))
                .param(
                    "player",
                    TypeHint::Host(TypeKey::new(TypeId::new(1), "Player")),
                )
                .param("level", TypeHint::Int)
                .returns(TypeHint::Null)
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public()),
            |args, ctx| {
                let player = args.required::<HostRef>(0)?;
                let level = args.required::<i64>(1)?;
                ctx.set_path(
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
    game::patch_checked_set_level(player, 13);
    return 1;
}
"#,
    )
    .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    let error = runtime
        .call(
            "main",
            &[Value::HostRef(host_ref)],
            CallOptions::new(u64::MAX, usize::MAX, usize::MAX, 0),
            &mut adapter,
            &mut tx,
        )
        .expect_err("native patch budget reservation should fail");

    assert_eq!(
        error.kind,
        VmErrorKind::BudgetExceeded {
            budget: ExecutionBudgetKind::Patches,
            limit: 0
        }
    );
    assert!(tx.patches().is_empty());
}

#[test]
fn context_host_native_patch_helpers_record_expected_patches() {
    let method = HostMethodId::new(77);
    let engine = Engine::builder()
        .register_context_host_native_fn(
            NativeFunctionDesc::new("game::record_patch_helpers", NativeFunctionId::new(31))
                .param(
                    "player",
                    TypeHint::Host(TypeKey::new(TypeId::new(1), "Player")),
                )
                .returns(TypeHint::Null)
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public()),
            move |args, ctx| {
                let player = args.required::<HostRef>(0)?;
                let numeric = HostPath::new(player).field(FieldId::new(1));
                let array = HostPath::new(player).field(FieldId::new(2));
                let inventory = HostPath::new(player).field(FieldId::new(3));
                ctx.add_path(numeric.clone(), HostValue::Int(2), HostValue::Int(10), None)?;
                ctx.sub_path(numeric.clone(), HostValue::Int(3), HostValue::Int(12), None)?;
                ctx.mul_path(numeric.clone(), HostValue::Int(4), HostValue::Int(9), None)?;
                ctx.div_path(numeric.clone(), HostValue::Int(2), HostValue::Int(36), None)?;
                ctx.rem_path(numeric.clone(), HostValue::Int(5), HostValue::Int(18), None)?;
                ctx.push_path(
                    array.clone(),
                    HostValue::String("gold".to_owned()),
                    HostValue::Array(vec![]),
                    None,
                )?;
                ctx.remove_path(array, None)?;
                ctx.call_method(inventory, method, vec![HostValue::Int(4)], None)?;
                Ok(Value::Null)
            },
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    game::record_patch_helpers(player);
    return 1;
}
"#,
    )
    .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call(
            "main",
            &[Value::HostRef(host_ref)],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx,
        ),
        Ok(Value::Int(1))
    );

    let numeric = HostPath::new(host_ref).field(FieldId::new(1));
    let array = HostPath::new(host_ref).field(FieldId::new(2));
    let inventory = HostPath::new(host_ref).field(FieldId::new(3));
    assert_eq!(tx.patches().len(), 8);
    assert_eq!(tx.patches()[0].path, numeric);
    assert_eq!(tx.patches()[0].op, PatchOp::Add(HostValue::Int(2)));
    assert_eq!(tx.patches()[1].path, numeric);
    assert_eq!(tx.patches()[1].op, PatchOp::Sub(HostValue::Int(3)));
    assert_eq!(tx.patches()[2].path, numeric);
    assert_eq!(tx.patches()[2].op, PatchOp::Mul(HostValue::Int(4)));
    assert_eq!(tx.patches()[3].path, numeric);
    assert_eq!(tx.patches()[3].op, PatchOp::Div(HostValue::Int(2)));
    assert_eq!(tx.patches()[4].path, numeric);
    assert_eq!(tx.patches()[4].op, PatchOp::Rem(HostValue::Int(5)));
    assert_eq!(tx.patches()[5].path, array);
    assert_eq!(
        tx.patches()[5].op,
        PatchOp::Push(HostValue::String("gold".to_owned()))
    );
    assert_eq!(tx.patches()[6].path, array);
    assert_eq!(tx.patches()[6].op, PatchOp::Remove);
    assert_eq!(tx.patches()[7].path, inventory);
    assert_eq!(
        tx.patches()[7].op,
        PatchOp::CallHostMethod {
            method,
            args: vec![HostValue::Int(4)]
        }
    );
}

#[test]
fn context_host_native_patch_helpers_reserve_patch_budget_before_patching() {
    let method = HostMethodId::new(78);
    let engine = Engine::builder()
        .register_context_host_native_fn(
            NativeFunctionDesc::new("game::patch_checked_helper", NativeFunctionId::new(32))
                .param(
                    "player",
                    TypeHint::Host(TypeKey::new(TypeId::new(1), "Player")),
                )
                .param("mode", TypeHint::Int)
                .returns(TypeHint::Null)
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public()),
            move |args, ctx| {
                let player = args.required::<HostRef>(0)?;
                let mode = args.required::<i64>(1)?;
                let numeric = HostPath::new(player).field(FieldId::new(1));
                let array = HostPath::new(player).field(FieldId::new(2));
                let inventory = HostPath::new(player).field(FieldId::new(3));
                match mode {
                    0 => ctx.add_path(numeric, HostValue::Int(1), HostValue::Int(10), None)?,
                    1 => ctx.sub_path(numeric, HostValue::Int(1), HostValue::Int(10), None)?,
                    2 => ctx.mul_path(numeric, HostValue::Int(2), HostValue::Int(10), None)?,
                    3 => ctx.div_path(numeric, HostValue::Int(2), HostValue::Int(10), None)?,
                    4 => ctx.rem_path(numeric, HostValue::Int(3), HostValue::Int(10), None)?,
                    5 => ctx.push_path(
                        array,
                        HostValue::String("gold".to_owned()),
                        HostValue::Array(vec![]),
                        None,
                    )?,
                    6 => ctx.remove_path(array, None)?,
                    7 => ctx.call_method(inventory, method, vec![HostValue::Int(1)], None)?,
                    _ => {}
                }
                Ok(Value::Null)
            },
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player, mode) {
    game::patch_checked_helper(player, mode);
    return 1;
}
"#,
    )
    .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();

    for mode in 0..=7 {
        let mut tx = PatchTx::new();
        let error = runtime
            .call(
                "main",
                &[Value::HostRef(host_ref), Value::Int(mode)],
                CallOptions::new(u64::MAX, usize::MAX, usize::MAX, 0),
                &mut adapter,
                &mut tx,
            )
            .expect_err("native patch helper budget reservation should fail");

        assert_eq!(
            error.kind,
            VmErrorKind::BudgetExceeded {
                budget: ExecutionBudgetKind::Patches,
                limit: 0
            }
        );
        assert!(tx.patches().is_empty());
    }
}

#[test]
fn host_native_patch_budget_rolls_back_overflow_patch() {
    let engine = Engine::builder()
        .grant_permission("player.write")
        .register_host_native_fn(
            NativeFunctionDesc::new("game::unchecked_set_level", NativeFunctionId::new(28))
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
    game::unchecked_set_level(player, 13);
    return 1;
}
"#,
    )
    .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    let error = runtime
        .call(
            "main",
            &[Value::HostRef(host_ref)],
            CallOptions::new(u64::MAX, usize::MAX, usize::MAX, 0),
            &mut adapter,
            &mut tx,
        )
        .expect_err("host native overflow patch should fail");

    assert_eq!(
        error.kind,
        VmErrorKind::BudgetExceeded {
            budget: ExecutionBudgetKind::Patches,
            limit: 0
        }
    );
    assert!(tx.patches().is_empty());
}

#[test]
fn host_native_error_rolls_back_recorded_patches() {
    let engine = Engine::builder()
        .grant_permission("player.write")
        .register_host_native_fn(
            NativeFunctionDesc::new("game::failing_set_level", NativeFunctionId::new(29))
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
                Err(VmError {
                    kind: VmErrorKind::TypeMismatch {
                        operation: "failing host native",
                    },
                    source_span: None,
                    call_stack: Default::default(),
                })
            },
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    game::failing_set_level(player, 13);
    return 1;
}
"#,
    )
    .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    let error = runtime
        .call(
            "main",
            &[Value::HostRef(host_ref)],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx,
        )
        .expect_err("host native error should fail");

    assert_eq!(
        error.kind,
        VmErrorKind::TypeMismatch {
            operation: "failing host native"
        }
    );
    assert!(tx.patches().is_empty());
}

#[test]
fn host_native_error_rolls_back_patches_without_call_options() {
    let engine = Engine::builder()
        .grant_permission("player.write")
        .register_host_native_fn(
            NativeFunctionDesc::new("game::direct_failing_set_level", NativeFunctionId::new(30))
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
                Err(VmError {
                    kind: VmErrorKind::TypeMismatch {
                        operation: "direct failing host native",
                    },
                    source_span: None,
                    call_stack: Default::default(),
                })
            },
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    game::direct_failing_set_level(player, 13);
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

    let error = engine
        .into_vm()
        .run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host)
        .expect_err("host native error should fail");

    assert_eq!(
        error.kind,
        VmErrorKind::TypeMismatch {
            operation: "direct failing host native"
        }
    );
    assert!(tx.patches().is_empty());
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
    let mut runtime = Runtime::new(engine, program);
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
            call_stack: Arc::from([vela_vm::error::VmStackFrame {
                function: "main".to_owned(),
                call_site: None,
                bytecode_offset: None,
            }]),
        }
    );
    assert!(tx.patches().is_empty());
}

#[test]
fn engine_denies_native_calls_missing_required_permission() {
    let engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::secret", NativeFunctionId::new(3))
                .returns(TypeHint::Int)
                .access(FunctionAccess::public().require_permission("game::secret")),
            |_| Ok(Value::Int(99)),
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    return game::secret();
}
"#,
    )
    .expect("program should compile");

    assert!(matches!(
        engine.into_vm().run_program(&program, "main", &[]),
        Err(error) if error.kind == VmErrorKind::PermissionDenied {
            native: "game::secret".to_owned(),
            permission: "game::secret".to_owned(),
        }
    ));
}

#[test]
fn engine_denies_host_native_before_recording_patches() {
    let engine = Engine::builder()
        .register_host_native_fn(
            NativeFunctionDesc::new("game::set_level", NativeFunctionId::new(4))
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
    game::set_level(player, 9);
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
            native: "game::set_level".to_owned(),
            permission: "player.write".to_owned(),
        }
    ));
    assert!(tx.patches().is_empty());
}
