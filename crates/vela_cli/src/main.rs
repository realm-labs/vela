use std::path::{Path, PathBuf};

mod diagnostics;

use clap::Parser;
use vela_engine::prelude::*;

#[derive(Debug, Parser)]
#[command(
    name = "vela_cli",
    about = "Run a Vela script file",
    version,
    disable_help_subcommand = true
)]
struct Cli {
    /// Vela script file to execute.
    script: PathBuf,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    run_script(&cli.script)
}

fn run_script(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let engine = Engine::builder()
        .with_standard_natives()
        .capability(Capability::Time)
        .capability(Capability::Random)
        .with_time_clock(1_700_000_000, 42)
        .with_controlled_random(7)
        .build()
        .map_err(|error| format!("{error:?}"))?;
    let program = engine
        .compile_file(path)
        .map_err(|error| diagnostics::render_engine_source_error(path, &error))?;
    let mut runtime = Runtime::new(engine, program);
    let output = runtime
        .call(
            "main",
            CallArgs::new(),
            CallOptions::new(10_000, 1024 * 1024, 64),
        )
        .map_err(|error| diagnostics::render_vm_error(path, &error))?;
    println!("{:?}", runtime.value_to_owned(&output)?);
    Ok(())
}
