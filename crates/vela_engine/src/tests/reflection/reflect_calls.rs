use super::*;
use vela_bytecode::UnlinkedProgram;
use vela_vm::budget::ExecutionBudget;
use vela_vm::error::VmResult;
use vela_vm::owned_value::OwnedValue;

fn run_linked_program_with_host(
    engine: &Engine,
    program: &UnlinkedProgram,
    args: &[OwnedValue],
    host: &mut HostExecution<'_>,
) -> VmResult<OwnedValue> {
    let linked = engine
        .link_program(program)
        .expect("engine reflection call test program should link");
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
fn engine_reflect_call_invokes_reflect_callable_native_functions() {
    let engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::add", NativeFunctionId::new(91))
                .param("lhs", TypeHint::i64())
                .param("rhs", TypeHint::i64())
                .returns(TypeHint::i64())
                .access(FunctionAccess::public().reflect_callable(true)),
            |args| {
                let [
                    OwnedValue::Scalar(vela_common::ScalarValue::I64(lhs)),
                    OwnedValue::Scalar(vela_common::ScalarValue::I64(rhs)),
                ] = args
                else {
                    return Ok(OwnedValue::Null);
                };
                Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(lhs + rhs)))
            },
        )
        .reflection_permissions(ReflectPermissionSet::all())
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
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
        run_linked_program_with_host(&engine, &program, &[], &mut host),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(5)))
    );
}

#[test]
fn engine_reflect_call_checks_typed_native_parameter_contracts() {
    let engine = Engine::builder()
        .register_typed_native_fn::<(i64,), _>(
            NativeFunctionDesc::new("game::double", NativeFunctionId::new(96))
                .param("value", TypeHint::i64())
                .returns(TypeHint::i64())
                .access(FunctionAccess::public().reflect_callable(true)),
            |value: i64| value * 2,
        )
        .reflection_permissions(ReflectPermissionSet::all())
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main() {
    let double = reflect::function("game::double");
    return reflect::call(double, "bad");
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
        run_linked_program_with_host(&engine, &program, &[], &mut host),
        Err(error) if matches!(error.kind(), VmErrorKind::TypeMismatch { .. })
    ));
}

#[test]
fn engine_reflect_call_requires_call_permission_for_function_descriptors() {
    let engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::add", NativeFunctionId::new(95))
                .param("lhs", TypeHint::i64())
                .param("rhs", TypeHint::i64())
                .returns(TypeHint::i64())
                .access(FunctionAccess::public().reflect_callable(true)),
            |_| Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(0))),
        )
        .reflection_permissions(ReflectPermissionSet::new().with(ReflectPermission::ReadTypeInfo))
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
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
        run_linked_program_with_host(&engine, &program, &[], &mut host),
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
                .param("lhs", TypeHint::i64())
                .param("rhs", TypeHint::i64())
                .returns(TypeHint::i64()),
            |_| Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(0))),
        )
        .reflection_permissions(ReflectPermissionSet::all())
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
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
        run_linked_program_with_host(&engine, &program, &[], &mut host),
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
                .param("level", TypeHint::i64())
                .returns(TypeHint::null())
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public().reflect_callable(true)),
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
        .reflection_permissions(ReflectPermissionSet::all())
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
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
fn engine_reflect_call_denies_effectful_native_functions_without_effect_permission() {
    let engine = Engine::builder()
        .capability(Capability::HostWrite)
        .register_host_native_fn(
            NativeFunctionDesc::new("game::set_level", NativeFunctionId::new(94))
                .param(
                    "player",
                    TypeHint::Host(TypeKey::new(TypeId::new(1), "Player")),
                )
                .param("level", TypeHint::i64())
                .returns(TypeHint::null())
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public().reflect_callable(true)),
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
        .reflection_permissions(
            ReflectPermissionSet::new()
                .with(ReflectPermission::ReadTypeInfo)
                .with(ReflectPermission::CallMethods),
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
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
        run_linked_program_with_host(
            &engine,
            &program,
            &[OwnedValue::HostRef(host_ref)],
            &mut host,
        ),
        Err(error) if error.kind() == VmErrorKind::Reflect(
            ReflectErrorKind::FunctionEffectPermissionDenied {
                function: "game::set_level".to_owned(),
                permission: ReflectPermission::CallHostWriteMethods,
                source_span: None,
            }
        )
    ));
}
