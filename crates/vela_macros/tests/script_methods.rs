use vela_common::{FieldId, HostMethodId, HostTypeId, TypeId};
use vela_engine::{EffectSet, Engine, FunctionAccess, HostRef, NativeMethodDesc, TypeHint, Value};
use vela_macros::{ScriptHost, script_methods};
use vela_reflect::{FieldDesc, TypeDesc, TypeKey, TypeKind};

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
}

#[test]
fn script_methods_generates_native_method_metadata() {
    let owner = TypeKey::new(TypeId::new(1001), "Player");
    let descs = Player::vela_native_method_descs();

    assert_eq!(descs.len(), 1);
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
    let mut descs = <Player as vela_engine::ScriptHostMethodMetadata>::script_host_method_descs();
    let desc = descs.pop().expect("method descriptor");
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
