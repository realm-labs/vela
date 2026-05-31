use vela_common::{HostTypeId, TypeId};
use vela_engine::{
    EffectSet, Engine, EngineResult, FunctionAccess, NativeFunctionDesc, PermissionSet, TypeHint,
    context_host_type_desc,
};
use vela_reflect::{
    FieldDesc, MethodAccess, MethodDesc, MethodEffectSet, ModuleDesc, ReflectPolicy, SchemaHash,
    TraitDesc, TypeDesc, TypeKey, TypeRegistry,
};
use vela_vm::Value;

use super::ids::{CONFIG_TYPE, DemoIds, MONSTER_TYPE, PLAYER_TYPE};

pub(crate) fn demo_engine(ids: DemoIds) -> EngineResult<Engine> {
    let registry = demo_type_registry(ids);
    let mut builder = Engine::builder()
        .with_standard_natives()
        .permissions(PermissionSet::gameplay())
        .with_context_clock(1_700_000_000, 42)
        .reflection_policy(ReflectPolicy::all())
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

pub(crate) fn demo_type_registry(ids: DemoIds) -> TypeRegistry {
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(100), "Player"))
            .schema_hash(SchemaHash::new(0x1000_0000_0000_0001))
            .host_type(HostTypeId::new(PLAYER_TYPE))
            .field(FieldDesc::new(ids.id_field, "id"))
            .field(FieldDesc::new(ids.level_field, "level").writable(true))
            .field(FieldDesc::new(ids.exp_field, "exp").writable(true))
            .field(FieldDesc::new(ids.quest_count_field, "quest_count").writable(true))
            .field(FieldDesc::new(ids.quest_goal_field, "quest_goal"))
            .field(FieldDesc::new(ids.quest_done_field, "quest_done").writable(true))
            .field(FieldDesc::new(ids.inventory_field, "inventory").type_hint("Inventory"))
            .method(
                MethodDesc::new(ids.add_reward_method, "add_reward")
                    .effects(MethodEffectSet::host_write())
                    .access(MethodAccess::new().reflect_callable(true)),
            )
            .trait_impl(TraitDesc::new("Damageable")),
    );
    registry.register(
        context_host_type_desc()
            .field(FieldDesc::new(ids.config_field, "config").type_hint("Config")),
    );
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(102), "Monster"))
            .schema_hash(SchemaHash::new(0x1000_0000_0000_0003))
            .host_type(HostTypeId::new(MONSTER_TYPE))
            .field(FieldDesc::new(ids.id_field, "id"))
            .field(FieldDesc::new(ids.exp_field, "exp")),
    );
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(103), "Inventory"))
            .schema_hash(SchemaHash::new(0x1000_0000_0000_0004))
            .field(FieldDesc::new(ids.items_field, "items").type_hint("map")),
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
        TypeDesc::new(TypeKey::new(TypeId::new(105), "Config"))
            .schema_hash(SchemaHash::new(0x1000_0000_0000_0006))
            .host_type(HostTypeId::new(CONFIG_TYPE))
            .docs("Demo gameplay configuration exposed through context host paths.")
            .field(
                FieldDesc::new(ids.exp_to_next_level_field, "exp_to_next_level")
                    .type_hint("int")
                    .docs("Experience threshold for the next level."),
            )
            .field(
                FieldDesc::new(ids.kill_rewards_field, "kill_rewards")
                    .type_hint("array")
                    .docs("Configured monster reward table."),
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
