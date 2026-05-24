use std::error::Error;
use std::fs;

use vela_bytecode::{
    CodeObject,
    compiler::{CompilerOptions, compile_program_source_with_options},
};
use vela_common::{FieldId, HostMethodId, HostObjectId, HostTypeId, SourceId};
use vela_host::{HostPath, HostRef, HostValue, MockStateAdapter, PatchTx, ScriptStateAdapter};
use vela_vm::{ExecutionBudget, HostExecution, Value, Vm};

const PLAYER_TYPE: u32 = 1;
const CTX_TYPE: u32 = 2;
const MONSTER_TYPE: u32 = 3;
const PLAYER_OBJECT: u64 = 7;
const CTX_OBJECT: u64 = 100;
const MONSTER_OBJECT: u64 = 200;
const PLAYER_GENERATION: u32 = 3;
const CTX_GENERATION: u32 = 1;
const MONSTER_GENERATION: u32 = 1;
const LEVEL_FIELD: u32 = 2;
const NOW_FIELD: u32 = 3;
const TICK_FIELD: u32 = 4;
const EXP_FIELD: u32 = 6;
const ID_FIELD: u32 = 7;
const REWARD_COUNT_FIELD: u32 = 8;
const EMIT_METHOD: u32 = 5;
const ADD_REWARD_METHOD: u32 = 9;

pub(crate) fn run_script(path: &str) -> Result<(), Box<dyn Error>> {
    let source = fs::read_to_string(path)?;
    let ids = DemoIds::new();
    let program = compile_program_source_with_options(
        SourceId::new(1),
        &source,
        &CompilerOptions::new()
            .with_host_field("level", ids.level_field)
            .with_host_field("now", ids.now_field)
            .with_host_field("tick", ids.tick_field)
            .with_host_field("exp", ids.exp_field)
            .with_host_field("id", ids.id_field)
            .with_host_field("reward_count", ids.reward_count_field)
            .with_host_method("emit", ids.emit_method)
            .with_host_method("add_reward", ids.add_reward_method),
    )
    .map_err(|error| format!("{error:?}"))?;

    let main = program
        .function("main")
        .ok_or("script must define fn main(...)")?;
    let mut host_state =
        DemoHostState::new(ids, main.params.iter().any(|param| param == "monster"));
    let args = host_state.main_args(main)?;

    let mut tx = PatchTx::new();
    let mut budget = ExecutionBudget::new(10_000, 1024 * 1024, 64, 1024);
    let result = {
        let mut host = HostExecution {
            adapter: &mut host_state.adapter,
            tx: &mut tx,
        };
        Vm::new()
            .run_program_with_host_managed_heap_and_budget(
                &program,
                "main",
                &args,
                &mut host,
                &mut budget,
            )
            .map_err(|error| format!("{error:?}"))?
    };
    let patch_count = tx.patches().len();
    tx.apply(&mut host_state.adapter)
        .map_err(|error| format!("{error:?}"))?;
    host_state.print_result(result, patch_count)
}

#[derive(Clone, Copy)]
struct DemoIds {
    level_field: FieldId,
    now_field: FieldId,
    tick_field: FieldId,
    exp_field: FieldId,
    id_field: FieldId,
    reward_count_field: FieldId,
    emit_method: HostMethodId,
    add_reward_method: HostMethodId,
}

impl DemoIds {
    fn new() -> Self {
        Self {
            level_field: FieldId::new(LEVEL_FIELD),
            now_field: FieldId::new(NOW_FIELD),
            tick_field: FieldId::new(TICK_FIELD),
            exp_field: FieldId::new(EXP_FIELD),
            id_field: FieldId::new(ID_FIELD),
            reward_count_field: FieldId::new(REWARD_COUNT_FIELD),
            emit_method: HostMethodId::new(EMIT_METHOD),
            add_reward_method: HostMethodId::new(ADD_REWARD_METHOD),
        }
    }
}

struct DemoHostState {
    ids: DemoIds,
    player: HostRef,
    ctx: HostRef,
    monster: HostRef,
    has_monster: bool,
    level_path: HostPath,
    exp_path: HostPath,
    now_path: HostPath,
    tick_path: HostPath,
    adapter: MockStateAdapter,
}

impl DemoHostState {
    fn new(ids: DemoIds, has_monster: bool) -> Self {
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
            now_path,
            tick_path,
            adapter,
        }
    }

    fn main_args(&self, main: &CodeObject) -> Result<Vec<Value>, Box<dyn Error>> {
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

    fn print_result(&self, result: Value, patch_count: usize) -> Result<(), Box<dyn Error>> {
        let level = self
            .adapter
            .read_path(&self.level_path)
            .map_err(|error| format!("{error:?}"))?;
        let now = self
            .adapter
            .read_path(&self.now_path)
            .map_err(|error| format!("{error:?}"))?;
        let tick = self
            .adapter
            .read_path(&self.tick_path)
            .map_err(|error| format!("{error:?}"))?;

        if self.adapter.method_calls().is_empty() {
            println!("result={result:?} level={level:?} patches={patch_count}");
        } else if self.has_monster {
            let exp = self
                .adapter
                .read_path(&self.exp_path)
                .map_err(|error| format!("{error:?}"))?;
            let rewards = self
                .adapter
                .method_calls()
                .iter()
                .filter(|(_, method, _)| *method == self.ids.add_reward_method)
                .count();
            let emits = self
                .adapter
                .method_calls()
                .iter()
                .filter(|(_, method, _)| *method == self.ids.emit_method)
                .count();
            println!(
                "result={result:?} level={level:?} exp={exp:?} rewards={rewards} \
                 emits={emits} patches={patch_count}",
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
}
