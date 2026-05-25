use vela_common::{FieldId, HostObjectId, HostTypeId, TypeId};
use vela_engine::{HostPath, HostRef, PathProxy};
use vela_macros::{ScriptHost, ScriptReflect};
use vela_reflect::{FieldAccess, FieldDesc, TypeDesc, TypeKey, TypeKind};

#[allow(dead_code)]
#[derive(ScriptHost, ScriptReflect)]
#[script(
    name = "Player",
    id = 1001,
    module = "game.player",
    docs = "Player host schema.",
    attr = "domain=gameplay"
)]
struct Player {
    #[script(
        get,
        set,
        id = 1,
        hint = "int",
        docs = "Current level.",
        attr = "unit=level"
    )]
    level: u32,
    #[script(get, id = 2, name = "display_name", permission = "player.profile")]
    name: String,
    #[script(skip)]
    internal_revision: u64,
}

#[allow(dead_code)]
#[derive(ScriptHost)]
#[script(name = "RewardConfig", id = 2001, module = "game.reward")]
struct RewardConfigA {
    #[script(get, id = 1, hint = "string")]
    item_id: String,
    #[script(get, id = 2, hint = "int")]
    count: i64,
}

#[allow(dead_code)]
#[derive(ScriptHost)]
#[script(name = "RewardConfig", id = 2001, module = "game.reward")]
struct RewardConfigB {
    #[script(get, id = 2, hint = "int")]
    count: i64,
    #[script(get, id = 1, hint = "string")]
    item_id: String,
}

#[test]
fn script_host_derive_generates_type_metadata() {
    let desc = Player::vela_host_type_desc();
    let expected = TypeDesc::new(TypeKey::new(TypeId::new(1001), "Player"))
        .kind(TypeKind::Host)
        .schema_hash(desc.schema_hash.expect("schema hash should be generated"))
        .host_type(HostTypeId::new(1001))
        .attr("module", "game.player")
        .attr("domain", "gameplay")
        .docs("Player host schema.")
        .field(
            FieldDesc::new(FieldId::new(1), "level")
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
            FieldDesc::new(FieldId::new(2), "display_name")
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
    assert_eq!(desc.host_type_id, Some(HostTypeId::new(1001)));
    assert_eq!(desc.attrs.get("module"), Some("game.player"));
    assert_eq!(desc.attrs.get("domain"), Some("gameplay"));
    assert_eq!(desc.fields[0].attrs.get("unit"), Some("level"));
    assert_eq!(desc.fields.len(), 2);
    assert_eq!(
        desc.fields[1].access.required_permissions(),
        &["player.profile".to_owned()]
    );
    assert_eq!(
        <Player as vela_engine::ScriptHostSchema>::script_host_type_desc(),
        desc,
    );
}

#[test]
fn script_host_derive_generates_field_helpers() {
    let player = HostRef::new(HostTypeId::new(1001), HostObjectId::new(42), 3);

    assert_eq!(Player::vela_field_id_level(), FieldId::new(1));
    assert_eq!(Player::vela_field_id_name(), FieldId::new(2));
    assert_eq!(
        Player::vela_field_path_level(player),
        HostPath::new(player).field(FieldId::new(1)),
    );
    assert_eq!(
        Player::vela_field_path_name(player),
        HostPath::new(player).field(FieldId::new(2)),
    );
    assert_eq!(
        Player::vela_field_proxy_level(player),
        PathProxy::new(HostPath::new(player).field(FieldId::new(1))),
    );
    assert_eq!(
        Player::vela_field_proxy_name(player),
        PathProxy::new(HostPath::new(player).field(FieldId::new(2))),
    );
}

#[test]
fn script_reflect_derive_generates_matching_metadata() {
    let host_desc = Player::vela_host_type_desc();
    let reflect_desc = Player::vela_reflect_type_desc();

    assert_eq!(reflect_desc, host_desc);
    assert!(reflect_desc.schema_hash.is_some());
    assert_eq!(
        <Player as vela_engine::ScriptReflectSchema>::script_reflect_type_desc(),
        reflect_desc,
    );
}

#[test]
fn script_host_schema_hash_survives_field_reordering() {
    let first = RewardConfigA::vela_host_type_desc();
    let second = RewardConfigB::vela_host_type_desc();

    assert_eq!(first.schema_hash, second.schema_hash);
    assert_ne!(first.fields, second.fields);
}
