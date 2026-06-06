use super::*;

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
                .param("level", TypeHint::Int)
                .returns(TypeHint::Null)
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public()),
            |args, host| {
                let [OwnedValue::HostRef(player), OwnedValue::Int(level)] = args else {
                    return Ok(OwnedValue::Null);
                };
                host.tx.set_path(
                    host.adapter,
                    HostPath::new(*player).field(FieldId::new(1)),
                    HostValue::Int(*level),
                    None,
                )?;
                Ok(OwnedValue::Null)
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
            &[OwnedValue::HostRef(host_ref)],
            &mut host
        ),
        Ok(OwnedValue::Int(1))
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
        .capability(Capability::HostWrite)
        .register_context_host_native_fn(
            NativeFunctionDesc::new("game::context_set_level", NativeFunctionId::new(23))
                .param(
                    "player",
                    TypeHint::Host(TypeKey::new(TypeId::new(1), "Player")),
                )
                .param("level", TypeHint::Int)
                .returns(TypeHint::Bool)
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public()),
            |args, ctx| {
                let [OwnedValue::HostRef(player), OwnedValue::Int(level)] = args else {
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
                    HostValue::Int(*level),
                    None,
                )?;
                Ok(OwnedValue::Bool(true))
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
    assert!(function.access.required_permissions().is_empty());
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
            &[OwnedValue::HostRef(host_ref)],
            &mut host
        ),
        Ok(OwnedValue::Bool(true))
    );
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(
        tx.patches()[0].path,
        HostPath::new(host_ref).field(FieldId::new(1))
    );
    assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Int(11)));
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
                    HostValue::Int(value) => Ok(OwnedValue::Int(value)),
                    _ => Ok(OwnedValue::Null),
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
        runtime.call_raw(
            "main",
            &[OwnedValue::HostRef(host_ref)],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx,
        ),
        Ok(OwnedValue::Int(17))
    );
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(tx.patches()[0].path, level);
    assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Int(17)));
    assert_eq!(adapter.read_path(&level), Ok(HostValue::Int(17)));
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
                .returns(TypeHint::String)
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public()),
            move |args, ctx| {
                let player = args.required::<HostRef>(0)?;
                let inventory = HostPath::new(player).field(FieldId::new(3));
                let method_args = vec![HostValue::String("gold".to_owned()), HostValue::Int(2)];
                let result = ctx.call_method(inventory, method, method_args, None)?;
                match result {
                    HostValue::String(value) => Ok(OwnedValue::String(value)),
                    _ => Ok(OwnedValue::Null),
                }
            },
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source(
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
    adapter.insert_value(inventory.clone(), HostValue::Array(vec![]));
    adapter.insert_method_return(method, HostValue::String("accepted".to_owned()));
    let mut tx = PatchTx::new();

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
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(tx.patches()[0].path, inventory);
    assert_eq!(
        tx.patches()[0].op,
        PatchOp::CallHostMethod {
            method,
            args: vec![HostValue::String("gold".to_owned()), HostValue::Int(2)]
        }
    );
    assert_eq!(
        adapter.method_calls(),
        &[(
            inventory,
            method,
            vec![HostValue::String("gold".to_owned()), HostValue::Int(2)]
        )]
    );
}

#[test]
fn context_host_native_can_charge_execution_budget_before_journaling() {
    let engine = Engine::builder()
        .capability(Capability::HostWrite)
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
                Ok(OwnedValue::Null)
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
        .call_raw(
            "main",
            &[OwnedValue::HostRef(host_ref)],
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
fn context_host_native_can_charge_memory_budget_before_journaling() {
    let engine = Engine::builder()
        .capability(Capability::HostWrite)
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
                Ok(OwnedValue::Null)
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
    let numeric = HostPath::new(host_ref).field(FieldId::new(1));
    let array = HostPath::new(host_ref).field(FieldId::new(2));
    adapter.insert_value(numeric.clone(), HostValue::Int(10));
    adapter.insert_value(array.clone(), HostValue::Array(vec![]));
    let mut tx = PatchTx::new();

    let error = runtime
        .call_raw(
            "main",
            &[OwnedValue::HostRef(host_ref)],
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
fn context_host_native_set_path_reserves_patch_budget_before_writing() {
    let engine = Engine::builder()
        .capability(Capability::HostWrite)
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
                Ok(OwnedValue::Null)
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
        .call_raw(
            "main",
            &[OwnedValue::HostRef(host_ref)],
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
        .capability(Capability::HostWrite)
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
                ctx.add_path(numeric.clone(), HostValue::Int(2), None)?;
                ctx.sub_path(numeric.clone(), HostValue::Int(3), None)?;
                ctx.mul_path(numeric.clone(), HostValue::Int(4), None)?;
                ctx.div_path(numeric.clone(), HostValue::Int(2), None)?;
                ctx.rem_path(numeric.clone(), HostValue::Int(5), None)?;
                ctx.push_path(array.clone(), HostValue::String("gold".to_owned()), None)?;
                ctx.remove_path(array, None)?;
                ctx.call_method(inventory, method, vec![HostValue::Int(4)], None)?;
                Ok(OwnedValue::Null)
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
    let numeric = HostPath::new(host_ref).field(FieldId::new(1));
    let array = HostPath::new(host_ref).field(FieldId::new(2));
    adapter.insert_value(numeric.clone(), HostValue::Int(10));
    adapter.insert_value(array.clone(), HostValue::Array(vec![]));
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call_raw(
            "main",
            &[OwnedValue::HostRef(host_ref)],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx,
        ),
        Ok(OwnedValue::Int(1))
    );

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
fn context_host_native_patch_helpers_reserve_patch_budget_before_writing() {
    let method = HostMethodId::new(78);
    let engine = Engine::builder()
        .capability(Capability::HostWrite)
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
                    0 => ctx.add_path(numeric, HostValue::Int(1), None)?,
                    1 => ctx.sub_path(numeric, HostValue::Int(1), None)?,
                    2 => ctx.mul_path(numeric, HostValue::Int(2), None)?,
                    3 => ctx.div_path(numeric, HostValue::Int(2), None)?,
                    4 => ctx.rem_path(numeric, HostValue::Int(3), None)?,
                    5 => ctx.push_path(array, HostValue::String("gold".to_owned()), None)?,
                    6 => ctx.remove_path(array, None)?,
                    7 => {
                        let _ =
                            ctx.call_method(inventory, method, vec![HostValue::Int(1)], None)?;
                    }
                    _ => {}
                }
                Ok(OwnedValue::Null)
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
            .call_raw(
                "main",
                &[OwnedValue::HostRef(host_ref), OwnedValue::Int(mode)],
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
