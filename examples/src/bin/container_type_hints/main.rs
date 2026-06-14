use std::error::Error;

use vela_engine::prelude::*;

fn main() -> Result<(), Box<dyn Error>> {
    let engine = Engine::builder()
        .with_standard_natives()
        .execution_profile(ExecutionProfile::trusted())
        .build()?;
    let program = engine.compile_source(include_str!("main.vela"))?;
    let mut runtime = Runtime::new(engine, program);

    let output = runtime.call("main", CallArgs::new(), CallOptions::unbounded())?;

    println!(
        "container_type_hints result={:?}",
        runtime.value_to_owned(&output)?
    );
    Ok(())
}
