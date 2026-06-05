use super::*;
use vela_vm::owned_value::OwnedValue;

#[test]
fn script_macros_feed_engine_builder_registration() {
    let desc =
        <Player as vela_engine::schema::ScriptHostMethodMetadata>::script_host_method_descs()
            .into_iter()
            .find(|desc| desc.id == method_id("grant_exp"))
            .expect("method descriptor");
    let engine = Engine::builder()
        .register_host_type::<Player>()
        .grant_permission("player.write")
        .register_native_method_fn(desc, |_, _, _| Ok(OwnedValue::Null))
        .build()
        .expect("engine should build from macro metadata");

    let registry = engine.registry();
    let player = registry.type_by_name("Player").expect("registered player");
    assert_eq!(player.fields.len(), 1);
    assert_eq!(player.methods.len(), 1);
    assert_eq!(player.methods[0].name, "grant_exp");
    assert!(player.methods[0].effects.writes_host);
    assert_eq!(
        player.methods[0].access.required_permissions(),
        &["player.write".to_owned()],
    );
}

#[test]
fn script_methods_generate_callable_native_registration() {
    let engine = Player::vela_register_native_method_fns(
        Engine::builder()
            .register_host_type::<Player>()
            .grant_permission("player.write"),
    )
    .build()
    .expect("engine should build from macro callable methods");
    let player = HostRef::new(Player::vela_host_type_id(), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine.call_native_method(
            method_id("grant_score"),
            &HostPath::new(player),
            &[OwnedValue::Int(13)],
            &mut host,
        ),
        Ok(OwnedValue::Int(13)),
    );
    assert_eq!(
        tx.patches()[0].path,
        HostPath::new(player).field(Player::vela_field_id_level()),
    );
    assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Int(13)));
}

#[test]
fn script_methods_feed_stable_engine_registration_api() {
    let generated_schema = Player::vela_host_type_desc();
    let generated_methods = Player::vela_native_method_descs();
    let engine = Engine::builder()
        .register_script_host::<Player>()
        .grant_permission("player.write")
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
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine.call_native_method(
            method_id("sum_score"),
            &HostPath::new(player),
            &[
                OwnedValue::Int(1),
                OwnedValue::Int(2),
                OwnedValue::Int(3),
                OwnedValue::Int(4),
                OwnedValue::Int(5),
            ],
            &mut host,
        ),
        Ok(OwnedValue::Int(15)),
    );

    assert_eq!(
        engine.call_native_method(
            method_id("sum6_score"),
            &HostPath::new(player),
            &[
                OwnedValue::Int(1),
                OwnedValue::Int(2),
                OwnedValue::Int(3),
                OwnedValue::Int(4),
                OwnedValue::Int(5),
                OwnedValue::Int(6),
            ],
            &mut host,
        ),
        Ok(OwnedValue::Int(21)),
    );
    assert_eq!(
        tx.patches()[0].path,
        HostPath::new(player).field(Player::vela_field_id_level()),
    );
    assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Int(15)));
    assert_eq!(
        tx.patches()[1].path,
        HostPath::new(player).field(Player::vela_field_id_level()),
    );
    assert_eq!(tx.patches()[1].op, PatchOp::Set(HostValue::Int(21)));
}

fn assert_registered_method_matches_native_desc(
    registered: &vela_reflect::registry::MethodDesc,
    generated: &NativeMethodDesc,
) {
    assert_eq!(registered.id, generated.id);
    assert_eq!(registered.name, generated.name);
    assert_eq!(
        registered.return_type.as_deref(),
        Some(type_hint_name(&generated.returns))
    );
    assert_eq!(registered.effects.reads_host, generated.effects.reads_host);
    assert_eq!(
        registered.effects.writes_host,
        generated.effects.writes_host
    );
    assert_eq!(
        registered.effects.emits_events,
        generated.effects.emits_events
    );
    assert_eq!(registered.access.public, generated.access.public);
    assert_eq!(
        registered.access.reflect_callable,
        generated.access.reflect_callable
    );
    assert_eq!(
        registered.access.required_permissions(),
        generated
            .access
            .required_permissions
            .iter()
            .map(str::to_owned)
            .collect::<Vec<_>>()
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
            Some(type_hint_name(&generated_param.hint))
        );
        assert!(!registered_param.has_default);
    }
}

fn type_hint_name(hint: &TypeHint) -> &str {
    match hint {
        TypeHint::Any => "any",
        TypeHint::Null => "null",
        TypeHint::Bool => "bool",
        TypeHint::Int => "int",
        TypeHint::Float => "float",
        TypeHint::String => "string",
        TypeHint::Array => "array",
        TypeHint::Map => "map",
        TypeHint::Set => "set",
        TypeHint::PathProxy => "path_proxy",
        TypeHint::Record(key) | TypeHint::Enum(key) | TypeHint::Host(key) => &key.name,
        TypeHint::Trait(name) => name,
        TypeHint::Function => "function",
    }
}

#[test]
fn script_methods_generate_callable_result_native_registration() {
    let engine = Player::vela_register_native_method_fns(
        Engine::builder()
            .register_host_type::<Player>()
            .grant_permission("player.write"),
    )
    .build()
    .expect("engine should build from macro callable methods");
    let player = HostRef::new(Player::vela_host_type_id(), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
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
            fields: [("0".to_owned(), OwnedValue::Int(17))].into(),
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
    assert!(tx.patches().is_empty());
}

#[test]
fn script_methods_generate_callable_option_native_registration() {
    let engine = Player::vela_register_native_method_fns(
        Engine::builder()
            .register_host_type::<Player>()
            .grant_permission("player.write"),
    )
    .build()
    .expect("engine should build from macro callable methods");
    let player = HostRef::new(Player::vela_host_type_id(), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
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
            &[OwnedValue::Int(4)],
            &mut host,
        ),
        Ok(OwnedValue::Int(5)),
    );
    assert!(tx.patches().is_empty());
}

#[test]
fn script_method_metadata_compiles_to_patch_tx_calls() {
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
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine.into_vm().run_program_with_host(
            &program,
            "main",
            &[OwnedValue::HostRef(player)],
            &mut host
        ),
        Ok(OwnedValue::Int(1)),
    );
    assert_eq!(
        tx.patches()[0].op,
        PatchOp::CallHostMethod {
            method: method_id("grant_exp"),
            args: vec![HostValue::Int(5)],
        },
    );
}
