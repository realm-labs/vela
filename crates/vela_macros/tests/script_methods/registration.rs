use super::*;

#[test]
fn script_macros_feed_engine_builder_registration() {
    let desc =
        <Player as vela_engine::schema::ScriptHostMethodMetadata>::script_host_method_descs()
            .into_iter()
            .find(|desc| desc.id == HostMethodId::new(7))
            .expect("method descriptor");
    let engine = Engine::builder()
        .register_host_schema::<Player>()
        .grant_permission("player.write")
        .register_native_method_fn(desc, |_, _, _| Ok(Value::Null))
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
            .register_host_schema::<Player>()
            .grant_permission("player.write"),
    )
    .build()
    .expect("engine should build from macro callable methods");
    let player = HostRef::new(HostTypeId::new(1001), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine.call_native_method(
            HostMethodId::new(8),
            &HostPath::new(player),
            &[Value::Int(13)],
            &mut host,
        ),
        Ok(Value::Int(13)),
    );
    assert_eq!(
        tx.patches()[0].path,
        HostPath::new(player).field(FieldId::new(1)),
    );
    assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Int(13)));
}

#[test]
fn script_methods_feed_stable_engine_registration_api() {
    let engine = Engine::builder()
        .register_host_schema::<Player>()
        .register_host_methods::<Player>()
        .grant_permission("player.write")
        .build()
        .expect("engine should build from macro host methods");
    let registry = engine.registry();
    let player_type = registry
        .type_by_name("Player")
        .expect("registered player type");
    assert_eq!(player_type.methods.len(), 6);
    assert_eq!(player_type.methods[0].name, "grant_exp");
    assert_eq!(player_type.methods[3].name, "sum_score");
    assert_eq!(player_type.methods[4].name, "sum6_score");

    let player = HostRef::new(HostTypeId::new(1001), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine.call_native_method(
            HostMethodId::new(10),
            &HostPath::new(player),
            &[
                Value::Int(1),
                Value::Int(2),
                Value::Int(3),
                Value::Int(4),
                Value::Int(5),
            ],
            &mut host,
        ),
        Ok(Value::Int(15)),
    );

    assert_eq!(
        engine.call_native_method(
            HostMethodId::new(12),
            &HostPath::new(player),
            &[
                Value::Int(1),
                Value::Int(2),
                Value::Int(3),
                Value::Int(4),
                Value::Int(5),
                Value::Int(6),
            ],
            &mut host,
        ),
        Ok(Value::Int(21)),
    );
    assert_eq!(
        tx.patches()[0].path,
        HostPath::new(player).field(FieldId::new(1)),
    );
    assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Int(15)));
    assert_eq!(
        tx.patches()[1].path,
        HostPath::new(player).field(FieldId::new(1)),
    );
    assert_eq!(tx.patches()[1].op, PatchOp::Set(HostValue::Int(21)));
}

#[test]
fn script_methods_generate_callable_result_native_registration() {
    let engine = Player::vela_register_native_method_fns(
        Engine::builder()
            .register_host_schema::<Player>()
            .grant_permission("player.write"),
    )
    .build()
    .expect("engine should build from macro callable methods");
    let player = HostRef::new(HostTypeId::new(1001), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine.call_native_method(
            HostMethodId::new(11),
            &HostPath::new(player),
            &[Value::Bool(true)],
            &mut host,
        ),
        Ok(Value::Enum {
            enum_name: "Result".to_owned(),
            variant: "Ok".to_owned(),
            fields: [("0".to_owned(), Value::Int(17))].into(),
        }),
    );
    assert_eq!(
        engine.call_native_method(
            HostMethodId::new(11),
            &HostPath::new(player),
            &[Value::Bool(false)],
            &mut host,
        ),
        Ok(Value::Enum {
            enum_name: "Result".to_owned(),
            variant: "Err".to_owned(),
            fields: [("0".to_owned(), Value::String("blocked".to_owned()))].into(),
        }),
    );
    assert!(tx.patches().is_empty());
}

#[test]
fn script_methods_generate_callable_option_native_registration() {
    let engine = Player::vela_register_native_method_fns(
        Engine::builder()
            .register_host_schema::<Player>()
            .grant_permission("player.write"),
    )
    .build()
    .expect("engine should build from macro callable methods");
    let player = HostRef::new(HostTypeId::new(1001), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine.call_native_method(
            HostMethodId::new(9),
            &HostPath::new(player),
            &[Value::Null],
            &mut host,
        ),
        Ok(Value::Null),
    );
    assert_eq!(
        engine.call_native_method(
            HostMethodId::new(9),
            &HostPath::new(player),
            &[Value::Int(4)],
            &mut host,
        ),
        Ok(Value::Int(5)),
    );
    assert!(tx.patches().is_empty());
}

#[test]
fn script_method_metadata_compiles_to_patch_tx_calls() {
    let engine = Engine::builder()
        .register_host_schema::<Player>()
        .register_host_method_metadata::<Player>()
        .build()
        .expect("engine should build from macro metadata");
    let root = unique_test_dir("script_method_metadata");
    std::fs::create_dir_all(&root).expect("create temp source dir");
    let source = root.join("main.lang");
    std::fs::write(
        &source,
        r#"
fn main(player: Player) {
    player.grant_exp(5);
    return 1;
}
"#,
    )
    .expect("write source");
    let program = engine.compile_file(&source).expect("compile source");
    let player = HostRef::new(HostTypeId::new(1001), HostObjectId::new(42), 1);
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
            &[Value::HostRef(player)],
            &mut host
        ),
        Ok(Value::Int(1)),
    );
    assert_eq!(
        tx.patches()[0].op,
        PatchOp::CallHostMethod {
            method: HostMethodId::new(7),
            args: vec![HostValue::Int(5)],
        },
    );
    std::fs::remove_dir_all(root).expect("clean temp source dir");
}
