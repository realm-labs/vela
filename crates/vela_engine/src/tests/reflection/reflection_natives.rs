use super::*;
use vela_vm::owned_value::OwnedValue;

#[test]
fn engine_builder_registers_module_reflection_metadata() {
    let engine = Engine::builder()
        .register_module(
            ModuleDesc::new("game::reward")
                .docs("Reward module.")
                .attr("domain", "gameplay"),
        )
        .register_native_fn(
            NativeFunctionDesc::new("game::reward::grant", NativeFunctionId::new(221))
                .returns(TypeHint::Bool),
            |_| Ok(OwnedValue::Bool(true)),
        )
        .build()
        .expect("engine should build");

    let registry = engine.registry();
    let module = registry
        .module_by_name("game::reward")
        .expect("registered module metadata");
    assert_eq!(module.docs.as_deref(), Some("Reward module."));
    assert_eq!(module.attrs.get("domain"), Some("gameplay"));
    assert_eq!(module.exports.len(), 1);
    assert_eq!(module.exports[0].name, "game::reward::grant");
}

#[test]
fn engine_registers_native_method_source_span_metadata() {
    let source_span = Span::new(SourceId::new(8), 30, 42);
    let owner = TypeKey::new(TypeId::new(1), "Player");
    let engine = Engine::builder()
        .register_type(player_type(TypeId::new(1), HostTypeId::new(1)))
        .register_native_method_fn(
            NativeMethodDesc::new(owner, HostMethodId::new(51), "grant_exp")
                .param("amount", TypeHint::Int)
                .returns(TypeHint::Int)
                .effects(EffectSet::host_write())
                .source_span(source_span),
            |_, _, _| Ok(OwnedValue::Int(0)),
        )
        .build()
        .expect("engine should build");
    let registry = engine.registry();
    let method = registry
        .type_by_name("Player")
        .and_then(|desc| {
            desc.methods
                .iter()
                .find(|method| method.name == "grant_exp")
        })
        .expect("native method metadata");

    assert_eq!(method.source_span, Some(source_span));

    let method_abi = MethodAbi::from_method("Player", method);
    assert_eq!(method_abi.source_span, Some(source_span));
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
    if reflect::name(player) == "Player" && reflect::get(player, "level") == 7 {
        reflect::set(player, "level", 8);
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
            .run_program_with_host(&program, "main", &[OwnedValue::HostRef(host_ref)], &mut host),
        Err(error) if error.kind == VmErrorKind::Reflect(ReflectErrorKind::PermissionDenied {
            permission: ReflectPermission::WriteValueFields
        })
    ));
    assert!(tx.is_empty());
}

#[test]
fn engine_compiler_keeps_reflect_module_calls_off_host_method_lowering() {
    let engine = Engine::builder()
        .register_type(
            player_type(TypeId::new(1), HostTypeId::new(1))
                .method(MethodDesc::new(HostMethodId::new(9), "set")),
        )
        .reflection_policy(vela_reflect::permissions::ReflectPolicy::all())
        .build()
        .expect("engine should build");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    reflect::set(player, "level", 12);
    return reflect::get(player, "level");
}
"#,
        &engine.compiler_options(),
    )
    .expect("reflect::set should compile as a native module call");
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

    assert_eq!(
        engine.into_vm().run_program_with_host(
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host
        ),
        Ok(OwnedValue::Int(12))
    );
    assert_eq!(tx.mutation_count(), 1);
}

#[test]
fn public_reflection_metadata_lists_do_not_need_engine_permissions() {
    let engine = Engine::builder()
        .register_type(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
                .host_type(HostTypeId::new(1))
                .field(FieldDesc::new(FieldId::new(1), "secret_level")),
        )
        .register_native_fn(
            NativeFunctionDesc::new("game::secret_bonus", NativeFunctionId::new(77))
                .returns(TypeHint::Int)
                .access(FunctionAccess::public().reflect_callable(true)),
            |_| Ok(OwnedValue::Int(5)),
        )
        .reflection_permissions(ReflectPermissionSet::new().with(ReflectPermission::ReadTypeInfo))
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    let fields = reflect::fields();
    let functions = reflect::functions();
    if fields.len() == 1
        && fields[0].owner == "Player"
        && fields[0].name == "secret_level"
        && functions.len() == 1
        && functions[0].name == "game::secret_bonus" {
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
        Ok(OwnedValue::Int(1))
    );
    assert!(tx.is_empty());
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
            NativeFunctionDesc::new("game::secret_bonus", NativeFunctionId::new(77))
                .returns(TypeHint::Int)
                .access(FunctionAccess::public().reflect_callable(true)),
            |_| Ok(OwnedValue::Int(5)),
        )
        .reflection_permissions(ReflectPermissionSet::new().with(ReflectPermission::ReadTypeInfo))
        .build()
        .expect("engine should build");
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    return reflect::fields().len() + reflect::functions().len();
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
        Ok(OwnedValue::Int(1))
    );
    assert!(tx.is_empty());
}
