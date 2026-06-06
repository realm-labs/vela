use vela_engine::context_schema::context_host_type_desc;
use vela_engine::engine::Engine;
use vela_engine::error::EngineResult;
use vela_engine::native::{EffectSet, FunctionAccess, NativeFunctionDesc, TypeHint};
use vela_engine::permission::Capability;
use vela_macros::{ScriptHost, ScriptReflect, script_methods};
use vela_reflect::modules::ModuleDesc;
use vela_reflect::permissions::ReflectPolicy;
use vela_reflect::registry::{FieldDesc, TypeKey, TypeRegistry};
use vela_vm::owned_value::OwnedValue;

use super::ids::DemoIds;
use crate::demo::DemoEngineOptions;

pub(crate) fn demo_engine(ids: DemoIds, options: DemoEngineOptions) -> EngineResult<Engine> {
    let registry = demo_support_type_registry(ids);
    let mut builder = Engine::builder()
        .with_standard_natives()
        .capability(Capability::HostRead)
        .capability(Capability::HostWrite)
        .capability(Capability::EventEmit)
        .capability(Capability::Time)
        .with_time_clock(1_700_000_000, 42)
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
    if options.allow_random {
        builder = builder.capability(Capability::Random);
    }
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

fn demo_reward_grant(_: OwnedValue, _: String) -> bool {
    true
}

#[allow(dead_code)]
#[derive(ScriptHost)]
#[script(path = "game::player::Player", implements = "Damageable")]
pub(crate) struct Player {
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
pub(crate) struct Monster {
    #[script(get)]
    id: i64,
    #[script(get)]
    exp: i64,
}

#[allow(dead_code)]
#[derive(ScriptHost)]
#[script(
    path = "game::config::Config",
    docs = "Demo host configuration exposed through context host paths."
)]
pub(crate) struct Config {
    #[script(get, hint = "int", docs = "Experience threshold for the next level.")]
    exp_to_next_level: i64,
    #[script(get, hint = "array", docs = "Configured monster reward table.")]
    kill_rewards: Vec<KillRewardConfig>,
}

#[allow(dead_code)]
#[derive(ScriptHost)]
#[script(path = "game::inventory::Inventory")]
pub(crate) struct Inventory {
    #[script(get, hint = "map")]
    items: std::collections::BTreeMap<String, ItemStack>,
}

#[script_methods]
impl Inventory {}

#[allow(dead_code)]
#[derive(ScriptHost)]
#[script(path = "game::inventory::ItemStack")]
pub(crate) struct ItemStack {
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

impl vela_host::object::ScriptHostFieldAccess for HostQuestProgress {
    fn script_host_type_id(&self) -> vela_common::HostTypeId {
        vela_common::HostTypeId::new(0)
    }

    fn read_host_path_from(
        &self,
        path: &vela_host::path::HostPath,
        _offset: usize,
    ) -> vela_host::error::HostResult<vela_host::value::HostValue> {
        Err(vela_host::error::HostError {
            kind: vela_host::error::HostErrorKind::MissingPath { path: path.clone() },
            source_span: None,
        })
    }

    fn write_host_path_from(
        &mut self,
        path: &vela_host::path::HostPath,
        _offset: usize,
        _value: vela_host::value::HostValue,
    ) -> vela_host::error::HostResult<()> {
        Err(vela_host::error::HostError {
            kind: vela_host::error::HostErrorKind::PermissionDenied {
                path: path.clone(),
                action: "write",
            },
            source_span: None,
        })
    }
}

impl vela_host::object::ScriptHostObject for HostQuestProgress {
    fn host_type_id(&self) -> vela_common::HostTypeId {
        vela_host::object::ScriptHostFieldAccess::script_host_type_id(self)
    }

    fn read_host_path(
        &self,
        path: &vela_host::path::HostPath,
    ) -> vela_host::error::HostResult<vela_host::value::HostValue> {
        vela_host::object::ScriptHostFieldAccess::read_host_path_from(self, path, 0)
    }
}

impl vela_host::object::ScriptHostFieldAccess for KillRewardConfig {
    fn script_host_type_id(&self) -> vela_common::HostTypeId {
        vela_common::HostTypeId::new(0)
    }

    fn read_host_path_from(
        &self,
        path: &vela_host::path::HostPath,
        _offset: usize,
    ) -> vela_host::error::HostResult<vela_host::value::HostValue> {
        Err(vela_host::error::HostError {
            kind: vela_host::error::HostErrorKind::MissingPath { path: path.clone() },
            source_span: None,
        })
    }

    fn write_host_path_from(
        &mut self,
        path: &vela_host::path::HostPath,
        _offset: usize,
        _value: vela_host::value::HostValue,
    ) -> vela_host::error::HostResult<()> {
        Err(vela_host::error::HostError {
            kind: vela_host::error::HostErrorKind::PermissionDenied {
                path: path.clone(),
                action: "write",
            },
            source_span: None,
        })
    }
}
