use std::error::Error;

use vela_engine::prelude::*;
use vela_examples::example_dir;

fn main() -> Result<(), Box<dyn Error>> {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()?;
    let program = engine.compile_dir(example_dir("modules"))?;
    let mut runtime = Runtime::new(engine, program);

    let output = runtime.call(
        "game::main::main",
        CallArgs::new(),
        CallOptions::new(10_000, 1024 * 1024, 64),
    )?;

    println!("module_result={:?}", output.value());
    Ok(())
}
