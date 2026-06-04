use std::error::Error;
use std::path::Path;

use vela_engine::engine::Engine;
use vela_engine::error::EngineResult;
use vela_engine::runtime::{CallOptions, Runtime};
use vela_host::tx::PatchTx;

use self::ids::DemoIds;
use self::registry::demo_engine;
use self::state::DemoHostState;

mod ids;
mod registry;
mod state;

pub(crate) fn run_script(path: &str) -> Result<(), Box<dyn Error>> {
    run_script_with_options(path, DemoRunOptions::default())
}

pub(crate) fn run_script_with_random(path: &str) -> Result<(), Box<dyn Error>> {
    run_script_with_options(
        path,
        DemoRunOptions {
            engine: DemoEngineOptions { allow_random: true },
            ..DemoRunOptions::default()
        },
    )
}

pub(crate) fn run_script_with_stale_player(path: &str) -> Result<(), Box<dyn Error>> {
    run_script_with_options(
        path,
        DemoRunOptions {
            stale_player: true,
            ..DemoRunOptions::default()
        },
    )
}

fn run_script_with_options(path: &str, options: DemoRunOptions) -> Result<(), Box<dyn Error>> {
    let ids = DemoIds::new();
    let engine = build_engine(ids, options.engine).map_err(|error| format!("{error:?}"))?;
    let path = Path::new(path);
    let program = engine
        .compile_file(path)
        .map_err(|error| crate::diagnostics::render_engine_source_error(path, &error))?;

    let main = program
        .function("main")
        .ok_or("script must define fn main(...)")?;
    let mut host_state = DemoHostState::new(
        ids,
        main.params.iter().any(|param| param == "monster"),
        options.stale_player,
    );
    let args = host_state.main_args(main)?;

    let mut tx = PatchTx::new();
    let mut runtime = Runtime::new(engine, program);
    let result = runtime
        .call(
            "main",
            &args,
            CallOptions::new(10_000, 1024 * 1024, 64, 1024),
            &mut host_state.adapter,
            &mut tx,
        )
        .map_err(|error| crate::diagnostics::render_vm_error(path, &error))?;
    let patch_count = tx.patches().len();
    tx.apply(&mut host_state.adapter)
        .map_err(|error| format!("{error:?}"))?;
    host_state.print_result(result, patch_count)
}

pub(crate) fn hot_reload_engine() -> EngineResult<Engine> {
    build_engine(DemoIds::new(), DemoEngineOptions::default())
}

fn build_engine(ids: DemoIds, options: DemoEngineOptions) -> EngineResult<Engine> {
    demo_engine(ids, options)
}

#[derive(Clone, Copy, Debug, Default)]
struct DemoRunOptions {
    engine: DemoEngineOptions,
    stale_player: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct DemoEngineOptions {
    pub(crate) allow_random: bool,
}
