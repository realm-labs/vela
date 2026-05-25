use std::sync::Arc;

use vela_common::{HostTypeId, TypeId};
use vela_reflect::{
    FieldDesc, MethodAccess, MethodDesc, MethodEffectSet, SchemaHash, TraitDesc, TypeDesc, TypeKey,
    TypeRegistry,
};
use vela_vm::Vm;

use super::ids::{CTX_TYPE, DemoIds, MONSTER_TYPE, PLAYER_TYPE};

pub(crate) fn register_demo_reflection_natives(vm: &mut Vm, ids: DemoIds) {
    vm.register_reflection_natives(Arc::new(demo_type_registry(ids)));
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
    registry
}
