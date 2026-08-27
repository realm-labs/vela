use vela_def::TypeId;
use vela_engine::context_schema::context_host_type_desc;
use vela_engine::engine::Engine;
use vela_engine::error::EngineResult;
use vela_engine::host_type::HostTypeSpec;
use vela_engine::native::{EffectSet, FunctionAccess, NativeFunctionDesc, TypeHint};
use vela_engine::permission::Capability;
use vela_engine::registration::VelaBindings;
use vela_macros::{ScriptHost, ScriptReflect, methods};
use vela_reflect::modules::ModuleDesc;
use vela_reflect::permissions::ReflectPolicy;
use vela_reflect::registry::HostIndexCapability;
use vela_reflect::registry::{FieldDesc, TypeDesc, TypeKey};
use vela_vm::owned_value::OwnedValue;

use super::GameEngineOptions;
use super::ids;

pub(crate) fn build_gameplay_engine(options: GameEngineOptions) -> EngineResult<Engine> {
    let mut builder = Engine::builder().with_standard_natives();
    let mut bindings = VelaBindings::new();

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
        builder = builder.register_type_desc(context_type_desc(options.schema.config));
    }
    if options.schema.player {
        bindings
            .register_type(Player::vela_type())
            .register_methods(Player::vela_methods());
    }
    if options.schema.monster {
        bindings.register_type(Monster::vela_type());
    }
    if options.schema.inventory {
        bindings.register_type(Inventory::vela_type());
        bindings.register_type(ItemStack::vela_type());
        builder = builder.register_type_spec(string_item_map_type());
    }
    if options.schema.quest && !options.schema.player {
        builder = builder.register_type_desc(HostQuestProgress::vela_reflect_type_desc());
    }
    if options.schema.config {
        bindings.register_type(Config::vela_type());
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
    builder.register_bindings(bindings).build()
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

#[derive(ScriptHost)]
#[vela(path = "game::player::Player", implements = "Damageable")]
pub(crate) struct Player {
    #[vela(get)]
    id: i64,
    #[vela(get, set)]
    level: i64,
    #[vela(get, set)]
    exp: i64,
    #[vela(get, hint = "HostQuestProgress")]
    quest_progress: HostQuestProgress,
    #[vela(get)]
    quest_goal: i64,
    #[vela(get, hint = "Inventory")]
    inventory: Inventory,
}

#[methods]
impl Player {
    #[vela(name = "add_reward", reflect = true)]
    pub fn add_reward(&mut self, _item_id: String, _count: i64) {}
}

#[derive(ScriptHost)]
#[vela(path = "game::monster::Monster")]
pub(crate) struct Monster {
    #[vela(get)]
    id: i64,
    #[vela(get)]
    exp: i64,
}

#[derive(ScriptHost)]
#[vela(
    path = "game::config::Config",
    docs = "Demo host configuration exposed through context host paths."
)]
pub(crate) struct Config {
    #[vela(get, hint = "i64", docs = "Experience threshold for the next level.")]
    exp_to_next_level: i64,
    #[vela(get, hint = "array", docs = "Configured monster reward table.")]
    kill_rewards: Vec<KillRewardConfig>,
}

#[derive(ScriptHost)]
#[vela(path = "game::inventory::Inventory")]
pub(crate) struct Inventory {
    #[vela(get, hint = "StringItemMap")]
    items: std::collections::BTreeMap<String, ItemStack>,
}

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

#[derive(ScriptHost)]
#[vela(path = "game::inventory::ItemStack")]
pub(crate) struct ItemStack {
    #[vela(get, set, hint = "i64")]
    count: i64,
}

#[derive(ScriptReflect)]
#[vela(path = "game::quest::HostQuestProgress")]
enum HostQuestProgress {
    #[expect(
        dead_code,
        reason = "example registers this variant for script reflection; Rust never constructs it"
    )]
    Active {
        #[vela(get, set, hint = "i64")]
        quest_count: i64,
        #[vela(get, set, hint = "bool")]
        quest_done: bool,
    },
}

#[derive(ScriptHost)]
#[vela(path = "game::config::KillRewardConfig")]
struct KillRewardConfig {}

impl vela_engine::type_registration::VelaType for HostQuestProgress {
    fn register(
        builder: vela_engine::builder::EngineBuilder,
    ) -> vela_engine::builder::EngineBuilder {
        builder.register_type_desc(Self::vela_reflect_type_desc())
    }
}

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
