#![allow(clippy::result_large_err)]

use vela_common::{FieldId, HostMethodId, HostObjectId, HostTypeId, TypeId};
use vela_engine::{EffectSet, Engine, FunctionAccess, HostRef, NativeMethodDesc, TypeHint, Value};
use vela_host::{HostPath, HostValue, MockStateAdapter, PatchOp, PatchTx};
use vela_macros::{ScriptHost, script_methods};
use vela_reflect::{FieldDesc, TypeDesc, TypeKey, TypeKind};
use vela_vm::{HostExecution, VmResult};

#[allow(dead_code)]
#[derive(ScriptHost)]
#[script(id = 1001, name = "Player")]
struct Player {
    #[script(get, set, id = 1)]
    level: u32,
}

#[allow(dead_code)]
#[script_methods]
impl Player {
    /// Grants copied experience through the host patch path.
    #[script_method(
        id = 7,
        effect = "write_host",
        permission = "player.write",
        reflect = true
    )]
    pub fn grant_exp(
        _ctx: &mut vela_engine::NativeCallContext<'_, '_>,
        _player: HostRef,
        _amount: i64,
    ) {
    }

    /// Grants copied score through a callable native method.
    #[script_method(
        id = 8,
        effect = "write_host",
        permission = "player.write",
        reflect = true
    )]
    pub fn grant_score(
        receiver: &HostPath,
        host: &mut HostExecution<'_>,
        amount: i64,
    ) -> VmResult<i64> {
        host.tx.set_path(
            receiver.clone().field(FieldId::new(1)),
            HostValue::Int(amount),
            None,
        )?;
        Ok(amount)
    }

    /// Previews an optional copied bonus through a callable native method.
    #[script_method(id = 9, effect = "read_host", reflect = true)]
    pub fn preview_bonus(
        _receiver: &HostPath,
        _host: &mut HostExecution<'_>,
        bonus: Option<i64>,
    ) -> Option<i64> {
        bonus.map(|bonus| bonus + 1)
    }

    /// Sums five copied method values through a callable native method.
    #[script_method(
        id = 10,
        effect = "write_host",
        permission = "player.write",
        reflect = true
    )]
    pub fn sum_score(
        receiver: &HostPath,
        host: &mut HostExecution<'_>,
        a: i64,
        b: i64,
        c: i64,
        d: i64,
        e: i64,
    ) -> VmResult<i64> {
        let total = a + b + c + d + e;
        host.tx.set_path(
            receiver.clone().field(FieldId::new(1)),
            HostValue::Int(total),
            None,
        )?;
        Ok(total)
    }

    /// Sums six copied method values through a callable native method.
    #[allow(clippy::too_many_arguments)]
    #[script_method(
        id = 12,
        effect = "write_host",
        permission = "player.write",
        reflect = true
    )]
    pub fn sum6_score(
        receiver: &HostPath,
        host: &mut HostExecution<'_>,
        a: i64,
        b: i64,
        c: i64,
        d: i64,
        e: i64,
        f: i64,
    ) -> VmResult<i64> {
        let total = a + b + c + d + e + f;
        host.tx.set_path(
            receiver.clone().field(FieldId::new(1)),
            HostValue::Int(total),
            None,
        )?;
        Ok(total)
    }

    /// Previews a dynamic copied Result through a callable native method.
    #[script_method(id = 11, effect = "read_host", reflect = true)]
    pub fn checked_preview(
        _receiver: &HostPath,
        _host: &mut HostExecution<'_>,
        ok: bool,
    ) -> std::result::Result<i64, String> {
        if ok {
            Ok(17)
        } else {
            Err("blocked".to_owned())
        }
    }
}

#[test]
fn script_methods_generates_native_method_metadata() {
    let owner = TypeKey::new(TypeId::new(1001), "Player");
    let descs = Player::vela_native_method_descs();

    assert_eq!(descs.len(), 6);
    assert_eq!(
        descs[0],
        NativeMethodDesc::new(owner.clone(), HostMethodId::new(7), "grant_exp")
            .param("amount", TypeHint::Int)
            .returns(TypeHint::Null)
            .effects(EffectSet::host_write())
            .access(
                FunctionAccess::public()
                    .reflect_callable(true)
                    .require_permission("player.write"),
            )
            .docs("Grants copied experience through the host patch path."),
    );
    assert_eq!(
        descs[1],
        NativeMethodDesc::new(owner.clone(), HostMethodId::new(8), "grant_score")
            .param("amount", TypeHint::Int)
            .returns(TypeHint::Int)
            .effects(EffectSet::host_write())
            .access(
                FunctionAccess::public()
                    .reflect_callable(true)
                    .require_permission("player.write"),
            )
            .docs("Grants copied score through a callable native method."),
    );
    assert_eq!(
        descs[2],
        NativeMethodDesc::new(owner.clone(), HostMethodId::new(9), "preview_bonus")
            .param("bonus", TypeHint::Int)
            .returns(TypeHint::Int)
            .effects(EffectSet::host_read())
            .access(FunctionAccess::public().reflect_callable(true))
            .docs("Previews an optional copied bonus through a callable native method."),
    );
    assert_eq!(
        descs[3],
        NativeMethodDesc::new(owner.clone(), HostMethodId::new(10), "sum_score")
            .param("a", TypeHint::Int)
            .param("b", TypeHint::Int)
            .param("c", TypeHint::Int)
            .param("d", TypeHint::Int)
            .param("e", TypeHint::Int)
            .returns(TypeHint::Int)
            .effects(EffectSet::host_write())
            .access(
                FunctionAccess::public()
                    .reflect_callable(true)
                    .require_permission("player.write"),
            )
            .docs("Sums five copied method values through a callable native method."),
    );
    assert_eq!(
        descs[4],
        NativeMethodDesc::new(owner.clone(), HostMethodId::new(12), "sum6_score")
            .param("a", TypeHint::Int)
            .param("b", TypeHint::Int)
            .param("c", TypeHint::Int)
            .param("d", TypeHint::Int)
            .param("e", TypeHint::Int)
            .param("f", TypeHint::Int)
            .returns(TypeHint::Int)
            .effects(EffectSet::host_write())
            .access(
                FunctionAccess::public()
                    .reflect_callable(true)
                    .require_permission("player.write"),
            )
            .docs("Sums six copied method values through a callable native method."),
    );
    assert_eq!(
        descs[5],
        NativeMethodDesc::new(owner.clone(), HostMethodId::new(11), "checked_preview")
            .param("ok", TypeHint::Bool)
            .returns(TypeHint::Any)
            .effects(EffectSet::host_read())
            .access(FunctionAccess::public().reflect_callable(true))
            .docs("Previews a dynamic copied Result through a callable native method."),
    );
    assert_eq!(descs[0].owner, Player::vela_host_type_desc().key);
    assert_eq!(
        <Player as vela_engine::ScriptHostMethodMetadata>::script_host_method_descs(),
        descs,
    );
}

#[test]
fn script_methods_coexists_with_host_schema_metadata() {
    let schema = Player::vela_host_type_desc();
    assert_eq!(
        schema,
        TypeDesc::new(TypeKey::new(TypeId::new(1001), "Player"))
            .kind(TypeKind::Host)
            .schema_hash(schema.schema_hash.expect("schema hash should be generated"))
            .host_type(HostTypeId::new(1001))
            .field(
                FieldDesc::new(FieldId::new(1), "level")
                    .access(
                        vela_reflect::FieldAccess::new()
                            .readable(true)
                            .writable(true)
                            .reflect_readable(true)
                            .reflect_writable(true),
                    )
                    .attr("rust_name", "level")
                    .type_hint("int"),
            ),
    );
}

#[test]
fn script_macros_feed_engine_builder_registration() {
    let desc = <Player as vela_engine::ScriptHostMethodMetadata>::script_host_method_descs()
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

fn unique_test_dir(name: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "vela_macros_{name}_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time before epoch")
            .as_nanos()
    ));
    path
}
