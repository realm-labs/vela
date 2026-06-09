use std::error::Error;

use vela_bytecode::CodeObject;
use vela_common::HostObjectId;
use vela_engine::runtime::CallArgs;
use vela_host::mock::MockStateAdapter;
use vela_host::path::{HostPath, HostRef};
use vela_host::value::HostValue;
use vela_vm::owned_value::OwnedValue;

use super::ids::{DemoIds, context_type, monster_type, player_type};

const PLAYER_OBJECT: u64 = 7;
const CTX_OBJECT: u64 = 100;
const MONSTER_OBJECT: u64 = 200;
const PLAYER_GENERATION: u32 = 3;
const CTX_GENERATION: u32 = 1;
const MONSTER_GENERATION: u32 = 1;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct DemoHostOptions {
    pub(crate) has_monster: bool,
    pub(crate) stale_player_arg: bool,
    pub(crate) deny_player_level_read: bool,
    pub(crate) deny_player_level_write: bool,
    pub(crate) deny_context_emit_call: bool,
}

pub(crate) struct DemoHostState {
    ids: DemoIds,
    player_arg: HostRef,
    ctx: HostRef,
    monster: HostRef,
    has_monster: bool,
    level_path: HostPath,
    exp_path: HostPath,
    quest_count_path: HostPath,
    quest_done_path: HostPath,
    inventory_gold_count_path: HostPath,
    now_path: HostPath,
    tick_path: HostPath,
    pub(crate) adapter: MockStateAdapter,
}

impl DemoHostState {
    pub(crate) fn new(ids: DemoIds, options: DemoHostOptions) -> Self {
        let player = HostRef::new(
            player_type(),
            HostObjectId::new(PLAYER_OBJECT),
            PLAYER_GENERATION,
        );
        let player_arg = if options.stale_player_arg {
            HostRef::new(
                player_type(),
                HostObjectId::new(PLAYER_OBJECT),
                PLAYER_GENERATION - 1,
            )
        } else {
            player
        };
        let ctx = HostRef::new(
            context_type(),
            HostObjectId::new(CTX_OBJECT),
            CTX_GENERATION,
        );
        let monster = HostRef::new(
            monster_type(),
            HostObjectId::new(MONSTER_OBJECT),
            MONSTER_GENERATION,
        );
        let level_path = HostPath::new(player).field(ids.level_field);
        let exp_path = HostPath::new(player).field(ids.exp_field);
        let quest_progress_path = HostPath::new(player).field(ids.quest_progress_field);
        let quest_count_path = quest_progress_path
            .clone()
            .variant_field(ids.quest_count_field);
        let quest_goal_path = HostPath::new(player).field(ids.quest_goal_field);
        let quest_done_path = quest_progress_path.variant_field(ids.quest_done_field);
        let inventory_gold_count_path = HostPath::new(player)
            .field(ids.inventory_field)
            .field(ids.items_field)
            .key("gold")
            .field(ids.count_field);
        let now_path = HostPath::new(ctx).field(ids.now_field);
        let tick_path = HostPath::new(ctx).field(ids.tick_field);
        let exp_to_next_level_path = HostPath::new(ctx)
            .field(ids.config_field)
            .field(ids.exp_to_next_level_field);

        let mut adapter = MockStateAdapter::new();
        adapter.insert_value(
            level_path.clone(),
            HostValue::Int(if options.has_monster { 1 } else { 9 }),
        );
        adapter.insert_value(
            exp_path.clone(),
            HostValue::Int(if options.has_monster { 90 } else { 0 }),
        );
        adapter.insert_value(HostPath::new(player).field(ids.id_field), HostValue::Int(7));
        adapter.insert_value(quest_count_path.clone(), HostValue::Int(2));
        adapter.insert_value(quest_goal_path, HostValue::Int(3));
        adapter.insert_value(quest_done_path.clone(), HostValue::Bool(false));
        adapter.insert_value(inventory_gold_count_path.clone(), HostValue::Int(0));
        adapter.insert_value(now_path.clone(), HostValue::Int(1_700_000_000));
        adapter.insert_value(tick_path.clone(), HostValue::Int(42));
        adapter.insert_value(exp_to_next_level_path, HostValue::Int(100));
        adapter.insert_value(
            HostPath::new(monster).field(ids.monster_exp_field),
            HostValue::Int(20),
        );
        adapter.insert_value(
            HostPath::new(monster).field(ids.exp_field),
            HostValue::Int(20),
        );
        adapter.insert_value(
            HostPath::new(monster).field(ids.monster_id_field),
            HostValue::Int(11),
        );
        adapter.insert_value(
            HostPath::new(monster).field(ids.id_field),
            HostValue::Int(11),
        );
        adapter.insert_method_return(ids.emit_method, HostValue::Null);
        adapter.insert_method_return(ids.add_reward_method, HostValue::Null);
        adapter.insert_method_return(ids.log_method, HostValue::Null);
        if options.deny_player_level_read {
            adapter.deny_read(level_path.clone());
        }
        if options.deny_player_level_write {
            adapter.deny_write(level_path.clone());
        }
        if options.deny_context_emit_call {
            adapter.deny_call(HostPath::new(ctx));
        }

        Self {
            ids,
            player_arg,
            ctx,
            monster,
            has_monster: options.has_monster,
            level_path,
            exp_path,
            quest_count_path,
            quest_done_path,
            inventory_gold_count_path,
            now_path,
            tick_path,
            adapter,
        }
    }

    pub(crate) fn main_args(&self, main: &CodeObject) -> Result<CallArgs<'static>, Box<dyn Error>> {
        let mut args = CallArgs::new();
        for param in &main.params {
            match param.as_str() {
                "player" => {
                    args.push_host_handle("player", self.player_arg);
                }
                "ctx" => {
                    args.push_host_handle("ctx", self.ctx);
                }
                "monster" => {
                    args.push_host_handle("monster", self.monster);
                }
                _ => return Err(format!("unsupported demo main parameter `{param}`").into()),
            }
        }
        Ok(args)
    }

    pub(crate) fn print_result(&self, result: OwnedValue) -> Result<(), Box<dyn Error>> {
        let level = self.read(&self.level_path)?;
        let now = self.read(&self.now_path)?;
        let tick = self.read(&self.tick_path)?;

        if self.adapter.method_calls().is_empty() {
            println!("result={result:?} level={level:?}");
        } else if self.has_monster {
            let exp = self.read(&self.exp_path)?;
            let quest_count = self.read(&self.quest_count_path)?;
            let quest_done = self.read(&self.quest_done_path)?;
            let inventory_gold = self.read(&self.inventory_gold_count_path)?;
            let rewards = self.method_call_count(self.ids.add_reward_method);
            let emits = self.method_call_count(self.ids.emit_method);
            println!(
                "result={result:?} level={level:?} exp={exp:?} quest_count={quest_count:?} \
                 quest_done={quest_done:?} inventory_gold={inventory_gold:?} \
                 reward_calls={rewards} emits={emits}",
            );
        } else {
            let emits = self.method_call_count(self.ids.emit_method);
            let logs = self.method_call_count(self.ids.log_method);
            if logs == 0 {
                println!(
                    "result={result:?} level={level:?} ctx_now={now:?} ctx_tick={tick:?} \
                     emits={emits}",
                );
            } else {
                println!(
                    "result={result:?} level={level:?} ctx_now={now:?} ctx_tick={tick:?} \
                     emits={emits} logs={logs}",
                );
            }
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
            .filter(|call| call.method == method)
            .count()
    }
}
