use std::env;
use std::path::Path;

mod diagnostics;

use vela_engine::prelude::*;

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    match args.as_slice() {
        [path] => run_script(Path::new(path)),
        _ => Err("usage: vela_cli <script-path>".into()),
    }
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
    println!("{:?}", output.value());
    Ok(())
}
