use vela_common::{HostMethodId, HostTypeId, stable_id};
use vela_def::{FieldId, FunctionId};
use vela_engine::context_schema::{
    CONTEXT_EMIT_METHOD_ID, CONTEXT_HOST_TYPE_ID, CONTEXT_LOG_METHOD_ID, CONTEXT_NOW_FIELD_ID,
    CONTEXT_TICK_FIELD_ID,
};

use super::schema::{Config, Inventory, ItemStack, Monster, Player};

pub(crate) fn player_type() -> HostTypeId {
    host_type("game::player::Player")
}

pub(crate) fn context_type() -> HostTypeId {
    CONTEXT_HOST_TYPE_ID
}

pub(crate) fn monster_type() -> HostTypeId {
    host_type("game::monster::Monster")
}

pub(crate) fn level_field() -> FieldId {
    Player::vela_field_id_level()
}

pub(crate) fn now_field() -> FieldId {
    CONTEXT_NOW_FIELD_ID
}

pub(crate) fn tick_field() -> FieldId {
    CONTEXT_TICK_FIELD_ID
}

pub(crate) fn exp_field() -> FieldId {
    Player::vela_field_id_exp()
}

pub(crate) fn player_id_field() -> FieldId {
    Player::vela_field_id_id()
}

pub(crate) fn monster_exp_field() -> FieldId {
    Monster::vela_field_id_exp()
}

pub(crate) fn monster_id_field() -> FieldId {
    Monster::vela_field_id_id()
}

pub(crate) fn quest_progress_field() -> FieldId {
    Player::vela_field_id_quest_progress()
}

pub(crate) fn quest_count_field() -> FieldId {
    FieldId::new(u128::from(stable_id(
        "field",
        "HostQuestProgress::Active",
        "quest_count",
    )))
}

pub(crate) fn quest_goal_field() -> FieldId {
    Player::vela_field_id_quest_goal()
}

pub(crate) fn quest_done_field() -> FieldId {
    FieldId::new(u128::from(stable_id(
        "field",
        "HostQuestProgress::Active",
        "quest_done",
    )))
}

pub(crate) fn inventory_field() -> FieldId {
    Player::vela_field_id_inventory()
}

pub(crate) fn items_field() -> FieldId {
    Inventory::vela_field_id_items()
}

pub(crate) fn item_count_field() -> FieldId {
    ItemStack::vela_field_id_count()
}

pub(crate) fn config_field() -> FieldId {
    host_field("Context", "config")
}

pub(crate) fn exp_to_next_level_field() -> FieldId {
    Config::vela_field_id_exp_to_next_level()
}

pub(crate) fn emit_method() -> HostMethodId {
    CONTEXT_EMIT_METHOD_ID
}

pub(crate) fn add_reward_method() -> HostMethodId {
    HostMethodId::new(u128::from(stable_id(
        "host_method",
        "game::player::Player",
        "add_reward",
    )))
}

pub(crate) fn log_method() -> HostMethodId {
    CONTEXT_LOG_METHOD_ID
}

pub(crate) fn reward_grant_function() -> FunctionId {
    FunctionId::new(u128::from(stable_id(
        "native_function",
        "",
        "game::reward::grant",
    )))
}

fn host_type(path: &str) -> HostTypeId {
    HostTypeId::new(stable_id("host_ref_type", "", path))
}

fn host_field(owner: &str, field: &str) -> FieldId {
    FieldId::new(u128::from(stable_id("host_field", owner, field)))
}
