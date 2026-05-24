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
const PLAYER_OBJECT: u64 = 7;
const CTX_OBJECT: u64 = 100;
const PLAYER_GENERATION: u32 = 3;
const CTX_GENERATION: u32 = 1;
const LEVEL_FIELD: u32 = 2;
const NOW_FIELD: u32 = 3;
const TICK_FIELD: u32 = 4;
const EMIT_METHOD: u32 = 5;

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
            .with_host_method("emit", ids.emit_method),
    )
    .map_err(|error| format!("{error:?}"))?;

    let main = program
        .function("main")
        .ok_or("script must define fn main(...)")?;
    let mut host_state = DemoHostState::new(ids);
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
    emit_method: HostMethodId,
}

impl DemoIds {
    fn new() -> Self {
        Self {
            level_field: FieldId::new(LEVEL_FIELD),
            now_field: FieldId::new(NOW_FIELD),
            tick_field: FieldId::new(TICK_FIELD),
            emit_method: HostMethodId::new(EMIT_METHOD),
        }
    }
}

struct DemoHostState {
    player: HostRef,
    ctx: HostRef,
    level_path: HostPath,
    now_path: HostPath,
    tick_path: HostPath,
    adapter: MockStateAdapter,
}

impl DemoHostState {
    fn new(ids: DemoIds) -> Self {
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
        let level_path = HostPath::new(player).field(ids.level_field);
        let now_path = HostPath::new(ctx).field(ids.now_field);
        let tick_path = HostPath::new(ctx).field(ids.tick_field);
        let mut adapter = MockStateAdapter::new();
        adapter.insert_value(level_path.clone(), HostValue::Int(9));
        adapter.insert_value(now_path.clone(), HostValue::Int(1_700_000_000));
        adapter.insert_value(tick_path.clone(), HostValue::Int(42));
        adapter.insert_method_return(ids.emit_method, HostValue::Null);

        Self {
            player,
            ctx,
            level_path,
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
