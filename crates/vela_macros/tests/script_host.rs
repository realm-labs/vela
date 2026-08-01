use std::collections::{BTreeMap, BTreeSet};
use std::ops::{Deref, DerefMut};

use vela_common::{HostObjectId, StoragePolicy, stable_id};
use vela_def::{FieldId, TypeId};
use vela_engine::engine::Engine;
use vela_engine::permission::Capability;
use vela_engine::runtime::{CallArgs, CallOptions, Runtime};
use vela_host::path::{HostPath, HostRef};
use vela_host::proxy::PathProxy;
use vela_host::resolved::{HostAccessOp, HostAccessSpec, ResolvedHostAccessKind};
use vela_host::target::{HostTargetInstance, HostTargetPlan};
use vela_host::value::HostValue;
use vela_macros::{ScriptHost, ScriptReflect};
use vela_reflect::access::FieldAccess;
use vela_reflect::registry::{FieldDesc, TraitDesc, TypeDesc, TypeKey, TypeKind, VariantDesc};
use vela_vm::owned_value::OwnedValue;

#[allow(dead_code)]
#[derive(ScriptHost, ScriptReflect)]
#[vela(
    path = "game::player::Player",
    docs = "Player host schema.",
    attr = "domain=gameplay",
    implements = "Damageable"
)]
struct Player {
    #[vela(get, set, hint = "u32", docs = "Current level.", attr = "unit=level")]
    level: u32,
    #[vela(get, name = "display_name", permission = "player.profile")]
    name: String,
    #[vela(skip)]
    internal_revision: u64,
}

#[allow(dead_code)]
#[derive(ScriptHost)]
#[vela(path = "game::reward::RewardConfig")]
struct RewardConfigA {
    #[vela(get, hint = "String")]
    item_id: String,
    #[vela(get, hint = "i64")]
    count: i64,
}

#[allow(dead_code)]
#[derive(ScriptHost)]
#[vela(module = "game::reward", name = "RewardConfig")]
struct RewardConfigB {
    #[vela(get, hint = "i64")]
    count: i64,
    #[vela(get, hint = "String")]
    item_id: String,
}

#[allow(dead_code)]
#[derive(ScriptHost)]
#[vela(
    path = "game::reward::RewardConfigV2",
    alias = "game::reward::RewardConfig"
)]
struct RewardConfigRenamed {
    #[vela(get, hint = "String", alias = "item_id")]
    item_key: String,
    #[vela(get, hint = "i64")]
    count: i64,
}

#[allow(dead_code)]
#[derive(ScriptReflect)]
#[vela(path = "game::quest::HostQuestProgress")]
enum HostQuestProgress {
    #[vela(docs = "Active quest progress.")]
    Active {
        #[vela(get, set, hint = "i64")]
        quest_count: i64,
        #[vela(get, set, hint = "bool")]
        quest_done: bool,
    },
    Finished,
}

#[allow(dead_code)]
#[derive(ScriptHost)]
#[vela(path = "game::monster::Monster", docs = "Monster host schema.")]
struct Monster {
    #[vela(get, hint = "i64")]
    exp: i64,
    #[vela(get, hint = "String")]
    species: String,
}

#[allow(dead_code)]
#[derive(ScriptHost)]
#[vela(path = "game::inventory::Inventory", docs = "Inventory host schema.")]
struct Inventory {
    #[vela(get, set, hint = "i64")]
    gold: i64,
    #[vela(get, hint = "u32")]
    capacity: u32,
}

#[allow(dead_code)]
#[derive(ScriptHost)]
#[vela(path = "game::config::Config", docs = "Config host schema.")]
struct GameConfig {
    #[vela(get, hint = "i64")]
    exp_to_next_level: i64,
    #[vela(get, hint = "u32")]
    max_inventory_slots: u32,
}

#[allow(dead_code)]
#[derive(ScriptHost)]
#[vela(path = "game::containers::ContainerHints")]
struct ContainerHints {
    #[vela(get)]
    rewards: Vec<i64>,
    #[vela(get)]
    bytes: Vec<u8>,
    #[vela(get)]
    tags: BTreeSet<String>,
    #[vela(get)]
    scores: BTreeMap<String, i64>,
    #[vela(get, hint = "Array<i64>")]
    explicit_rewards: Vec<i64>,
}

#[derive(Debug)]
struct Tracked<T> {
    value: T,
    mutation_count: u32,
}

impl<T> Tracked<T> {
    fn new(value: T) -> Self {
        Self {
            value,
            mutation_count: 0,
        }
    }
}

impl<T> Deref for Tracked<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> DerefMut for Tracked<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.mutation_count += 1;
        &mut self.value
    }
}

#[derive(Debug, ScriptHost)]
#[vela(path = "game::script::EquipmentRecord", fields)]
struct EquipmentRecord {
    quality: i64,
    slot: i64,
}

#[derive(Debug, ScriptHost)]
#[vela(path = "game::script::EquipmentState", fields)]
struct EquipmentState {
    equipment: BTreeMap<i64, EquipmentRecord>,
}

#[vela_macros::methods(path = "game::script::EquipmentState")]
impl EquipmentState {
    pub fn mark_all_legendary(&mut self) {
        for equipment in self.equipment.values_mut() {
            equipment.quality = 100;
        }
    }
}

#[derive(Debug, ScriptHost)]
#[vela(path = "game::script::PlayerState", fields)]
struct PlayerState {
    #[vela(deref)]
    equipment: Tracked<EquipmentState>,
}

#[derive(Debug, ScriptHost)]
#[vela(path = "game::script::ActorState", fields)]
struct ActorState {
    player: PlayerState,
}

#[test]
fn script_host_derive_generates_type_metadata() {
    let desc = Player::vela_host_type_desc();
    let expected = TypeDesc::new(TypeKey::new(Player::vela_type_id(), "Player"))
        .kind(TypeKind::Host)
        .schema_hash(desc.schema_hash.expect("schema hash should be generated"))
        .host_type(Player::vela_host_type_id())
        .attr("module", "game::player")
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
                .type_hint("u32")
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
                .type_hint("String"),
        );

    assert_eq!(desc, expected);
    assert_eq!(desc.kind, TypeKind::Host);
    assert_eq!(
        Player::vela_type_id(),
        TypeId::new(u128::from(stable_id(
            "host_type",
            "",
            "game::player::Player",
        ))),
    );
    assert_eq!(desc.host_type_id, Some(Player::vela_host_type_id()));
    assert_eq!(desc.attrs.get("module"), Some("game::player"));
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
fn script_host_derive_generates_unified_type_binding() {
    let engine = vela_engine::engine::Engine::builder()
        .register_type::<Player>()
        .build()
        .expect("derived host TypeBinding should seal");
    let type_bindings = engine.type_bindings();
    let binding = type_bindings
        .get_for::<Player>()
        .expect("derived host binding should use typed lookup");

    assert_eq!(binding.storage, StoragePolicy::Host);
    assert_eq!(binding.key, Player::vela_host_type_desc().key);
}

#[test]
fn script_host_fields_and_deref_projection_register_the_complete_host_graph() {
    let engine = Engine::builder()
        .register_type::<ActorState>()
        .build()
        .expect("one root registration should install the complete host graph");
    let registry = engine.registry();

    for type_name in [
        "ActorState",
        "PlayerState",
        "EquipmentState",
        "EquipmentRecord",
    ] {
        assert!(
            registry.type_by_name(type_name).is_some(),
            "missing recursively registered host type {type_name}",
        );
    }

    let player = registry
        .type_by_name("PlayerState")
        .expect("player schema should be registered");
    let equipment = &player.fields[0];
    assert_eq!(equipment.name, "equipment");
    assert_eq!(equipment.type_hint.as_deref(), Some("EquipmentState"));
    assert!(equipment.access.readable);
    assert!(!equipment.access.writable);

    let record = registry
        .type_by_name("EquipmentRecord")
        .expect("record schema should be registered");
    assert_eq!(record.fields.len(), 2);
    assert!(record.fields.iter().all(|field| field.access.readable));
    assert!(record.fields.iter().all(|field| field.access.writable));
}

#[test]
fn script_host_deref_projection_supports_live_iteration_and_write_through() {
    let engine = Engine::builder()
        .capability(Capability::HostRead)
        .capability(Capability::HostWrite)
        .register_type::<ActorState>()
        .register_exports(EquipmentState::vela_inherent_exports())
        .build()
        .expect("projected host graph should seal");
    let program = engine
        .compile_source(
            r#"
fn upgrade(actor: ActorState) {
    actor.player.equipment.mark_all_legendary();
    actor.player.equipment.equipment[7i64].quality += 1;
    for pair in actor.player.equipment.equipment {
        let entry = pair.value;
        entry.quality += 1;
        host::release(entry);
    }
    return 0;
}
"#,
        )
        .expect("Rust-shaped projected field traversal should compile");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");
    let mut actor = ActorState {
        player: PlayerState {
            equipment: Tracked::new(EquipmentState {
                equipment: BTreeMap::from([
                    (
                        7,
                        EquipmentRecord {
                            quality: 3,
                            slot: 1,
                        },
                    ),
                    (
                        9,
                        EquipmentRecord {
                            quality: 5,
                            slot: 2,
                        },
                    ),
                ]),
            }),
        },
    };

    let result = runtime
        .call(
            "upgrade",
            CallArgs::new().with_host_mut("actor", &mut actor),
            CallOptions::unbounded(),
        )
        .expect("projected mutation should execute");

    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(0)));
    assert_eq!(actor.player.equipment.value.equipment[&7].quality, 102);
    assert_eq!(actor.player.equipment.value.equipment[&9].quality, 101);
    assert!(
        actor.player.equipment.mutation_count > 0,
        "write-through must pass through DerefMut so persistence wrappers observe mutation",
    );
}

#[test]
fn script_reflect_derive_generates_enum_variant_metadata() {
    let desc = HostQuestProgress::vela_reflect_type_desc();
    let active_variant = VariantDesc::new(
        vela_def::VariantId::new(u128::from(stable_id(
            "variant",
            "game::quest::HostQuestProgress",
            "Active",
        ))),
        "Active",
    )
    .docs("Active quest progress.")
    .field(
        FieldDesc::new(
            FieldId::new(u128::from(stable_id(
                "field",
                "HostQuestProgress::Active",
                "quest_count",
            ))),
            "quest_count",
        )
        .access(
            FieldAccess::new()
                .readable(true)
                .writable(true)
                .reflect_readable(true)
                .reflect_writable(true),
        )
        .attr("rust_name", "quest_count")
        .type_hint("i64"),
    )
    .field(
        FieldDesc::new(
            FieldId::new(u128::from(stable_id(
                "field",
                "HostQuestProgress::Active",
                "quest_done",
            ))),
            "quest_done",
        )
        .access(
            FieldAccess::new()
                .readable(true)
                .writable(true)
                .reflect_readable(true)
                .reflect_writable(true),
        )
        .attr("rust_name", "quest_done")
        .type_hint("bool"),
    );
    let finished_variant = VariantDesc::new(
        vela_def::VariantId::new(u128::from(stable_id(
            "variant",
            "game::quest::HostQuestProgress",
            "Finished",
        ))),
        "Finished",
    );

    assert_eq!(desc.key.name, "HostQuestProgress");
    assert_eq!(desc.kind, TypeKind::Host);
    assert_eq!(desc.attrs.get("module"), Some("game::quest"));
    assert_eq!(desc.variants, vec![active_variant, finished_variant]);
    assert!(desc.schema_hash.is_some());
    assert_eq!(
        <HostQuestProgress as vela_engine::schema::ScriptReflectSchema>::script_reflect_type_desc(),
        desc,
    );
}

#[test]
fn script_reflect_enum_schema_feeds_engine_registration_api() {
    let engine = vela_engine::engine::Engine::builder()
        .register_type_desc(HostQuestProgress::vela_reflect_type_desc())
        .build()
        .expect("engine should build from reflected enum schema");

    let registry = engine.registry();
    let progress = registry
        .type_by_name("HostQuestProgress")
        .expect("reflected enum schema should be registered");
    assert_eq!(progress.variants.len(), 2);
    assert_eq!(progress.variants[0].fields.len(), 2);
    assert_eq!(progress.variants[0].fields[0].name, "quest_count");
}

#[test]
fn script_host_derive_generates_field_helpers() {
    let player = HostRef::new(Player::vela_host_type_id(), HostObjectId::new(42), 3);

    assert_eq!(
        Player::vela_field_id_level(),
        FieldId::new(u128::from(stable_id(
            "host_field",
            "game::player::Player",
            "level",
        ))),
    );
    assert_eq!(
        Player::vela_field_id_name(),
        FieldId::new(u128::from(stable_id(
            "host_field",
            "game::player::Player",
            "display_name"
        ))),
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
        PathProxy::new(
            player,
            HostTargetPlan::new(Player::vela_host_type_id()).field(Player::vela_field_id_level()),
        ),
    );
    assert_eq!(
        Player::vela_field_proxy_name(player),
        PathProxy::new(
            player,
            HostTargetPlan::new(Player::vela_host_type_id()).field(Player::vela_field_id_name()),
        ),
    );
}

#[test]
fn script_host_derive_resolves_leaf_fields_to_direct_access() {
    let player = Player {
        level: 7,
        name: "Ada".to_owned(),
        internal_revision: 1,
    };
    let plan =
        HostTargetPlan::new(Player::vela_host_type_id()).field(Player::vela_field_id_level());

    let access = <Player as vela_host::object::ScriptHostFieldAccess>::resolve_host_target_from(
        &player,
        HostAccessSpec::new(HostAccessOp::Read, &plan),
        0,
    )
    .expect("generated host field resolver should resolve level");

    assert_eq!(access.adapter_kind, ResolvedHostAccessKind::DirectField(0));
}

#[test]
fn script_host_derive_executes_dense_field_slots() {
    let root = HostRef::new(Player::vela_host_type_id(), HostObjectId::new(42), 3);
    let plan =
        HostTargetPlan::new(Player::vela_host_type_id()).field(Player::vela_field_id_level());
    let target = HostTargetInstance::new(root, &plan, &[]);
    let mut player = Player {
        level: 7,
        name: "Ada".to_owned(),
        internal_revision: 1,
    };

    let value =
        <Player as vela_host::object::ScriptHostFieldAccess>::read_direct_field(&player, 0, target)
            .expect("generated dense field read should execute");
    assert_eq!(value, HostValue::Scalar(vela_common::ScalarValue::U32(7)));

    <Player as vela_host::object::ScriptHostFieldAccess>::write_direct_field(
        &mut player,
        0,
        target,
        HostValue::Scalar(vela_common::ScalarValue::U32(11)),
    )
    .expect("generated dense field write should execute");
    assert_eq!(player.level, 11);
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
        .register_type_desc(Player::vela_reflect_type_desc())
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
fn script_host_and_reflect_derive_register_matching_engine_schemas() {
    let host_engine = vela_engine::engine::Engine::builder()
        .register_type::<Player>()
        .build()
        .expect("engine should build from host schema");
    let reflect_engine = vela_engine::engine::Engine::builder()
        .register_type_desc(Player::vela_reflect_type_desc())
        .build()
        .expect("engine should build from reflected schema");

    let host_registry = host_engine.registry();
    let reflect_registry = reflect_engine.registry();
    let host_player = host_registry
        .type_by_name("Player")
        .expect("host schema should be registered");
    let reflect_player = reflect_registry
        .type_by_name("Player")
        .expect("reflected schema should be registered");

    assert_eq!(host_player, reflect_player);
    assert_eq!(host_player, &Player::vela_host_type_desc());
    assert_eq!(reflect_player, &Player::vela_reflect_type_desc());
}

#[test]
fn script_host_sample_game_schemas_register_with_engine_builder() {
    let engine = vela_engine::engine::Engine::builder()
        .register_type::<Player>()
        .register_type::<Monster>()
        .register_type::<Inventory>()
        .register_type::<GameConfig>()
        .build()
        .expect("engine should build from sample game host schemas");
    let registry = engine.registry();

    for desc in [
        Player::vela_host_type_desc(),
        Monster::vela_host_type_desc(),
        Inventory::vela_host_type_desc(),
        GameConfig::vela_host_type_desc(),
    ] {
        let registered = registry
            .type_by_name(&desc.key.name)
            .expect("sample host schema should register");
        assert_eq!(registered, &desc);
        assert_eq!(registered.kind, TypeKind::Host);
        assert!(registered.host_type_id.is_some());
        assert_eq!(registered.fields.len(), 2);
    }

    assert!(registry.type_by_name("Player").is_some());
    assert!(registry.type_by_name("Monster").is_some());
    assert!(registry.type_by_name("Inventory").is_some());
    assert!(registry.type_by_name("Config").is_some());
}

#[test]
fn script_host_derive_infers_parameterized_container_hints() {
    let desc = ContainerHints::vela_host_type_desc();

    assert_eq!(desc.fields.len(), 5);
    assert_eq!(desc.fields[0].name, "rewards");
    assert_eq!(desc.fields[0].type_hint.as_deref(), Some("Array<i64>"));
    assert_eq!(desc.fields[1].name, "bytes");
    assert_eq!(desc.fields[1].type_hint.as_deref(), Some("Bytes"));
    assert_eq!(desc.fields[2].name, "tags");
    assert_eq!(desc.fields[2].type_hint.as_deref(), Some("Set<String>"));
    assert_eq!(desc.fields[3].name, "scores");
    assert_eq!(
        desc.fields[3].type_hint.as_deref(),
        Some("Map<String, i64>")
    );
    assert_eq!(desc.fields[4].name, "explicit_rewards");
    assert_eq!(desc.fields[4].type_hint.as_deref(), Some("Array<i64>"));
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
