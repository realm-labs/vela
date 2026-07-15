#![cfg_attr(not(test), deny(clippy::wildcard_imports))]

use std::path::{Path, PathBuf};

mod diagnostics;
mod executor;

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
    /// Drive an async `main` with the CLI-owned executor.
    #[arg(long = "async")]
    async_entry: bool,

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
    run_script(&cli.script, cli.async_entry)
}

fn run_script(path: &Path, async_entry: bool) -> Result<(), Box<dyn std::error::Error>> {
    let fs_root = path.parent().unwrap_or_else(|| Path::new("."));
    let engine = Engine::builder()
        .with_standard_natives()
        .capability(Capability::Time)
        .capability(Capability::Random)
        .capability(Capability::IoRead)
        .capability(Capability::IoWrite)
        .with_time_clock(1_700_000_000, 42)
        .with_controlled_random(7)
        .with_stdio()
        .with_fs_io(fs_root)
        .build()
        .map_err(|error| error.to_string())?;
    let program = engine
        .compile_file(path)
        .map_err(|error| diagnostics::render_engine_source_error(path, &error))?;
    let mut runtime = Runtime::new_compiled(engine, program)?;
    let args = CallArgs::new();
    let options = CallOptions::new(10_000, 1024 * 1024, 64);
    let output = if async_entry {
        executor::block_on(runtime.call_async("main", args, options))
    } else {
        runtime.call("main", args, options)
    }
    .map_err(|error| diagnostics::render_vm_error(path, &error))?;
    println!("{:?}", runtime.value_to_owned(&output)?);
    Ok(())
}
