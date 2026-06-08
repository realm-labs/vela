use super::*;
use vela_vm::owned_value::OwnedValue;

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
                let [OwnedValue::Int(lhs), OwnedValue::Int(rhs)] = args else {
                    return Ok(OwnedValue::Null);
                };
                Ok(OwnedValue::Int(lhs + rhs))
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
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        engine
            .into_vm()
            .run_program_with_host(&program, "main", &[], &mut host),
        Ok(OwnedValue::Int(5))
    );
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
            |_| Ok(OwnedValue::Int(0)),
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
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert!(matches!(
        engine
            .into_vm()
            .run_program_with_host(&program, "main", &[], &mut host),
        Err(error) if error.kind() == VmErrorKind::Reflect(ReflectErrorKind::PermissionDenied {
            permission: ReflectPermission::CallMethods,
        })
    ));
}

#[test]
fn engine_reflect_call_rejects_non_callable_native_functions() {
    let engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::add", NativeFunctionId::new(92))
                .param("lhs", TypeHint::Int)
                .param("rhs", TypeHint::Int)
                .returns(TypeHint::Int),
            |_| Ok(OwnedValue::Int(0)),
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
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert!(matches!(
        engine
            .into_vm()
            .run_program_with_host(&program, "main", &[], &mut host),
        Err(error) if error.kind() == VmErrorKind::Reflect(
            ReflectErrorKind::FunctionNotReflectCallable {
                function: "game::add".to_owned(),
                source_span: None,
            }
        )
    ));
}

#[test]
fn engine_reflect_call_invokes_host_native_functions_through_host_access() {
    let engine = Engine::builder()
        .capability(Capability::HostWrite)
        .register_host_native_fn(
            NativeFunctionDesc::new("game::set_level", NativeFunctionId::new(93))
                .param(
                    "player",
                    TypeHint::Host(TypeKey::new(TypeId::new(1), "Player")),
                )
                .param("level", TypeHint::Int)
                .returns(TypeHint::Null)
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public().reflect_callable(true)),
            |args, host| {
                let [OwnedValue::HostRef(player), OwnedValue::Int(level)] = args else {
                    return Ok(OwnedValue::Null);
                };
                host.access.set_path(
                    host.adapter,
                    HostPath::new(*player).field(FieldId::new(1)),
                    HostValue::Int(*level),
                    None,
                )?;
                Ok(OwnedValue::Null)
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
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
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
}

#[test]
fn engine_reflect_call_denies_effectful_native_functions_without_effect_permission() {
    let engine = Engine::builder()
        .capability(Capability::HostWrite)
        .register_host_native_fn(
            NativeFunctionDesc::new("game::set_level", NativeFunctionId::new(94))
                .param(
                    "player",
                    TypeHint::Host(TypeKey::new(TypeId::new(1), "Player")),
                )
                .param("level", TypeHint::Int)
                .returns(TypeHint::Null)
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public().reflect_callable(true)),
            |args, host| {
                let [OwnedValue::HostRef(player), OwnedValue::Int(level)] = args else {
                    return Ok(OwnedValue::Null);
                };
                host.access.set_path(
                    host.adapter,
                    HostPath::new(*player).field(FieldId::new(1)),
                    HostValue::Int(*level),
                    None,
                )?;
                Ok(OwnedValue::Null)
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
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert!(matches!(
        engine
            .into_vm()
            .run_program_with_host(&program, "main", &[OwnedValue::HostRef(host_ref)], &mut host),
        Err(error) if error.kind() == VmErrorKind::Reflect(
            ReflectErrorKind::FunctionEffectPermissionDenied {
                function: "game::set_level".to_owned(),
                permission: ReflectPermission::CallHostWriteMethods,
                source_span: None,
            }
        )
    ));
}
