#![cfg_attr(not(test), deny(clippy::wildcard_imports))]

use std::error::Error;

use vela_engine::prelude::*;
use vela_examples::temp_dir::TempDir;

fn main() -> Result<(), Box<dyn Error>> {
    let root = TempDir::new()?;
    std::fs::write(root.path().join("input.txt"), "hello from fs")?;

    let engine = Engine::builder()
        .with_standard_natives()
        .capability(Capability::IoRead)
        .capability(Capability::IoWrite)
        .with_stdio()
        .with_fs_io(root.path())
        .build()?;
    let program = engine.compile_source(include_str!("main.vela"))?;
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");

    let output = runtime.call("main", CallArgs::new(), CallOptions::unbounded())?;
    let written = std::fs::read_to_string(root.path().join("output.txt"))?;
    println!(
        "io_stdlib len={:?} output={written}",
        runtime.value_to_owned(&output)?
    );
    Ok(())
}
