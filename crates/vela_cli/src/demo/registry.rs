use vela_common::{TypeId, VariantId};
use vela_engine::context_schema::context_host_type_desc;
use vela_engine::engine::Engine;
use vela_engine::error::EngineResult;
use vela_engine::native::{EffectSet, FunctionAccess, NativeFunctionDesc, TypeHint};
use vela_engine::permission::PermissionSet;
use vela_engine::random::CONTROLLED_RANDOM_PERMISSION;
use vela_macros::{ScriptHost, script_methods};
use vela_reflect::modules::ModuleDesc;
use vela_reflect::permissions::ReflectPolicy;
use vela_reflect::registry::{FieldDesc, SchemaHash, TypeDesc, TypeKey, TypeRegistry, VariantDesc};
use vela_vm::value::Value;

use super::ids::DemoIds;
use crate::demo::DemoEngineOptions;

pub(crate) fn demo_engine(ids: DemoIds, options: DemoEngineOptions) -> EngineResult<Engine> {
    let registry = demo_support_type_registry(ids);
    let mut permissions = PermissionSet::gameplay();
    if options.allow_random {
        permissions.insert(CONTROLLED_RANDOM_PERMISSION);
    }
    let mut builder = Engine::builder()
        .with_standard_natives()
        .permissions(permissions)
        .with_context_clock(1_700_000_000, 42)
        .with_controlled_random(7)
        .reflection_policy(ReflectPolicy::all())
        .register_host_type::<Player>()
        .register_host_type::<Monster>()
        .register_host_type::<Inventory>()
        .register_host_type::<Config>()
        .register_host_methods::<Player>()
        .register_module(
            ModuleDesc::new("game.reward")
                .docs("Demo reward helper module.")
                .attr("domain", "gameplay"),
        );
    for desc in registry.types() {
        builder = builder.register_type(desc.clone());
    }
    builder = builder.register_typed_native_fn(demo_reward_grant_desc(ids), demo_reward_grant);
    builder.build()
}

pub(crate) fn demo_support_type_registry(ids: DemoIds) -> TypeRegistry {
    let mut registry = TypeRegistry::new();
    registry.register(
        context_host_type_desc()
            .field(FieldDesc::new(ids.config_field, "config").type_hint("Config")),
    );
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(104), "ItemStack"))
            .schema_hash(SchemaHash::new(0x1000_0000_0000_0005))
            .field(
                FieldDesc::new(ids.count_field, "count")
                    .writable(true)
                    .type_hint("int"),
            ),
    );
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(106), "HostQuestProgress"))
            .schema_hash(SchemaHash::new(0x1000_0000_0000_0007))
            .variant(
                VariantDesc::new(VariantId::new(1), "Active")
                    .field(
                        FieldDesc::new(ids.quest_count_field, "quest_count")
                            .writable(true)
                            .type_hint("int"),
                    )
                    .field(
                        FieldDesc::new(ids.quest_done_field, "quest_done")
                            .writable(true)
                            .type_hint("bool"),
                    ),
            ),
    );
    registry
}

fn demo_reward_grant_desc(ids: DemoIds) -> NativeFunctionDesc {
    NativeFunctionDesc::new("game.reward.grant", ids.reward_grant_function)
        .param(
            "player",
            TypeHint::Host(TypeKey::new(TypeId::new(100), "Player")),
        )
        .param("item_id", TypeHint::String)
        .returns(TypeHint::Bool)
        .effects(EffectSet::pure())
        .access(FunctionAccess::public().reflect_callable(true))
        .docs("Grant reward.")
        .attr("event", "reward")
}

fn demo_reward_grant(_: Value, _: String) -> bool {
    true
}

#[allow(dead_code)]
#[derive(ScriptHost)]
#[script(id = 100, host_id = 1, name = "Player", implements = "Damageable")]
struct Player {
    #[script(get, id = 7)]
    id: i64,
    #[script(get, set, id = 2)]
    level: i64,
    #[script(get, set, id = 6)]
    exp: i64,
    #[script(get, id = 10, hint = "HostQuestProgress")]
    quest_progress: HostQuestProgress,
    #[script(get, id = 11)]
    quest_goal: i64,
    #[script(get, id = 14, hint = "Inventory")]
    inventory: Inventory,
}

#[script_methods]
impl Player {
    #[script_method(id = 9, name = "add_reward", effect = "write_host", reflect = true)]
    #[allow(dead_code)]
    pub fn add_reward(
        _ctx: &mut vela_engine::context::NativeCallContext<'_, '_>,
        _player: vela_host::path::HostRef,
        _item_id: String,
        _count: i64,
    ) {
    }
}

#[allow(dead_code)]
#[derive(ScriptHost)]
#[script(id = 102, host_id = 3, name = "Monster")]
struct Monster {
    #[script(get, id = 7)]
    id: i64,
    #[script(get, id = 6)]
    exp: i64,
}

#[allow(dead_code)]
#[derive(ScriptHost)]
#[script(
    id = 105,
    host_id = 4,
    name = "Config",
    docs = "Demo gameplay configuration exposed through context host paths."
)]
struct Config {
    #[script(
        get,
        id = 18,
        hint = "int",
        docs = "Experience threshold for the next level."
    )]
    exp_to_next_level: i64,
    #[script(
        get,
        id = 19,
        hint = "array",
        docs = "Configured monster reward table."
    )]
    kill_rewards: Vec<KillRewardConfig>,
}

#[allow(dead_code)]
#[derive(ScriptHost)]
#[script(id = 103, host_id = 5, name = "Inventory")]
struct Inventory {
    #[script(get, id = 15, hint = "map")]
    items: std::collections::BTreeMap<String, ItemStack>,
}

#[allow(dead_code)]
struct ItemStack;

#[allow(dead_code)]
struct HostQuestProgress;

#[allow(dead_code)]
struct KillRewardConfig;
