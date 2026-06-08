use super::*;
use vela_vm::owned_value::OwnedValue;

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
    reflect::name(player);
    reflect::kind(player);
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
        Err(error) if error.kind() == VmErrorKind::Reflect(ReflectErrorKind::LookupBudgetExceeded {
            limit: 1
        })
    ));
}

#[test]
fn engine_reflect_call_denies_native_methods_without_effect_permission() {
    let method = HostMethodId::new(6);
    let owner = TypeKey::new(TypeId::new(1), "Player");
    let engine = Engine::builder()
        .capability(Capability::HostWrite)
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .register_native_method_fn(
            NativeMethodDesc::new(owner, method, "grant_exp")
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public().reflect_callable(true)),
            |_, _, _| Ok(OwnedValue::Null),
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
    reflect::call(player, "grant_exp", 10);
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
        Err(error) if error.kind() == VmErrorKind::Reflect(ReflectErrorKind::MethodEffectPermissionDenied {
            method: "grant_exp".to_owned(),
            permission: ReflectPermission::CallHostWriteMethods,
            source_span: None,
        })
    ));
}

#[test]
fn engine_reflect_call_records_approved_native_methods() {
    let method = HostMethodId::new(6);
    let owner = TypeKey::new(TypeId::new(1), "Player");
    let engine = Engine::builder()
        .capability(Capability::HostWrite)
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .register_native_method_fn(
            NativeMethodDesc::new(owner, method, "grant_exp")
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public().reflect_callable(true)),
            |_, _, _| Ok(OwnedValue::Null),
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
    reflect::call(player, "grant_exp", 10);
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
