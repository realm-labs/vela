use vela_common::{FieldId, FunctionId, HostMethodId};
use vela_engine::context_schema::{
    CONTEXT_EMIT_METHOD_ID, CONTEXT_HOST_TYPE_ID, CONTEXT_LOG_METHOD_ID, CONTEXT_NOW_FIELD_ID,
    CONTEXT_TICK_FIELD_ID,
};

pub(crate) const PLAYER_TYPE: u32 = 1;
pub(crate) const CTX_TYPE: u32 = CONTEXT_HOST_TYPE_ID.get();
pub(crate) const MONSTER_TYPE: u32 = 3;

const LEVEL_FIELD: u32 = 2;
const EXP_FIELD: u32 = 6;
const ID_FIELD: u32 = 7;
const QUEST_PROGRESS_FIELD: u32 = 10;
const QUEST_GOAL_FIELD: u32 = 11;
const INVENTORY_FIELD: u32 = 14;
const ITEMS_FIELD: u32 = 15;
const COUNT_FIELD: u32 = 16;
const CONFIG_FIELD: u32 = 17;
const EXP_TO_NEXT_LEVEL_FIELD: u32 = 18;
const KILL_REWARDS_FIELD: u32 = 19;
const QUEST_COUNT_FIELD: u32 = 20;
const QUEST_DONE_FIELD: u32 = 21;
const ADD_REWARD_METHOD: u32 = 9;
const REWARD_GRANT_FUNCTION: u64 = 40;

#[derive(Clone, Copy)]
pub(crate) struct DemoIds {
    pub(crate) level_field: FieldId,
    pub(crate) now_field: FieldId,
    pub(crate) tick_field: FieldId,
    pub(crate) exp_field: FieldId,
    pub(crate) id_field: FieldId,
    pub(crate) quest_progress_field: FieldId,
    pub(crate) quest_count_field: FieldId,
    pub(crate) quest_goal_field: FieldId,
    pub(crate) quest_done_field: FieldId,
    pub(crate) inventory_field: FieldId,
    pub(crate) items_field: FieldId,
    pub(crate) count_field: FieldId,
    pub(crate) config_field: FieldId,
    pub(crate) exp_to_next_level_field: FieldId,
    pub(crate) kill_rewards_field: FieldId,
    pub(crate) emit_method: HostMethodId,
    pub(crate) add_reward_method: HostMethodId,
    pub(crate) log_method: HostMethodId,
    pub(crate) reward_grant_function: FunctionId,
}

impl DemoIds {
    pub(crate) fn new() -> Self {
        Self {
            level_field: FieldId::new(LEVEL_FIELD),
            now_field: CONTEXT_NOW_FIELD_ID,
            tick_field: CONTEXT_TICK_FIELD_ID,
            exp_field: FieldId::new(EXP_FIELD),
            id_field: FieldId::new(ID_FIELD),
            quest_progress_field: FieldId::new(QUEST_PROGRESS_FIELD),
            quest_count_field: FieldId::new(QUEST_COUNT_FIELD),
            quest_goal_field: FieldId::new(QUEST_GOAL_FIELD),
            quest_done_field: FieldId::new(QUEST_DONE_FIELD),
            inventory_field: FieldId::new(INVENTORY_FIELD),
            items_field: FieldId::new(ITEMS_FIELD),
            count_field: FieldId::new(COUNT_FIELD),
            config_field: FieldId::new(CONFIG_FIELD),
            exp_to_next_level_field: FieldId::new(EXP_TO_NEXT_LEVEL_FIELD),
            kill_rewards_field: FieldId::new(KILL_REWARDS_FIELD),
            emit_method: CONTEXT_EMIT_METHOD_ID,
            add_reward_method: HostMethodId::new(ADD_REWARD_METHOD),
            log_method: CONTEXT_LOG_METHOD_ID,
            reward_grant_function: FunctionId::new(REWARD_GRANT_FUNCTION),
        }
    }
}
