use vela_common::{FieldId, HostMethodId};

pub(crate) const PLAYER_TYPE: u32 = 1;
pub(crate) const CTX_TYPE: u32 = 2;
pub(crate) const MONSTER_TYPE: u32 = 3;

const LEVEL_FIELD: u32 = 2;
const NOW_FIELD: u32 = 3;
const TICK_FIELD: u32 = 4;
const EXP_FIELD: u32 = 6;
const ID_FIELD: u32 = 7;
const REWARD_COUNT_FIELD: u32 = 8;
const QUEST_COUNT_FIELD: u32 = 10;
const QUEST_GOAL_FIELD: u32 = 11;
const QUEST_DONE_FIELD: u32 = 12;
const INVENTORY_FIELD: u32 = 14;
const ITEMS_FIELD: u32 = 15;
const COUNT_FIELD: u32 = 16;
const EMIT_METHOD: u32 = 5;
const ADD_REWARD_METHOD: u32 = 9;
const LOG_METHOD: u32 = 13;

#[derive(Clone, Copy)]
pub(crate) struct DemoIds {
    pub(crate) level_field: FieldId,
    pub(crate) now_field: FieldId,
    pub(crate) tick_field: FieldId,
    pub(crate) exp_field: FieldId,
    pub(crate) id_field: FieldId,
    pub(crate) reward_count_field: FieldId,
    pub(crate) quest_count_field: FieldId,
    pub(crate) quest_goal_field: FieldId,
    pub(crate) quest_done_field: FieldId,
    pub(crate) inventory_field: FieldId,
    pub(crate) items_field: FieldId,
    pub(crate) count_field: FieldId,
    pub(crate) emit_method: HostMethodId,
    pub(crate) add_reward_method: HostMethodId,
    pub(crate) log_method: HostMethodId,
}

impl DemoIds {
    pub(crate) fn new() -> Self {
        Self {
            level_field: FieldId::new(LEVEL_FIELD),
            now_field: FieldId::new(NOW_FIELD),
            tick_field: FieldId::new(TICK_FIELD),
            exp_field: FieldId::new(EXP_FIELD),
            id_field: FieldId::new(ID_FIELD),
            reward_count_field: FieldId::new(REWARD_COUNT_FIELD),
            quest_count_field: FieldId::new(QUEST_COUNT_FIELD),
            quest_goal_field: FieldId::new(QUEST_GOAL_FIELD),
            quest_done_field: FieldId::new(QUEST_DONE_FIELD),
            inventory_field: FieldId::new(INVENTORY_FIELD),
            items_field: FieldId::new(ITEMS_FIELD),
            count_field: FieldId::new(COUNT_FIELD),
            emit_method: HostMethodId::new(EMIT_METHOD),
            add_reward_method: HostMethodId::new(ADD_REWARD_METHOD),
            log_method: HostMethodId::new(LOG_METHOD),
        }
    }
}
