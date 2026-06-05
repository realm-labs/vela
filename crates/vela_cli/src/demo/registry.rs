use vela_engine::context_schema::context_host_type_desc;
use vela_engine::engine::Engine;
use vela_engine::error::EngineResult;
use vela_engine::native::{EffectSet, FunctionAccess, NativeFunctionDesc, TypeHint};
use vela_engine::permission::PermissionSet;
use vela_engine::random::CONTROLLED_RANDOM_PERMISSION;
use vela_macros::{ScriptHost, ScriptReflect, script_methods};
use vela_reflect::modules::ModuleDesc;
use vela_reflect::permissions::ReflectPolicy;
use vela_reflect::registry::{FieldDesc, TypeKey, TypeRegistry};
use vela_vm::owned_value::OwnedValue as Value;

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
        .register_script_host::<Player>()
        .register_host_type::<Monster>()
        .register_host_type::<Inventory>()
        .register_host_type::<ItemStack>()
        .register_host_type::<Config>()
        .register_reflect_schema::<HostQuestProgress>()
        .register_module(
            ModuleDesc::new("game::reward")
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
    registry
}

fn demo_reward_grant_desc(ids: DemoIds) -> NativeFunctionDesc {
    NativeFunctionDesc::new("game::reward::grant", ids.reward_grant_function)
        .param(
            "player",
            TypeHint::Host(TypeKey::new(Player::vela_type_id(), "Player")),
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
#[script(path = "game::player::Player", implements = "Damageable")]
struct Player {
    #[script(get)]
    id: i64,
    #[script(get, set)]
    level: i64,
    #[script(get, set)]
    exp: i64,
    #[script(get, hint = "HostQuestProgress")]
    quest_progress: HostQuestProgress,
    #[script(get)]
    quest_goal: i64,
    #[script(get, hint = "Inventory")]
    inventory: Inventory,
}

#[script_methods]
impl Player {
    #[script_method(name = "add_reward", effect = "write_host", reflect = true)]
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
#[script(path = "game::monster::Monster")]
struct Monster {
    #[script(get)]
    id: i64,
    #[script(get)]
    exp: i64,
}

#[allow(dead_code)]
#[derive(ScriptHost)]
#[script(
    path = "game::config::Config",
    docs = "Demo gameplay configuration exposed through context host paths."
)]
struct Config {
    #[script(get, hint = "int", docs = "Experience threshold for the next level.")]
    exp_to_next_level: i64,
    #[script(get, hint = "array", docs = "Configured monster reward table.")]
    kill_rewards: Vec<KillRewardConfig>,
}

#[allow(dead_code)]
#[derive(ScriptHost)]
#[script(path = "game::inventory::Inventory")]
struct Inventory {
    #[script(get, hint = "map")]
    items: std::collections::BTreeMap<String, ItemStack>,
}

#[allow(dead_code)]
#[derive(ScriptHost)]
#[script(path = "game::inventory::ItemStack")]
struct ItemStack {
    #[script(get, set, hint = "int")]
    count: i64,
}

#[allow(dead_code)]
#[derive(ScriptReflect)]
#[script(path = "game::quest::HostQuestProgress")]
enum HostQuestProgress {
    Active {
        #[script(get, set, hint = "int")]
        quest_count: i64,
        #[script(get, set, hint = "bool")]
        quest_done: bool,
    },
}

#[allow(dead_code)]
struct KillRewardConfig;
