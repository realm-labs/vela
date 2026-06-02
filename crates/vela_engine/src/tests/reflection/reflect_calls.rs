use super::*;

#[test]
fn engine_reflect_call_invokes_reflect_callable_native_functions() {
    let engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::add", NativeFunctionId::new(91))
                .param("lhs", TypeHint::Int)
                .param("rhs", TypeHint::Int)
                .returns(TypeHint::Int)
                .access(FunctionAccess::public().reflect_callable(true)),
            |args| {
                let [Value::Int(lhs), Value::Int(rhs)] = args else {
                    return Ok(Value::Null);
                };
                Ok(Value::Int(lhs + rhs))
            },
        )
        .reflection_permissions(ReflectPermissionSet::all())
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    let add = reflect::function("game::add");
    return reflect::call(add, 2, 3);
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
        Ok(Value::Int(5))
    );
    assert!(tx.patches().is_empty());
}

#[test]
fn engine_reflect_call_requires_call_permission_for_function_descriptors() {
    let engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::add", NativeFunctionId::new(95))
                .param("lhs", TypeHint::Int)
                .param("rhs", TypeHint::Int)
                .returns(TypeHint::Int)
                .access(FunctionAccess::public().reflect_callable(true)),
            |_| Ok(Value::Int(0)),
        )
        .reflection_permissions(ReflectPermissionSet::new().with(ReflectPermission::ReadTypeInfo))
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    let add = reflect::function("game::add");
    return reflect::call(add, 2, 3);
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
        Err(error) if error.kind == VmErrorKind::Reflect(ReflectErrorKind::PermissionDenied {
            permission: ReflectPermission::CallMethods,
        })
    ));
    assert!(tx.patches().is_empty());
}

#[test]
fn engine_reflect_call_rejects_non_callable_native_functions() {
    let engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::add", NativeFunctionId::new(92))
                .param("lhs", TypeHint::Int)
                .param("rhs", TypeHint::Int)
                .returns(TypeHint::Int),
            |_| Ok(Value::Int(0)),
        )
        .reflection_permissions(ReflectPermissionSet::all())
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    let add = reflect::function("game::add");
    return reflect::call(add, 2, 3);
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
        Err(error) if error.kind == VmErrorKind::Reflect(
            ReflectErrorKind::FunctionNotReflectCallable {
                function: "game::add".to_owned(),
                source_span: None,
            }
        )
    ));
    assert!(tx.patches().is_empty());
}

#[test]
fn engine_reflect_call_invokes_host_native_functions_through_patch_tx() {
    let engine = Engine::builder()
        .grant_permission("player.write")
        .register_host_native_fn(
            NativeFunctionDesc::new("game::set_level", NativeFunctionId::new(93))
                .param(
                    "player",
                    TypeHint::Host(TypeKey::new(TypeId::new(1), "Player")),
                )
                .param("level", TypeHint::Int)
                .returns(TypeHint::Null)
                .effects(EffectSet::host_write())
                .access(
                    FunctionAccess::public()
                        .reflect_callable(true)
                        .require_permission("player.write"),
                ),
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
        .reflection_permissions(ReflectPermissionSet::all())
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    let set_level = reflect::function("game::set_level");
    reflect::call(set_level, player, 12);
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
    assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Int(12)));
}

#[test]
fn engine_reflect_call_denies_effectful_native_functions_without_effect_permission() {
    let engine = Engine::builder()
        .grant_permission("player.write")
        .register_host_native_fn(
            NativeFunctionDesc::new("game::set_level", NativeFunctionId::new(94))
                .param(
                    "player",
                    TypeHint::Host(TypeKey::new(TypeId::new(1), "Player")),
                )
                .param("level", TypeHint::Int)
                .returns(TypeHint::Null)
                .effects(EffectSet {
                    reads_host: false,
                    writes_host: true,
                    emits_events: false,
                })
                .access(
                    FunctionAccess::public()
                        .reflect_callable(true)
                        .require_permission("player.write"),
                ),
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
        .reflection_permissions(
            ReflectPermissionSet::new()
                .with(ReflectPermission::ReadTypeInfo)
                .with(ReflectPermission::CallMethods),
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    let set_level = reflect::function("game::set_level");
    reflect::call(set_level, player, 12);
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
        Err(error) if error.kind == VmErrorKind::Reflect(
            ReflectErrorKind::FunctionEffectPermissionDenied {
                function: "game::set_level".to_owned(),
                permission: ReflectPermission::CallHostWriteMethods,
                source_span: None,
            }
        )
    ));
    assert!(tx.patches().is_empty());
}
