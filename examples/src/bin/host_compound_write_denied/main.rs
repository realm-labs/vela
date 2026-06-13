use std::error::Error;

use vela_engine::prelude::*;
use vela_examples::{
    diagnostics, expect_error,
    gameplay::{self, GameEngineOptions, GameHostFixture, GameHostOptions},
};

const SOURCE_LABEL: &str = "host_compound_write_denied.vela";
const SOURCE: &str = include_str!("host_compound_write_denied.vela");

fn main() -> Result<(), Box<dyn Error>> {
    expect_error(
        || {
            run_example(
                GameEngineOptions::default(),
                GameHostOptions {
                    deny_player_level_write: true,
                    ..GameHostOptions::default()
                },
            )
        },
        "action: \"write\"",
    )
}

fn run_example(
    engine_options: GameEngineOptions,
    host_options: GameHostOptions,
) -> Result<(), Box<dyn Error>> {
    let engine = gameplay::build_engine(engine_options).map_err(|error| format!("{error:?}"))?;
    let program = engine
        .compile_source(SourceId::new(1), SOURCE)
        .map_err(|error| diagnostics::render_engine_source_error(SOURCE_LABEL, SOURCE, &error))?;
    let main = program
        .function("main")
        .ok_or("script must define fn main(...)")?;
    let mut host = GameHostFixture::for_main(main, host_options);
    let args = host.main_args(main)?;

    let mut runtime = Runtime::new(engine, program);
    let output = runtime
        .call_with_adapter(
            "main",
            args,
            CallOptions::new(10_000, 1024 * 1024, 64),
            host.adapter_mut(),
        )
        .map_err(|error| diagnostics::render_vm_error(SOURCE_LABEL, SOURCE, &error))?;
    let output = runtime.value_to_owned(&output)?;
    host.print_result(output)
}
