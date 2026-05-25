use vela_common::{HostTypeId, TypeId};
use vela_engine::{Engine, EngineResult};
use vela_reflect::{
    FieldDesc, MethodAccess, MethodDesc, MethodEffectSet, ReflectPolicy, SchemaHash, TraitDesc,
    TypeDesc, TypeKey, TypeRegistry,
};

use super::ids::{CTX_TYPE, DemoIds, MONSTER_TYPE, PLAYER_TYPE};

pub(crate) fn demo_engine(ids: DemoIds) -> EngineResult<Engine> {
    let registry = demo_type_registry(ids);
    let mut builder = Engine::builder()
        .with_standard_natives()
        .reflection_policy(ReflectPolicy::all());
    for desc in registry.types() {
        builder = builder.register_type(desc.clone());
    }
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
        TypeDesc::new(TypeKey::new(TypeId::new(101), "Context"))
            .schema_hash(SchemaHash::new(0x1000_0000_0000_0002))
            .host_type(HostTypeId::new(CTX_TYPE))
            .field(FieldDesc::new(ids.now_field, "now"))
            .field(FieldDesc::new(ids.tick_field, "tick"))
            .method(
                MethodDesc::new(ids.emit_method, "emit")
                    .effects(MethodEffectSet::event_emit())
                    .access(MethodAccess::new().reflect_callable(true)),
            )
            .method(
                MethodDesc::new(ids.log_method, "log")
                    .effects(MethodEffectSet::event_emit())
                    .access(MethodAccess::new().reflect_callable(true)),
            ),
    );
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(102), "Monster"))
            .schema_hash(SchemaHash::new(0x1000_0000_0000_0003))
            .host_type(HostTypeId::new(MONSTER_TYPE))
            .field(FieldDesc::new(ids.id_field, "id"))
            .field(FieldDesc::new(ids.exp_field, "exp"))
            .field(FieldDesc::new(ids.reward_count_field, "reward_count")),
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
    registry
}
