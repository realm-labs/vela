use std::error::Error;

use vela_engine::prelude::*;

fn main() -> Result<(), Box<dyn Error>> {
    let root = std::env::temp_dir().join(format!("vela_io_stdlib_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root)?;
    std::fs::write(root.join("input.txt"), "hello from fs")?;

    let engine = Engine::builder()
        .with_standard_natives()
        .capability(Capability::IoRead)
        .capability(Capability::IoWrite)
        .with_stdio()
        .with_fs_io(&root)
        .build()?;
    let program = engine.compile_source(include_str!("main.vela"))?;
    let mut runtime = Runtime::new(engine, program);

    let output = runtime.call("main", CallArgs::new(), CallOptions::unbounded())?;
    let written = std::fs::read_to_string(root.join("output.txt"))?;
    println!(
        "io_stdlib len={:?} output={written}",
        runtime.value_to_owned(&output)?
    );
    Ok(())
}
