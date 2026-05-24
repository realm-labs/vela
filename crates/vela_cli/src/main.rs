use std::env;
use std::error::Error;
use std::fs;

use vela_bytecode::compiler::{CompilerOptions, compile_program_source_with_options};
use vela_common::{FieldId, HostObjectId, HostTypeId, SourceId};
use vela_host::{HostPath, HostRef, HostValue, MockStateAdapter, PatchTx, ScriptStateAdapter};
use vela_vm::{HostExecution, Value, Vm};

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let path = env::args().nth(1).ok_or("usage: vela_cli <script-path>")?;
    let source = fs::read_to_string(&path)?;
    let level_field = FieldId::new(2);
    let program = compile_program_source_with_options(
        SourceId::new(1),
        &source,
        &CompilerOptions::new().with_host_field("level", level_field),
    )
    .map_err(|error| format!("{error:?}"))?;

    let player = HostRef::new(HostTypeId::new(1), HostObjectId::new(7), 3);
    let level_path = HostPath::new(player).field(level_field);
    let mut adapter = MockStateAdapter::new();
    adapter.insert_value(level_path.clone(), HostValue::Int(9));
    let mut tx = PatchTx::new();
    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };
        Vm::new()
            .run_program_with_host(&program, "main", &[Value::HostRef(player)], &mut host)
            .map_err(|error| format!("{error:?}"))?
    };
    let patch_count = tx.patches().len();
    tx.apply(&mut adapter)
        .map_err(|error| format!("{error:?}"))?;
    let level = adapter
        .read_path(&level_path)
        .map_err(|error| format!("{error:?}"))?;

    println!("result={result:?} level={level:?} patches={patch_count}");
    Ok(())
}
