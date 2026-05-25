use std::error::Error;

use vela_engine::{CallOptions, Runtime};
use vela_host::PatchTx;
use vela_hot_reload::HotReloadAbi;

use self::ids::DemoIds;
use self::registry::{demo_engine, demo_type_registry};
use self::state::DemoHostState;

mod ids;
mod registry;
mod state;

pub(crate) fn run_script(path: &str) -> Result<(), Box<dyn Error>> {
    let ids = DemoIds::new();
    let engine = demo_engine(ids).map_err(|error| format!("{error:?}"))?;
    let program = engine
        .compile_file(path)
        .map_err(|error| format!("{error:?}"))?;

    let main = program
        .function("main")
        .ok_or("script must define fn main(...)")?;
    let mut host_state =
        DemoHostState::new(ids, main.params.iter().any(|param| param == "monster"));
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
        .map_err(|error| format!("{error:?}"))?;
    let patch_count = tx.patches().len();
    tx.apply(&mut host_state.adapter)
        .map_err(|error| format!("{error:?}"))?;
    host_state.print_result(result, patch_count)
}

pub(crate) fn hot_reload_abi() -> HotReloadAbi {
    HotReloadAbi::from_registry(&demo_type_registry(DemoIds::new()))
}
