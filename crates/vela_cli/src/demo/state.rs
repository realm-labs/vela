use std::error::Error;

use vela_bytecode::CodeObject;
use vela_common::{HostObjectId, HostTypeId};
use vela_host::{HostPath, HostRef, HostValue, MockStateAdapter, ScriptStateAdapter};
use vela_vm::Value;

use super::ids::{CTX_TYPE, DemoIds, MONSTER_TYPE, PLAYER_TYPE};

const PLAYER_OBJECT: u64 = 7;
const CTX_OBJECT: u64 = 100;
const MONSTER_OBJECT: u64 = 200;
const PLAYER_GENERATION: u32 = 3;
const CTX_GENERATION: u32 = 1;
const MONSTER_GENERATION: u32 = 1;

pub(crate) struct DemoHostState {
    ids: DemoIds,
    player: HostRef,
    ctx: HostRef,
    monster: HostRef,
    has_monster: bool,
    level_path: HostPath,
    exp_path: HostPath,
    quest_count_path: HostPath,
    quest_done_path: HostPath,
    now_path: HostPath,
    tick_path: HostPath,
    pub(crate) adapter: MockStateAdapter,
}

impl DemoHostState {
    pub(crate) fn new(ids: DemoIds, has_monster: bool) -> Self {
        let player = HostRef::new(
            HostTypeId::new(PLAYER_TYPE),
            HostObjectId::new(PLAYER_OBJECT),
            PLAYER_GENERATION,
        );
        let ctx = HostRef::new(
            HostTypeId::new(CTX_TYPE),
            HostObjectId::new(CTX_OBJECT),
            CTX_GENERATION,
        );
        let monster = HostRef::new(
            HostTypeId::new(MONSTER_TYPE),
            HostObjectId::new(MONSTER_OBJECT),
            MONSTER_GENERATION,
        );
        let level_path = HostPath::new(player).field(ids.level_field);
        let exp_path = HostPath::new(player).field(ids.exp_field);
        let quest_count_path = HostPath::new(player).field(ids.quest_count_field);
        let quest_goal_path = HostPath::new(player).field(ids.quest_goal_field);
        let quest_done_path = HostPath::new(player).field(ids.quest_done_field);
        let now_path = HostPath::new(ctx).field(ids.now_field);
        let tick_path = HostPath::new(ctx).field(ids.tick_field);

        let mut adapter = MockStateAdapter::new();
        adapter.insert_value(
            level_path.clone(),
            HostValue::Int(if has_monster { 1 } else { 9 }),
        );
        adapter.insert_value(
            exp_path.clone(),
            HostValue::Int(if has_monster { 90 } else { 0 }),
        );
        adapter.insert_value(HostPath::new(player).field(ids.id_field), HostValue::Int(7));
        adapter.insert_value(quest_count_path.clone(), HostValue::Int(2));
        adapter.insert_value(quest_goal_path, HostValue::Int(3));
        adapter.insert_value(quest_done_path.clone(), HostValue::Bool(false));
        adapter.insert_value(now_path.clone(), HostValue::Int(1_700_000_000));
        adapter.insert_value(tick_path.clone(), HostValue::Int(42));
        adapter.insert_value(
            HostPath::new(monster).field(ids.exp_field),
            HostValue::Int(20),
        );
        adapter.insert_value(
            HostPath::new(monster).field(ids.id_field),
            HostValue::Int(11),
        );
        adapter.insert_value(
            HostPath::new(monster).field(ids.reward_count_field),
            HostValue::Int(3),
        );
        adapter.insert_method_return(ids.emit_method, HostValue::Null);
        adapter.insert_method_return(ids.add_reward_method, HostValue::Null);

        Self {
            ids,
            player,
            ctx,
            monster,
            has_monster,
            level_path,
            exp_path,
            quest_count_path,
            quest_done_path,
            now_path,
            tick_path,
            adapter,
        }
    }

    pub(crate) fn main_args(&self, main: &CodeObject) -> Result<Vec<Value>, Box<dyn Error>> {
        main.params
            .iter()
            .map(|param| match param.as_str() {
                "player" => Ok(Value::HostRef(self.player)),
                "ctx" => Ok(Value::HostRef(self.ctx)),
                "monster" => Ok(Value::HostRef(self.monster)),
                _ => Err(format!("unsupported demo main parameter `{param}`").into()),
            })
            .collect()
    }

    pub(crate) fn print_result(
        &self,
        result: Value,
        patch_count: usize,
    ) -> Result<(), Box<dyn Error>> {
        let level = self.read(&self.level_path)?;
        let now = self.read(&self.now_path)?;
        let tick = self.read(&self.tick_path)?;

        if self.adapter.method_calls().is_empty() {
            println!("result={result:?} level={level:?} patches={patch_count}");
        } else if self.has_monster {
            let exp = self.read(&self.exp_path)?;
            let quest_count = self.read(&self.quest_count_path)?;
            let quest_done = self.read(&self.quest_done_path)?;
            let rewards = self.method_call_count(self.ids.add_reward_method);
            let emits = self.method_call_count(self.ids.emit_method);
            println!(
                "result={result:?} level={level:?} exp={exp:?} quest_count={quest_count:?} \
                 quest_done={quest_done:?} rewards={rewards} emits={emits} \
                 patches={patch_count}",
            );
        } else {
            println!(
                "result={result:?} level={level:?} ctx_now={now:?} ctx_tick={tick:?} \
                 emits={} patches={patch_count}",
                self.adapter.method_calls().len(),
            );
        }
        Ok(())
    }

    fn read(&self, path: &HostPath) -> Result<HostValue, Box<dyn Error>> {
        self.adapter
            .read_path(path)
            .map_err(|error| format!("{error:?}").into())
    }

    fn method_call_count(&self, method: vela_common::HostMethodId) -> usize {
        self.adapter
            .method_calls()
            .iter()
            .filter(|(_, called_method, _)| *called_method == method)
            .count()
    }
}
