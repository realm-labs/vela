use vela_common::{FieldId, HostObjectId, TypeId, stable_id};
use vela_host::path::{HostPath, HostRef};
use vela_host::proxy::PathProxy;
use vela_macros::{ScriptHost, ScriptReflect};
use vela_reflect::access::FieldAccess;
use vela_reflect::registry::{FieldDesc, TraitDesc, TypeDesc, TypeKey, TypeKind};

#[allow(dead_code)]
#[derive(ScriptHost, ScriptReflect)]
#[script(
    path = "game.player.Player",
    docs = "Player host schema.",
    attr = "domain=gameplay",
    implements = "Damageable"
)]
struct Player {
    #[script(get, set, hint = "int", docs = "Current level.", attr = "unit=level")]
    level: u32,
    #[script(get, name = "display_name", permission = "player.profile")]
    name: String,
    #[script(skip)]
    internal_revision: u64,
}

#[allow(dead_code)]
#[derive(ScriptHost)]
#[script(path = "game.reward.RewardConfig")]
struct RewardConfigA {
    #[script(get, hint = "string")]
    item_id: String,
    #[script(get, hint = "int")]
    count: i64,
}

#[allow(dead_code)]
#[derive(ScriptHost)]
#[script(module = "game.reward", name = "RewardConfig")]
struct RewardConfigB {
    #[script(get, hint = "int")]
    count: i64,
    #[script(get, hint = "string")]
    item_id: String,
}

#[allow(dead_code)]
#[derive(ScriptHost)]
#[script(
    path = "game.reward.RewardConfigV2",
    alias = "game.reward.RewardConfig"
)]
struct RewardConfigRenamed {
    #[script(get, hint = "string", alias = "item_id")]
    item_key: String,
    #[script(get, hint = "int")]
    count: i64,
}

#[test]
fn script_host_derive_generates_type_metadata() {
    let desc = Player::vela_host_type_desc();
    let expected = TypeDesc::new(TypeKey::new(Player::vela_type_id(), "Player"))
        .kind(TypeKind::Host)
        .schema_hash(desc.schema_hash.expect("schema hash should be generated"))
        .host_type(Player::vela_host_type_id())
        .attr("module", "game.player")
        .attr("domain", "gameplay")
        .docs("Player host schema.")
        .trait_impl(TraitDesc::new("Damageable"))
        .field(
            FieldDesc::new(Player::vela_field_id_level(), "level")
                .access(
                    FieldAccess::new()
                        .readable(true)
                        .writable(true)
                        .reflect_readable(true)
                        .reflect_writable(true),
                )
                .attr("rust_name", "level")
                .attr("unit", "level")
                .type_hint("int")
                .docs("Current level."),
        )
        .field(
            FieldDesc::new(Player::vela_field_id_name(), "display_name")
                .access(
                    FieldAccess::new()
                        .readable(true)
                        .writable(false)
                        .reflect_readable(true)
                        .reflect_writable(false)
                        .require_permission("player.profile"),
                )
                .attr("rust_name", "name")
                .type_hint("string"),
        );

    assert_eq!(desc, expected);
    assert_eq!(desc.kind, TypeKind::Host);
    assert_eq!(
        Player::vela_type_id(),
        TypeId::new(stable_id("host_type", "", "game.player.Player")),
    );
    assert_eq!(desc.host_type_id, Some(Player::vela_host_type_id()));
    assert_eq!(desc.attrs.get("module"), Some("game.player"));
    assert_eq!(desc.attrs.get("domain"), Some("gameplay"));
    assert_eq!(desc.traits, vec![TraitDesc::new("Damageable")]);
    assert_eq!(desc.fields[0].attrs.get("unit"), Some("level"));
    assert_eq!(desc.fields.len(), 2);
    assert_eq!(
        desc.fields[1].access.required_permissions(),
        &["player.profile".to_owned()]
    );
    assert_eq!(
        <Player as vela_engine::schema::ScriptHostSchema>::script_host_type_desc(),
        desc,
    );
}

#[test]
fn script_host_derive_generates_field_helpers() {
    let player = HostRef::new(Player::vela_host_type_id(), HostObjectId::new(42), 3);

    assert_eq!(
        Player::vela_field_id_level(),
        FieldId::new(stable_id("host_field", "game.player.Player", "level")),
    );
    assert_eq!(
        Player::vela_field_id_name(),
        FieldId::new(stable_id(
            "host_field",
            "game.player.Player",
            "display_name"
        )),
    );
    assert_eq!(
        Player::vela_field_path_level(player),
        HostPath::new(player).field(Player::vela_field_id_level()),
    );
    assert_eq!(
        Player::vela_field_path_name(player),
        HostPath::new(player).field(Player::vela_field_id_name()),
    );
    assert_eq!(
        Player::vela_field_proxy_level(player),
        PathProxy::new(HostPath::new(player).field(Player::vela_field_id_level())),
    );
    assert_eq!(
        Player::vela_field_proxy_name(player),
        PathProxy::new(HostPath::new(player).field(Player::vela_field_id_name())),
    );
}

#[test]
fn script_reflect_derive_generates_matching_metadata() {
    let host_desc = Player::vela_host_type_desc();
    let reflect_desc = Player::vela_reflect_type_desc();

    assert_eq!(reflect_desc, host_desc);
    assert!(reflect_desc.schema_hash.is_some());
    assert_eq!(
        <Player as vela_engine::schema::ScriptReflectSchema>::script_reflect_type_desc(),
        reflect_desc,
    );
}

#[test]
fn script_reflect_derive_feeds_engine_registration_api() {
    let engine = vela_engine::engine::Engine::builder()
        .register_reflect_schema::<Player>()
        .build()
        .expect("engine should build from reflected schema");

    let registry = engine.registry();
    let player = registry
        .type_by_name("Player")
        .expect("reflected schema should be registered");
    assert_eq!(player.key.id, Player::vela_type_id());
    assert_eq!(player.kind, TypeKind::Host);
    assert_eq!(player.fields.len(), 2);
    assert_eq!(player.fields[0].name, "level");
    assert_eq!(player.attrs.get("domain"), Some("gameplay"));
    assert_eq!(player.traits, vec![TraitDesc::new("Damageable")]);
}

#[test]
fn script_host_schema_hash_survives_field_reordering() {
    let first = RewardConfigA::vela_host_type_desc();
    let second = RewardConfigB::vela_host_type_desc();

    assert_eq!(first.schema_hash, second.schema_hash);
    assert_ne!(first.fields, second.fields);
}

#[test]
fn script_host_alias_preserves_generated_ids_across_renames() {
    let original = RewardConfigA::vela_host_type_desc();
    let renamed = RewardConfigRenamed::vela_host_type_desc();

    assert_eq!(renamed.key.id, original.key.id);
    assert_eq!(renamed.host_type_id, original.host_type_id);
    assert_eq!(renamed.key.name, "RewardConfigV2");
    assert_eq!(
        RewardConfigRenamed::vela_field_id_item_key(),
        RewardConfigA::vela_field_id_item_id(),
    );
}
