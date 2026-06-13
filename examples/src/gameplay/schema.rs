use vela_def::TypeId;
use vela_engine::context_schema::context_host_type_desc;
use vela_engine::engine::Engine;
use vela_engine::error::EngineResult;
use vela_engine::host_type::HostTypeSpec;
use vela_engine::native::{EffectSet, FunctionAccess, NativeFunctionDesc, TypeHint};
use vela_engine::permission::Capability;
use vela_macros::{ScriptHost, ScriptReflect, script_methods};
use vela_reflect::modules::ModuleDesc;
use vela_reflect::permissions::ReflectPolicy;
use vela_reflect::registry::HostIndexCapability;
use vela_reflect::registry::{FieldDesc, TypeDesc, TypeKey};
use vela_vm::owned_value::OwnedValue;

use super::GameEngineOptions;
use super::ids;

pub(crate) fn build_gameplay_engine(options: GameEngineOptions) -> EngineResult<Engine> {
    let mut builder = Engine::builder().with_standard_natives();

    if options.host_read {
        builder = builder.capability(Capability::HostRead);
    }
    if options.host_write {
        builder = builder.capability(Capability::HostWrite);
    }
    if options.event_emit {
        builder = builder.capability(Capability::EventEmit);
    }
    if options.time {
        builder = builder
            .capability(Capability::Time)
            .with_time_clock(1_700_000_000, 42);
    }
    if options.random_function || options.allow_random {
        builder = builder.with_controlled_random(7);
    }
    if options.allow_random {
        builder = builder.capability(Capability::Random);
    }
    if options.reflection {
        builder = builder.reflection_policy(ReflectPolicy::all());
    }

    if options.schema.context {
        builder = builder.register_type(context_type_desc(options.schema.config));
    }
    if options.schema.player {
        builder = builder.register_script_host::<Player>();
    }
    if options.schema.monster {
        builder = builder.register_host_type::<Monster>();
    }
    if options.schema.inventory {
        builder = builder
            .register_host_type::<Inventory>()
            .register_host_type::<ItemStack>()
            .register_host_type_spec(string_item_map_type());
    }
    if options.schema.quest {
        builder = builder.register_reflect_schema::<HostQuestProgress>();
    }
    if options.schema.config {
        builder = builder.register_host_type::<Config>();
    }
    if options.schema.reward {
        builder = builder
            .register_module(
                ModuleDesc::new("game::reward")
                    .docs("Demo reward helper module.")
                    .attr("domain", "gameplay"),
            )
            .register_typed_native_fn(gameplay_reward_grant_desc(), gameplay_reward_grant);
    }
    builder.build()
}

fn context_type_desc(with_config: bool) -> TypeDesc {
    let mut desc = context_host_type_desc();
    if with_config {
        desc = desc.field(FieldDesc::new(ids::config_field(), "config").type_hint("Config"));
    }
    desc
}

fn gameplay_reward_grant_desc() -> NativeFunctionDesc {
    NativeFunctionDesc::new("game::reward::grant", ids::reward_grant_function())
        .param(
            "player",
            TypeHint::Host(TypeKey::new(Player::vela_type_id(), "Player")),
        )
        .param("item_id", TypeHint::string())
        .returns(TypeHint::boolean())
        .effects(EffectSet::pure())
        .access(FunctionAccess::public().reflect_callable(true))
        .docs("Grant reward.")
        .attr("event", "reward")
}

fn gameplay_reward_grant(_: OwnedValue, _: String) -> bool {
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
    #[script(get, hint = "i64", docs = "Experience threshold for the next level.")]
    exp_to_next_level: i64,
    #[script(get, hint = "array", docs = "Configured monster reward table.")]
    kill_rewards: Vec<KillRewardConfig>,
}

#[allow(dead_code)]
#[derive(ScriptHost)]
#[script(path = "game::inventory::Inventory")]
pub(crate) struct Inventory {
    #[script(get, hint = "StringItemMap")]
    items: std::collections::BTreeMap<String, ItemStack>,
}

#[script_methods]
impl Inventory {}

fn string_item_map_type() -> HostTypeSpec {
    HostTypeSpec::new(
        TypeDesc::new(TypeKey::new(TypeId::new(8_802), "StringItemMap")).index_capability(
            HostIndexCapability::new()
                .readable(true)
                .writable(true)
                .key_type("string")
                .value_type("ItemStack"),
        ),
    )
}

#[allow(dead_code)]
#[derive(ScriptHost)]
#[script(path = "game::inventory::ItemStack")]
pub(crate) struct ItemStack {
    #[script(get, set, hint = "i64")]
    count: i64,
}

#[allow(dead_code)]
#[derive(ScriptReflect)]
#[script(path = "game::quest::HostQuestProgress")]
enum HostQuestProgress {
    Active {
        #[script(get, set, hint = "i64")]
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

    fn read_host_target_from(
        &self,
        target: vela_host::target::HostTargetInstance<'_>,
        _offset: usize,
    ) -> vela_host::error::HostResult<vela_host::value::HostValue> {
        Err(vela_host::error::HostError {
            kind: vela_host::error::HostErrorKind::MissingPath {
                path: target.to_diagnostic_path().to_host_path(),
            },
            source_span: None,
        })
    }

    fn write_host_target_from(
        &mut self,
        target: vela_host::target::HostTargetInstance<'_>,
        _offset: usize,
        _value: vela_host::value::HostValue,
    ) -> vela_host::error::HostResult<()> {
        Err(vela_host::error::HostError {
            kind: vela_host::error::HostErrorKind::PermissionDenied {
                path: target.to_diagnostic_path().to_host_path(),
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

    fn read_resolved_host(
        &self,
        _access: vela_host::resolved::ResolvedHostAccess,
        target: vela_host::target::HostTargetInstance<'_>,
    ) -> vela_host::error::HostResult<vela_host::value::HostValue> {
        vela_host::object::ScriptHostFieldAccess::read_host_target_from(self, target, 0)
    }
}

impl vela_host::object::ScriptHostFieldAccess for KillRewardConfig {
    fn script_host_type_id(&self) -> vela_common::HostTypeId {
        vela_common::HostTypeId::new(0)
    }

    fn read_host_target_from(
        &self,
        target: vela_host::target::HostTargetInstance<'_>,
        _offset: usize,
    ) -> vela_host::error::HostResult<vela_host::value::HostValue> {
        Err(vela_host::error::HostError {
            kind: vela_host::error::HostErrorKind::MissingPath {
                path: target.to_diagnostic_path().to_host_path(),
            },
            source_span: None,
        })
    }

    fn write_host_target_from(
        &mut self,
        target: vela_host::target::HostTargetInstance<'_>,
        _offset: usize,
        _value: vela_host::value::HostValue,
    ) -> vela_host::error::HostResult<()> {
        Err(vela_host::error::HostError {
            kind: vela_host::error::HostErrorKind::PermissionDenied {
                path: target.to_diagnostic_path().to_host_path(),
                action: "write",
            },
            source_span: None,
        })
    }
}
