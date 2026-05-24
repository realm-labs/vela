use std::error::Error;
use std::fs;

use vela_bytecode::compiler::{CompilerOptions, compile_program_source_with_options};
use vela_common::SourceId;
use vela_host::PatchTx;
use vela_vm::{ExecutionBudget, HostExecution, Vm};

use self::ids::DemoIds;
use self::state::DemoHostState;

mod ids;
mod state;

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
            .with_host_field("quest_count", ids.quest_count_field)
            .with_host_field("quest_goal", ids.quest_goal_field)
            .with_host_field("quest_done", ids.quest_done_field)
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
