use super::*;

#[test]
fn host_native_patch_budget_error_retains_prior_write() {
    let engine = Engine::builder()
        .capability(Capability::HostWrite)
        .register_host_native_fn(
            NativeFunctionDesc::new("game::unchecked_set_level", NativeFunctionId::new(28))
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
        .call_raw(
            "main",
            &[OwnedValue::HostRef(host_ref)],
            CallOptions::new(u64::MAX, usize::MAX, usize::MAX, 0),
            &mut adapter,
            &mut tx,
        )
        .expect_err("host native overflow patch should fail");

    assert_eq!(
        error.kind,
        VmErrorKind::BudgetExceeded {
            budget: ExecutionBudgetKind::HostMutations,
            limit: 0
        }
    );
    let level = HostPath::new(host_ref).field(FieldId::new(1));
    assert_eq!(tx.mutation_count(), 1);
    assert_eq!(adapter.read_path(&level), Ok(HostValue::Int(13)));
}

#[test]
fn host_native_error_retains_written_mutations() {
    let engine = Engine::builder()
        .capability(Capability::HostWrite)
        .register_host_native_fn(
            NativeFunctionDesc::new("game::failing_set_level", NativeFunctionId::new(29))
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
        .call_raw(
            "main",
            &[OwnedValue::HostRef(host_ref)],
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
    let level = HostPath::new(host_ref).field(FieldId::new(1));
    assert_eq!(tx.mutation_count(), 1);
    assert_eq!(adapter.read_path(&level), Ok(HostValue::Int(13)));
}

#[test]
fn host_native_error_retains_mutations_without_call_options() {
    let engine = Engine::builder()
        .capability(Capability::HostWrite)
        .register_host_native_fn(
            NativeFunctionDesc::new("game::direct_failing_set_level", NativeFunctionId::new(30))
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
        .run_program_with_host(
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host,
        )
        .expect_err("host native error should fail");

    assert_eq!(
        error.kind,
        VmErrorKind::TypeMismatch {
            operation: "direct failing host native"
        }
    );
    let level = HostPath::new(host_ref).field(FieldId::new(1));
    assert_eq!(tx.mutation_count(), 1);
    assert_eq!(adapter.read_path(&level), Ok(HostValue::Int(13)));
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
        .call_raw(
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
    assert!(tx.is_empty());
}

#[test]
fn engine_allows_pure_native_calls_without_capabilities() {
    let engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::secret", NativeFunctionId::new(3))
                .returns(TypeHint::Int)
                .access(FunctionAccess::public()),
            |_| Ok(OwnedValue::Int(99)),
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

    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(OwnedValue::Int(99))
    );
}

#[test]
fn engine_denies_host_native_before_mutation_counting() {
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

    assert!(matches!(
        engine
            .into_vm()
            .run_program_with_host(&program, "main", &[OwnedValue::HostRef(host_ref)], &mut host),
        Err(error) if error.kind == VmErrorKind::PermissionDenied {
            native: "game::set_level".to_owned(),
            capability: Capability::HostWrite.as_str().to_owned(),
        }
    ));
    assert!(tx.is_empty());
}
