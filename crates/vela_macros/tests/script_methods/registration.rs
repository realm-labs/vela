use super::*;
use vela_bytecode::UnlinkedProgram;
use vela_vm::budget::ExecutionBudget;
use vela_vm::owned_value::OwnedValue;

fn run_linked_program_with_host(
    engine: &Engine,
    program: &UnlinkedProgram,
    args: &[OwnedValue],
    host: &mut HostExecution<'_>,
) -> VmResult<OwnedValue> {
    let linked = engine
        .link_program(program)
        .expect("script method metadata program should link");
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
fn script_macros_feed_engine_builder_registration() {
    let desc =
        <Player as vela_engine::schema::ScriptHostMethodMetadata>::script_host_method_descs()
            .into_iter()
            .find(|desc| desc.id == method_id("grant_exp"))
            .expect("method descriptor");
    let engine = Engine::builder()
        .register_host_type::<Player>()
        .capability(Capability::HostWrite)
        .register_native_method_fn(desc, |_, _, _| Ok(OwnedValue::Null))
        .build()
        .expect("engine should build from macro metadata");

    let registry = engine.registry();
    let player = registry.type_by_name("Player").expect("registered player");
    assert_eq!(player.fields.len(), 1);
    assert_eq!(player.methods.len(), 1);
    assert_eq!(player.methods[0].name, "grant_exp");
    assert!(player.methods[0].effects.writes_host);
    assert!(player.methods[0].access.required_permissions().is_empty());
}

#[test]
fn script_methods_generate_callable_native_registration() {
    let engine = Player::vela_register_native_method_fns(
        Engine::builder()
            .register_host_type::<Player>()
            .capability(Capability::HostRead)
            .capability(Capability::HostWrite),
    )
    .build()
    .expect("engine should build from macro callable methods");
    let player = HostRef::new(Player::vela_host_type_id(), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        engine.call_native_method(
            method_id("grant_score"),
            &HostPath::new(player),
            &[OwnedValue::Scalar(vela_common::ScalarValue::I64(13))],
            &mut host,
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(13))),
    );
}

#[test]
fn script_methods_feed_stable_engine_registration_api() {
    let generated_schema = Player::vela_host_type_desc();
    let generated_methods = Player::vela_native_method_descs();
    let engine = Engine::builder()
        .register_script_host::<Player>()
        .capability(Capability::HostWrite)
        .build()
        .expect("engine should build from macro host methods");
    let registry = engine.registry();
    let player_type = registry
        .type_by_name("Player")
        .expect("registered player type");
    assert_eq!(player_type.key, generated_schema.key);
    assert_eq!(player_type.kind, generated_schema.kind);
    assert_eq!(player_type.schema_hash, generated_schema.schema_hash);
    assert_eq!(player_type.host_type_id, generated_schema.host_type_id);
    assert_eq!(player_type.fields, generated_schema.fields);
    assert_eq!(player_type.attrs, generated_schema.attrs);
    assert_eq!(player_type.methods.len(), generated_methods.len());
    for (registered, generated) in player_type.methods.iter().zip(generated_methods.iter()) {
        assert_registered_method_matches_native_desc(registered, generated);
    }
    assert_eq!(player_type.methods[0].name, "grant_exp");
    assert_eq!(player_type.methods[3].name, "sum_score");
    assert_eq!(player_type.methods[4].name, "sum6_score");

    let player = HostRef::new(Player::vela_host_type_id(), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        engine.call_native_method(
            method_id("sum_score"),
            &HostPath::new(player),
            &[
                OwnedValue::Scalar(vela_common::ScalarValue::I64(1)),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(2)),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(3)),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(4)),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(5)),
            ],
            &mut host,
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(15))),
    );

    assert_eq!(
        engine.call_native_method(
            method_id("sum6_score"),
            &HostPath::new(player),
            &[
                OwnedValue::Scalar(vela_common::ScalarValue::I64(1)),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(2)),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(3)),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(4)),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(5)),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(6)),
            ],
            &mut host,
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(21))),
    );
}

fn assert_registered_method_matches_native_desc(
    registered: &vela_reflect::registry::MethodDesc,
    generated: &NativeMethodDesc,
) {
    assert_eq!(registered.id, generated.id);
    assert_eq!(registered.name, generated.name);
    assert_eq!(
        registered.return_type.as_deref(),
        Some(type_hint_name(&generated.returns).as_str())
    );
    assert_eq!(
        registered.effects.reads_host,
        generated.effects.reads_host()
    );
    assert_eq!(
        registered.effects.writes_host,
        generated.effects.writes_host()
    );
    assert_eq!(
        registered.effects.emits_events,
        generated.effects.emits_events()
    );
    assert_eq!(registered.access.public, generated.access.public);
    assert_eq!(
        registered.access.reflect_callable,
        generated.access.reflect_callable
    );
    assert_eq!(registered.docs, generated.docs);
    assert_eq!(registered.attrs, generated.attrs);
    assert_eq!(registered.source_span, generated.source_span);
    assert_eq!(registered.params.len(), generated.params.len());
    for (registered_param, generated_param) in registered.params.iter().zip(generated.params.iter())
    {
        assert_eq!(registered_param.name, generated_param.name);
        assert_eq!(
            registered_param.type_hint.as_deref(),
            Some(type_hint_name(&generated_param.hint).as_str())
        );
        assert!(!registered_param.has_default);
    }
}

fn type_hint_name(hint: &TypeHint) -> String {
    match hint {
        TypeHint::Any => "Any".to_owned(),
        TypeHint::Primitive(tag) => tag.name().to_owned(),
        TypeHint::Array => "Array".to_owned(),
        TypeHint::ArrayOf(element) => format!("Array<{}>", type_hint_name(element)),
        TypeHint::Map => "Map".to_owned(),
        TypeHint::MapOf { key, value } => {
            format!("Map<{}, {}>", type_hint_name(key), type_hint_name(value))
        }
        TypeHint::Set => "Set".to_owned(),
        TypeHint::SetOf(element) => format!("Set<{}>", type_hint_name(element)),
        TypeHint::PathProxy => "path_proxy".to_owned(),
        TypeHint::Record(key) | TypeHint::Enum(key) | TypeHint::Host(key) => key.name.clone(),
        TypeHint::Trait(name) => name.clone(),
        TypeHint::Function => "Function".to_owned(),
        TypeHint::Iterator => "Iterator".to_owned(),
        TypeHint::IteratorOf(item) => format!("Iterator<{}>", type_hint_name(item)),
        TypeHint::OptionOf(payload) => format!("Option<{}>", type_hint_name(payload)),
        TypeHint::ResultOf { ok, err } => {
            format!("Result<{}, {}>", type_hint_name(ok), type_hint_name(err))
        }
    }
}

#[test]
fn script_methods_generate_callable_result_native_registration() {
    let engine = Player::vela_register_native_method_fns(
        Engine::builder()
            .register_host_type::<Player>()
            .capability(Capability::HostRead)
            .capability(Capability::HostWrite),
    )
    .build()
    .expect("engine should build from macro callable methods");
    let player = HostRef::new(Player::vela_host_type_id(), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        engine.call_native_method(
            method_id("checked_preview"),
            &HostPath::new(player),
            &[OwnedValue::Bool(true)],
            &mut host,
        ),
        Ok(OwnedValue::Enum {
            enum_name: "Result".to_owned(),
            variant: "Ok".to_owned(),
            fields: [(
                "0".to_owned(),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(17))
            )]
            .into(),
        }),
    );
    assert_eq!(
        engine.call_native_method(
            method_id("checked_preview"),
            &HostPath::new(player),
            &[OwnedValue::Bool(false)],
            &mut host,
        ),
        Ok(OwnedValue::Enum {
            enum_name: "Result".to_owned(),
            variant: "Err".to_owned(),
            fields: [("0".to_owned(), OwnedValue::String("blocked".to_owned()))].into(),
        }),
    );
}

#[test]
fn script_methods_generate_callable_option_native_registration() {
    let engine = Player::vela_register_native_method_fns(
        Engine::builder()
            .register_host_type::<Player>()
            .capability(Capability::HostRead)
            .capability(Capability::HostWrite),
    )
    .build()
    .expect("engine should build from macro callable methods");
    let player = HostRef::new(Player::vela_host_type_id(), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        engine.call_native_method(
            method_id("preview_bonus"),
            &HostPath::new(player),
            &[OwnedValue::Null],
            &mut host,
        ),
        Ok(OwnedValue::Null),
    );
    assert_eq!(
        engine.call_native_method(
            method_id("preview_bonus"),
            &HostPath::new(player),
            &[OwnedValue::Scalar(vela_common::ScalarValue::I64(4))],
            &mut host,
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(5))),
    );
}

#[test]
fn script_method_metadata_compiles_to_host_access_calls() {
    let engine = Engine::builder()
        .register_host_type::<Player>()
        .register_host_method_metadata::<Player>()
        .build()
        .expect("engine should build from macro metadata");
    let program = compile_source!(
        engine,
        r#"
fn main(player: Player) {
    player.grant_exp(5);
    return 1;
}
"#,
        "compile source"
    );
    let player = HostRef::new(Player::vela_host_type_id(), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        run_linked_program_with_host(&engine, &program, &[OwnedValue::HostRef(player)], &mut host),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1))),
    );
}
