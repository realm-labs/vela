use std::error::Error;

use vela_common::SourceId;
use vela_engine::engine::Engine;
use vela_engine::error::EngineResult;
use vela_engine::runtime::{CallOptions, Runtime};

use self::ids::DemoIds;
use self::registry::demo_engine;
use self::state::{DemoHostOptions, DemoHostState};

mod ids;
mod registry;
mod state;

pub fn run_script(label: &str, source: &str) -> Result<(), Box<dyn Error>> {
    run_script_with_options(label, source, DemoRunOptions::default())
}

pub fn run_script_with_random(label: &str, source: &str) -> Result<(), Box<dyn Error>> {
    run_script_with_options(
        label,
        source,
        DemoRunOptions {
            engine: DemoEngineOptions { allow_random: true },
            ..DemoRunOptions::default()
        },
    )
}

pub fn run_script_with_stale_player(label: &str, source: &str) -> Result<(), Box<dyn Error>> {
    run_script_with_options(
        label,
        source,
        DemoRunOptions {
            host: DemoHostOptions {
                stale_player_arg: true,
                ..DemoHostOptions::default()
            },
            ..DemoRunOptions::default()
        },
    )
}

pub fn run_script_with_denied_player_level_read(
    label: &str,
    source: &str,
) -> Result<(), Box<dyn Error>> {
    run_script_with_options(
        label,
        source,
        DemoRunOptions {
            host: DemoHostOptions {
                deny_player_level_read: true,
                ..DemoHostOptions::default()
            },
            ..DemoRunOptions::default()
        },
    )
}

pub fn run_script_with_denied_player_level_write(
    label: &str,
    source: &str,
) -> Result<(), Box<dyn Error>> {
    run_script_with_options(
        label,
        source,
        DemoRunOptions {
            host: DemoHostOptions {
                deny_player_level_write: true,
                ..DemoHostOptions::default()
            },
            ..DemoRunOptions::default()
        },
    )
}

pub fn run_script_with_denied_context_emit_call(
    label: &str,
    source: &str,
) -> Result<(), Box<dyn Error>> {
    run_script_with_options(
        label,
        source,
        DemoRunOptions {
            host: DemoHostOptions {
                deny_context_emit_call: true,
                ..DemoHostOptions::default()
            },
            ..DemoRunOptions::default()
        },
    )
}

fn run_script_with_options(
    label: &str,
    source: &str,
    options: DemoRunOptions,
) -> Result<(), Box<dyn Error>> {
    let ids = DemoIds::new();
    let engine = build_engine(ids, options.engine).map_err(|error| format!("{error:?}"))?;
    let program = engine
        .compile_source(SourceId::new(1), source)
        .map_err(|error| crate::diagnostics::render_engine_source_error(label, source, &error))?;

    let main = program
        .function("main")
        .ok_or("script must define fn main(...)")?;
    let mut host_options = options.host;
    host_options.has_monster = main.params.iter().any(|param| param == "monster");
    let mut host_state = DemoHostState::new(ids, host_options);
    let args = host_state.main_args(main)?;

    let mut runtime = Runtime::new(engine, program);
    let output = runtime
        .call_with_adapter(
            "main",
            args,
            CallOptions::new(10_000, 1024 * 1024, 64),
            &mut host_state.adapter,
        )
        .map_err(|error| crate::diagnostics::render_vm_error(label, source, &error))?;
    let output = runtime.value_to_owned(&output)?;
    host_state.print_result(output)
}

pub fn hot_reload_engine() -> EngineResult<Engine> {
    build_engine(DemoIds::new(), DemoEngineOptions::default())
}

fn build_engine(ids: DemoIds, options: DemoEngineOptions) -> EngineResult<Engine> {
    demo_engine(ids, options)
}

#[derive(Clone, Copy, Debug, Default)]
struct DemoRunOptions {
    engine: DemoEngineOptions,
    host: DemoHostOptions,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct DemoEngineOptions {
    pub(crate) allow_random: bool,
}
