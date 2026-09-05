use vela_engine::engine::Engine;
use vela_engine::runtime::{CallArgs, CallOptions, Runtime};
use vela_vm::error::VmErrorKind;

#[test]
fn cyclic_script_result_is_rejected_without_aborting_the_runtime() {
    let engine = Engine::builder().build().unwrap();
    let program = engine.compile_source(
        "fn main() { let values = []; values.push(values); return values; } fn ok() { return 7; }",
    ).unwrap();
    let mut runtime = Runtime::new_compiled(engine, program).unwrap();
    let result = runtime
        .call(
            "main",
            CallArgs::new(),
            CallOptions::new(10_000, 1024 * 1024, 64),
        )
        .unwrap();
    assert_eq!(
        runtime.value_to_owned(&result).unwrap_err().kind(),
        VmErrorKind::OwnedValueCycle
    );
    let result = runtime
        .call(
            "ok",
            CallArgs::new(),
            CallOptions::new(10_000, 1024 * 1024, 64),
        )
        .unwrap();
    assert_eq!(
        runtime.value_to_owned(&result).unwrap(),
        vela_vm::owned_value::OwnedValue::i64(7)
    );
}
