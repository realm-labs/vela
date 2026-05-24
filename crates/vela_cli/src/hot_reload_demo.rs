use std::error::Error;
use std::fs;

use vela_common::SourceId;
use vela_hot_reload::{HotReloadRuntime, compile_initial_with_abi, compile_update_with_abi};
use vela_vm::{Value, Vm};

pub(crate) fn run(initial_path: &str, updated_path: &str) -> Result<(), Box<dyn Error>> {
    let initial_source = fs::read_to_string(initial_path)?;
    let updated_source = fs::read_to_string(updated_path)?;
    let abi = crate::demo::hot_reload_abi();
    let initial = compile_initial_with_abi(SourceId::new(1), &initial_source, abi.clone())
        .map_err(|error| format!("{error:?}"))?;
    let mut runtime = HotReloadRuntime::new(initial);
    let old = runtime.current();
    let old_before = run_main(&old.to_program())?;

    let update = compile_update_with_abi(&old, SourceId::new(2), &updated_source, abi)
        .map_err(|error| format!("{error:?}"))?;
    let new = runtime
        .apply_hot_update(update)
        .map_err(|error| format!("{error:?}"))?;
    let old_after = run_main(&old.to_program())?;
    let new_after = run_main(&new.to_program())?;

    println!(
        "old_version={} new_version={} abi=checked old_before={old_before:?} \
         old_after={old_after:?} new_after={new_after:?}",
        old.id.0, new.id.0,
    );
    Ok(())
}

fn run_main(program: &vela_bytecode::Program) -> Result<Value, Box<dyn Error>> {
    Vm::new()
        .run_program(program, "main", &[])
        .map_err(|error| format!("{error:?}").into())
}
