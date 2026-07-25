#![cfg(feature = "artifact-codec")]

use vela_bytecode::PortableProgramArtifact;
use vela_engine::engine::Engine;
use vela_engine::runtime::{CallArgs, CallOptions, Runtime};
use vela_vm::owned_value::OwnedValue;

#[test]
fn decoded_portable_program_executes_without_source_compilation() {
    let build_engine = Engine::builder().build().expect("build compiler engine");
    let compiled = build_engine
        .compile_source("fn main() { return 40 + 2; }")
        .expect("compile source outside receiving engine");
    let bytes = PortableProgramArtifact::from_compiled(compiled)
        .expect("create portable artifact")
        .encode()
        .expect("encode portable artifact");

    let receiving_engine = Engine::builder().build().expect("build receiving engine");
    let decoded = PortableProgramArtifact::decode(&bytes).expect("decode portable artifact");
    let linked = receiving_engine
        .link_portable_program(decoded.into_compiled())
        .expect("bind stable identities");
    let mut runtime =
        Runtime::from_linked_artifact(receiving_engine, linked).expect("build portable runtime");
    let value = runtime
        .call(
            "main",
            CallArgs::from_positional([]),
            CallOptions::unbounded(),
        )
        .expect("execute portable bytecode");

    assert_eq!(
        runtime.value_to_owned(&value).expect("detach result"),
        OwnedValue::i64(42)
    );
}
