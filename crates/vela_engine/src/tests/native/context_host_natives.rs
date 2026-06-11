use super::*;

use vela_bytecode::UnlinkedProgram;
use vela_vm::budget::ExecutionBudget;
use vela_vm::error::VmResult;

fn run_linked_program_with_host(
    engine: &Engine,
    program: &UnlinkedProgram,
    args: &[OwnedValue],
    host: &mut HostExecution<'_>,
) -> VmResult<OwnedValue> {
    let linked = engine
        .link_program(program)
        .expect("engine context host native test program should link");
    let mut budget = ExecutionBudget::unbounded();
    engine
        .into_vm_for_program(program)
        .run_linked_program_with_host_budget_and_caches(
            &linked,
            "main",
            args,
            host,
            &mut budget,
            None,
        )
}

#[test]
fn engine_installs_registered_host_native_functions_into_vm() {
    let engine = Engine::builder()
        .capability(Capability::HostWrite)
        .register_host_native_fn(
            NativeFunctionDesc::new("game::set_level", NativeFunctionId::new(2))
                .param(
                    "player",
                    TypeHint::Host(TypeKey::new(TypeId::new(1), "Player")),
                )
                .param("level", TypeHint::i64())
                .returns(TypeHint::null())
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public()),
            |args, host| {
                let [
                    OwnedValue::HostRef(player),
                    OwnedValue::Scalar(vela_common::ScalarValue::I64(level)),
                ] = args
                else {
                    return Ok(OwnedValue::Null);
                };
                host.access.write_diagnostic_path(
                    host.adapter,
                    HostPath::new(*player).field(FieldId::new(1)),
                    HostValue::Scalar(vela_common::ScalarValue::I64(*level)),
                    None,
                )?;
                Ok(OwnedValue::Null)
            },
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
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
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        run_linked_program_with_host(
            &engine,
            &program,
            &[OwnedValue::HostRef(host_ref)],
            &mut host
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
}

#[test]
fn engine_installs_context_host_native_functions_into_vm() {
    let engine = Engine::builder()
        .capability(Capability::HostWrite)
        .register_context_host_native_fn(
            NativeFunctionDesc::new("game::context_set_level", NativeFunctionId::new(23))
                .param(
                    "player",
                    TypeHint::Host(TypeKey::new(TypeId::new(1), "Player")),
                )
                .param("level", TypeHint::i64())
                .returns(TypeHint::boolean())
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public()),
            |args, ctx| {
                let [
                    OwnedValue::HostRef(player),
                    OwnedValue::Scalar(vela_common::ScalarValue::I64(level)),
                ] = args
                else {
                    return Ok(OwnedValue::Bool(false));
                };
                assert!(ctx.has_capability(Capability::HostWrite));
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
                    HostValue::Scalar(vela_common::ScalarValue::I64(*level)),
                    None,
                )?;
                Ok(OwnedValue::Bool(true))
            },
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
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
    assert!(function.access.required_permissions().is_empty());
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        run_linked_program_with_host(
            &engine,
            &program,
            &[OwnedValue::HostRef(host_ref)],
            &mut host
        ),
        Ok(OwnedValue::Bool(true))
    );
}

#[test]
fn context_host_native_read_path_observes_write_through_state() {
    let engine = Engine::builder()
        .capability(Capability::HostWrite)
        .register_context_host_native_fn(
            NativeFunctionDesc::new("game::read_after_context_write", NativeFunctionId::new(33))
                .param(
                    "player",
                    TypeHint::Host(TypeKey::new(TypeId::new(1), "Player")),
                )
                .param("level", TypeHint::i64())
                .returns(TypeHint::i64())
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public()),
            |args, ctx| {
                let player = args.required::<HostRef>(0)?;
                let level = args.required::<i64>(1)?;
                let path = HostPath::new(player).field(FieldId::new(1));

                assert_eq!(
                    ctx.read_path(&path, None)?,
                    HostValue::Scalar(vela_common::ScalarValue::I64(3))
                );
                ctx.set_path(
                    path.clone(),
                    HostValue::Scalar(vela_common::ScalarValue::I64(level)),
                    None,
                )?;
                match ctx.read_path(&path, None)? {
                    HostValue::Scalar(vela_common::ScalarValue::I64(value)) => {
                        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(value)))
                    }
                    _ => Ok(OwnedValue::Null),
                }
            },
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
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
    adapter.insert_diagnostic_path_value(
        level.clone(),
        HostValue::Scalar(vela_common::ScalarValue::I64(3)),
    );
    let mut tx = HostAccess::new();

    assert_eq!(
        runtime.call_raw(
            "main",
            &[OwnedValue::HostRef(host_ref)],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx,
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(17)))
    );
    assert_eq!(
        adapter.read_diagnostic_path(&level),
        Ok(HostValue::Scalar(vela_common::ScalarValue::I64(17)))
    );
}

#[test]
fn context_host_native_returns_immediate_method_result() {
    let method = HostMethodId::new(79);
    let engine = Engine::builder()
        .capability(Capability::HostWrite)
        .register_context_host_native_fn(
            NativeFunctionDesc::new("game::preview_inventory_add", NativeFunctionId::new(34))
                .param(
                    "player",
                    TypeHint::Host(TypeKey::new(TypeId::new(1), "Player")),
                )
                .returns(TypeHint::string())
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public()),
            move |args, ctx| {
                let player = args.required::<HostRef>(0)?;
                let inventory = HostPath::new(player).field(FieldId::new(3));
                let method_args = vec![
                    HostValue::String("gold".to_owned()),
                    HostValue::Scalar(vela_common::ScalarValue::I64(2)),
                ];
                let result = ctx.call_method(inventory, method, method_args, None)?;
                match result {
                    HostValue::String(value) => Ok(OwnedValue::String(value)),
                    _ => Ok(OwnedValue::Null),
                }
            },
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main(player) {
    return game::preview_inventory_add(player);
}
"#,
        )
        .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let inventory = HostPath::new(host_ref).field(FieldId::new(3));
    let mut adapter = MockStateAdapter::new();
    adapter.insert_method_return(method, HostValue::String("accepted".to_owned()));
    let mut tx = HostAccess::new();

    assert_eq!(
        runtime.call_raw(
            "main",
            &[OwnedValue::HostRef(host_ref)],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx,
        ),
        Ok(OwnedValue::String("accepted".to_owned()))
    );
    assert_eq!(adapter.method_calls().len(), 1);
    assert_eq!(adapter.method_calls()[0].diagnostic_path(), inventory);
    assert_eq!(adapter.method_calls()[0].method, method);
    assert_eq!(
        adapter.method_calls()[0].args,
        vec![
            HostValue::String("gold".to_owned()),
            HostValue::Scalar(vela_common::ScalarValue::I64(2))
        ]
    );
}

#[test]
fn context_host_native_can_charge_execution_budget_before_host_access() {
    let engine = Engine::builder()
        .capability(Capability::HostWrite)
        .register_context_host_native_fn(
            NativeFunctionDesc::new("game::expensive_set_level", NativeFunctionId::new(24))
                .param(
                    "player",
                    TypeHint::Host(TypeKey::new(TypeId::new(1), "Player")),
                )
                .param("level", TypeHint::i64())
                .returns(TypeHint::null())
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public()),
            |args, ctx| {
                ctx.charge_instructions(100)?;
                let player = args.required::<HostRef>(0)?;
                let level = args.required::<i64>(1)?;
                ctx.set_path(
                    HostPath::new(player).field(FieldId::new(1)),
                    HostValue::Scalar(vela_common::ScalarValue::I64(level)),
                    None,
                )?;
                Ok(OwnedValue::Null)
            },
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
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
    let mut tx = HostAccess::new();

    let error = runtime
        .call_raw(
            "main",
            &[OwnedValue::HostRef(host_ref)],
            CallOptions::new(50, usize::MAX, usize::MAX),
            &mut adapter,
            &mut tx,
        )
        .expect_err("native budget charge should fail");

    assert_eq!(
        error.kind(),
        VmErrorKind::BudgetExceeded {
            budget: ExecutionBudgetKind::Instructions,
            limit: 50
        }
    );
}

#[test]
fn context_host_native_can_charge_memory_budget_before_host_access() {
    let engine = Engine::builder()
        .capability(Capability::HostWrite)
        .register_context_host_native_fn(
            NativeFunctionDesc::new("game::memory_checked_set_level", NativeFunctionId::new(25))
                .param(
                    "player",
                    TypeHint::Host(TypeKey::new(TypeId::new(1), "Player")),
                )
                .param("level", TypeHint::i64())
                .returns(TypeHint::null())
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public()),
            |args, ctx| {
                ctx.charge_memory_bytes(128)?;
                let player = args.required::<HostRef>(0)?;
                let level = args.required::<i64>(1)?;
                ctx.set_path(
                    HostPath::new(player).field(FieldId::new(1)),
                    HostValue::Scalar(vela_common::ScalarValue::I64(level)),
                    None,
                )?;
                Ok(OwnedValue::Null)
            },
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
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
    let numeric = HostPath::new(host_ref).field(FieldId::new(1));
    adapter.insert_diagnostic_path_value(
        numeric.clone(),
        HostValue::Scalar(vela_common::ScalarValue::I64(10)),
    );
    let mut tx = HostAccess::new();

    let error = runtime
        .call_raw(
            "main",
            &[OwnedValue::HostRef(host_ref)],
            CallOptions::new(u64::MAX, 64, usize::MAX),
            &mut adapter,
            &mut tx,
        )
        .expect_err("native memory budget charge should fail");

    assert_eq!(
        error.kind(),
        VmErrorKind::BudgetExceeded {
            budget: ExecutionBudgetKind::MemoryBytes,
            limit: 64
        }
    );
}

#[test]
fn context_host_native_set_path_writes_through() {
    let engine = Engine::builder()
        .capability(Capability::HostWrite)
        .register_context_host_native_fn(
            NativeFunctionDesc::new("game::patch_checked_set_level", NativeFunctionId::new(26))
                .param(
                    "player",
                    TypeHint::Host(TypeKey::new(TypeId::new(1), "Player")),
                )
                .param("level", TypeHint::i64())
                .returns(TypeHint::null())
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public()),
            |args, ctx| {
                let player = args.required::<HostRef>(0)?;
                let level = args.required::<i64>(1)?;
                ctx.set_path(
                    HostPath::new(player).field(FieldId::new(1)),
                    HostValue::Scalar(vela_common::ScalarValue::I64(level)),
                    None,
                )?;
                Ok(OwnedValue::Null)
            },
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
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
    let mut tx = HostAccess::new();

    assert_eq!(
        runtime.call_raw(
            "main",
            &[OwnedValue::HostRef(host_ref)],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx,
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
    assert_eq!(
        adapter.read_diagnostic_path(&HostPath::new(host_ref).field(FieldId::new(1))),
        Ok(HostValue::Scalar(vela_common::ScalarValue::I64(13)))
    );
}

#[test]
fn context_host_native_patch_helpers_write_through() {
    let method = HostMethodId::new(77);
    let engine = Engine::builder()
        .capability(Capability::HostWrite)
        .register_context_host_native_fn(
            NativeFunctionDesc::new("game::count_patch_helpers", NativeFunctionId::new(31))
                .param(
                    "player",
                    TypeHint::Host(TypeKey::new(TypeId::new(1), "Player")),
                )
                .returns(TypeHint::null())
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public()),
            move |args, ctx| {
                let player = args.required::<HostRef>(0)?;
                let numeric = HostPath::new(player).field(FieldId::new(1));
                let scratch = HostPath::new(player).field(FieldId::new(2));
                let inventory = HostPath::new(player).field(FieldId::new(3));
                ctx.add_path(
                    numeric.clone(),
                    HostValue::Scalar(vela_common::ScalarValue::I64(2)),
                    None,
                )?;
                ctx.sub_path(
                    numeric.clone(),
                    HostValue::Scalar(vela_common::ScalarValue::I64(3)),
                    None,
                )?;
                ctx.mul_path(
                    numeric.clone(),
                    HostValue::Scalar(vela_common::ScalarValue::I64(4)),
                    None,
                )?;
                ctx.div_path(
                    numeric.clone(),
                    HostValue::Scalar(vela_common::ScalarValue::I64(2)),
                    None,
                )?;
                ctx.rem_path(
                    numeric.clone(),
                    HostValue::Scalar(vela_common::ScalarValue::I64(5)),
                    None,
                )?;
                ctx.remove_path(scratch, None)?;
                ctx.call_method(
                    inventory,
                    method,
                    vec![HostValue::Scalar(vela_common::ScalarValue::I64(4))],
                    None,
                )?;
                Ok(OwnedValue::Null)
            },
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main(player) {
    game::count_patch_helpers(player);
    return 1;
}
"#,
        )
        .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let numeric = HostPath::new(host_ref).field(FieldId::new(1));
    let scratch = HostPath::new(host_ref).field(FieldId::new(2));
    adapter.insert_diagnostic_path_value(
        numeric.clone(),
        HostValue::Scalar(vela_common::ScalarValue::I64(10)),
    );
    adapter.insert_diagnostic_path_value(
        scratch.clone(),
        HostValue::Scalar(vela_common::ScalarValue::I64(0)),
    );
    let mut tx = HostAccess::new();

    assert_eq!(
        runtime.call_raw(
            "main",
            &[OwnedValue::HostRef(host_ref)],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx,
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
    assert_eq!(
        adapter.read_diagnostic_path(&numeric),
        Ok(HostValue::Scalar(vela_common::ScalarValue::I64(3)))
    );
    assert!(adapter.read_diagnostic_path(&scratch).is_err());
    assert_eq!(adapter.method_calls().len(), 1);
}

#[test]
fn context_host_native_repeated_writes_write_through() {
    let engine = Engine::builder()
        .capability(Capability::HostWrite)
        .register_context_host_native_fn(
            NativeFunctionDesc::new("game::double_set_level", NativeFunctionId::new(32))
                .param(
                    "player",
                    TypeHint::Host(TypeKey::new(TypeId::new(1), "Player")),
                )
                .returns(TypeHint::null())
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public()),
            |args, ctx| {
                let player = args.required::<HostRef>(0)?;
                let path = HostPath::new(player).field(FieldId::new(1));
                ctx.set_path(
                    path.clone(),
                    HostValue::Scalar(vela_common::ScalarValue::I64(12)),
                    None,
                )?;
                ctx.set_path(
                    path,
                    HostValue::Scalar(vela_common::ScalarValue::I64(13)),
                    None,
                )?;
                Ok(OwnedValue::Null)
            },
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main(player) {
    game::double_set_level(player);
    return 1;
}
"#,
        )
        .expect("program should compile");
    let mut runtime = Runtime::new(engine, program);
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    assert_eq!(
        runtime.call_raw(
            "main",
            &[OwnedValue::HostRef(host_ref)],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx,
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
    assert_eq!(
        adapter.read_diagnostic_path(&HostPath::new(host_ref).field(FieldId::new(1))),
        Ok(HostValue::Scalar(vela_common::ScalarValue::I64(13)))
    );
}
