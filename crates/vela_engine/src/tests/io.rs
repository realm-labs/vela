use std::path::PathBuf;

use vela_bytecode::UnlinkedProgram;
use vela_common::SourceId;
use vela_vm::error::{VmErrorKind, VmResult};
use vela_vm::owned_value::OwnedValue;

use crate::engine::Engine;
use crate::io::{
    FS_READ_TO_STRING_FUNCTION_ID, FS_WRITE_STRING_FUNCTION_ID, IO_PRINT_FUNCTION_ID,
    IO_PRINTLN_FUNCTION_ID,
};
use crate::permission::Capability;
use crate::runtime::{CallArgs, CallOptions, Runtime};

fn run_linked_program(engine: &Engine, program: &UnlinkedProgram) -> VmResult<OwnedValue> {
    let linked = engine
        .link_program(program)
        .expect("engine io test program should link");
    engine
        .into_vm_for_program(program)
        .run_linked_program(&linked, "main", &[])
}

#[test]
fn fs_read_requires_io_read_capability() {
    let root = temp_root("fs_read_requires_io_read_capability");
    let engine = Engine::builder()
        .with_standard_natives()
        .with_fs_io(&root)
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main() {
    return fs::read_to_string("input.txt");
}
"#,
        )
        .expect("program should compile");

    assert!(matches!(
        run_linked_program(&engine, &program),
        Err(error) if error.kind() == VmErrorKind::PermissionDenied {
            native: "fs::read_to_string".to_owned(),
            capability: "io_read".to_owned(),
        }
    ));
}

#[test]
fn fs_read_and_write_use_sandboxed_paths() {
    let root = temp_root("fs_read_and_write_use_sandboxed_paths");
    let engine = Engine::builder()
        .with_standard_natives()
        .capability(Capability::IoRead)
        .capability(Capability::IoWrite)
        .with_fs_io(&root)
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main() {
    fs::write_string("state.txt", "score=7");
    return result::unwrap_or(fs::read_to_string("state.txt"), "missing");
}
"#,
        )
        .expect("program should compile");

    assert_eq!(
        run_linked_program(&engine, &program),
        Ok(OwnedValue::String("score=7".to_owned()))
    );
}

#[test]
fn runtime_new_links_stdio_and_fs_io_programs() {
    let root = temp_root("runtime_new_links_stdio_and_fs_io_programs");
    std::fs::write(root.join("input.txt"), "hello from fs").expect("input fixture should write");
    let engine = Engine::builder()
        .with_standard_natives()
        .capability(Capability::IoRead)
        .capability(Capability::IoWrite)
        .with_stdio()
        .with_fs_io(&root)
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main() {
    let input = result::unwrap_or(fs::read_to_string("input.txt"), "missing");
    io::print("hello");
    io::println(" from fs");
    fs::write_string("output.txt", "done");
    return input.len();
}
"#,
        )
        .expect("program should compile");
    engine
        .link_program(&program)
        .expect("stdio plus fs program should link");
    let mut runtime = Runtime::new(engine, program);

    let output = runtime
        .call("main", CallArgs::new(), CallOptions::unbounded())
        .expect("runtime call should execute linked image");

    assert_eq!(
        runtime.value_to_owned(&output),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(13)))
    );
    assert_eq!(
        std::fs::read_to_string(root.join("output.txt")).expect("output should exist"),
        "done"
    );
}

#[test]
fn fs_rejects_parent_directory_escape() {
    let root = temp_root("fs_rejects_parent_directory_escape");
    let engine = Engine::builder()
        .with_standard_natives()
        .capability(Capability::IoRead)
        .with_fs_io(&root)
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            SourceId::new(1),
            r#"
fn main() {
    return result::is_err(fs::read_to_string("../secret.txt"));
}
"#,
        )
        .expect("program should compile");

    assert_eq!(
        run_linked_program(&engine, &program),
        Ok(OwnedValue::Bool(true))
    );
}

#[test]
fn io_stdlib_registers_metadata() {
    let root = temp_root("io_stdlib_registers_metadata");
    let engine = Engine::builder()
        .with_stdio()
        .with_fs_io(&root)
        .build()
        .expect("engine should build");
    let registry = engine.registry();

    assert_eq!(
        registry
            .function_by_name("io::print")
            .expect("print metadata")
            .id,
        IO_PRINT_FUNCTION_ID
    );
    assert_eq!(
        registry
            .function_by_name("io::println")
            .expect("println metadata")
            .id,
        IO_PRINTLN_FUNCTION_ID
    );
    assert_eq!(
        registry
            .function_by_name("fs::read_to_string")
            .expect("read metadata")
            .id,
        FS_READ_TO_STRING_FUNCTION_ID
    );
    assert_eq!(
        registry
            .function_by_name("fs::write_string")
            .expect("write metadata")
            .id,
        FS_WRITE_STRING_FUNCTION_ID
    );
    assert!(
        registry
            .function_by_name("io::print")
            .expect("print metadata")
            .effects
            .writes_io
    );
    assert!(
        registry
            .function_by_name("io::println")
            .expect("println metadata")
            .effects
            .writes_io
    );
    assert!(
        registry
            .function_by_name("fs::read_to_string")
            .expect("read metadata")
            .effects
            .reads_io
    );
    assert!(
        registry
            .function_by_name("fs::write_string")
            .expect("write metadata")
            .effects
            .writes_io
    );
}

fn temp_root(name: &str) -> PathBuf {
    let mut root = std::env::temp_dir();
    root.push(format!("vela_engine_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("temp root should be created");
    root
}
