use vela_common::{FieldId, FunctionId, HostMethodId, HostTypeId, stable_id};
use vela_engine::context_schema::{
    CONTEXT_EMIT_METHOD_ID, CONTEXT_HOST_TYPE_ID, CONTEXT_LOG_METHOD_ID, CONTEXT_NOW_FIELD_ID,
    CONTEXT_TICK_FIELD_ID,
};

use super::registry::{Config, Inventory, ItemStack, Monster, Player};

pub(crate) fn player_type() -> HostTypeId {
    host_type("game::player::Player")
}

pub(crate) fn context_type() -> HostTypeId {
    CONTEXT_HOST_TYPE_ID
}

pub(crate) fn monster_type() -> HostTypeId {
    host_type("game::monster::Monster")
}

#[derive(Clone, Copy)]
pub(crate) struct DemoIds {
    pub(crate) level_field: FieldId,
    pub(crate) now_field: FieldId,
    pub(crate) tick_field: FieldId,
    pub(crate) exp_field: FieldId,
    pub(crate) id_field: FieldId,
    pub(crate) monster_exp_field: FieldId,
    pub(crate) monster_id_field: FieldId,
    pub(crate) quest_progress_field: FieldId,
    pub(crate) quest_count_field: FieldId,
    pub(crate) quest_goal_field: FieldId,
    pub(crate) quest_done_field: FieldId,
    pub(crate) inventory_field: FieldId,
    pub(crate) items_field: FieldId,
    pub(crate) count_field: FieldId,
    pub(crate) config_field: FieldId,
    pub(crate) exp_to_next_level_field: FieldId,
    pub(crate) emit_method: HostMethodId,
    pub(crate) add_reward_method: HostMethodId,
    pub(crate) log_method: HostMethodId,
    pub(crate) reward_grant_function: FunctionId,
}

impl DemoIds {
    pub(crate) fn new() -> Self {
        Self {
            level_field: Player::vela_field_id_level(),
            now_field: CONTEXT_NOW_FIELD_ID,
            tick_field: CONTEXT_TICK_FIELD_ID,
            exp_field: Player::vela_field_id_exp(),
            id_field: Player::vela_field_id_id(),
            monster_exp_field: Monster::vela_field_id_exp(),
            monster_id_field: Monster::vela_field_id_id(),
            quest_progress_field: Player::vela_field_id_quest_progress(),
            quest_count_field: FieldId::new(stable_id(
                "field",
                "HostQuestProgress::Active",
                "quest_count",
            )),
            quest_goal_field: Player::vela_field_id_quest_goal(),
            quest_done_field: FieldId::new(stable_id(
                "field",
                "HostQuestProgress::Active",
                "quest_done",
            )),
            inventory_field: Player::vela_field_id_inventory(),
            items_field: Inventory::vela_field_id_items(),
            count_field: ItemStack::vela_field_id_count(),
            config_field: host_field("Context", "config"),
            exp_to_next_level_field: Config::vela_field_id_exp_to_next_level(),
            emit_method: CONTEXT_EMIT_METHOD_ID,
            add_reward_method: HostMethodId::new(stable_id(
                "host_method",
                "game::player::Player",
                "add_reward",
            )),
            log_method: CONTEXT_LOG_METHOD_ID,
            reward_grant_function: FunctionId::new(stable_id(
                "native_function",
                "",
                "game::reward::grant",
            )),
        }
    }
}

fn host_type(path: &str) -> HostTypeId {
    HostTypeId::new(stable_id("host_ref_type", "", path))
}

fn host_field(owner: &str, field: &str) -> FieldId {
    FieldId::new(stable_id("host_field", owner, field))
}
