use vela_engine::engine::Engine;
use vela_engine::runtime::{CallArgs, CallOptions, Runtime};
use vela_vm::error::VmErrorKind;

#[test]
fn cyclic_script_result_is_rejected_without_aborting_the_runtime() {
    let engine = Engine::builder()
        .build()
        .expect("engine fixture should build");
    let program = engine.compile_source(
        "fn main() { let values = []; values.push(values); return values; } fn ok() { return 7; }",
    ).expect("valid export fixture operation should succeed");
    let mut runtime = Runtime::new_compiled(engine, program)
        .expect("valid export fixture operation should succeed");
    let result = runtime
        .call(
            "main",
            CallArgs::new(),
            CallOptions::new(10_000, 1024 * 1024, 64),
        )
        .expect("valid export fixture operation should succeed");
    assert_eq!(
        runtime
            .value_to_owned(&result)
            .expect_err("cyclic or over-limit export must reject")
            .kind(),
        VmErrorKind::OwnedValueCycle
    );
    let result = runtime
        .call(
            "ok",
            CallArgs::new(),
            CallOptions::new(10_000, 1024 * 1024, 64),
        )
        .expect("valid export fixture operation should succeed");
    assert_eq!(
        runtime
            .value_to_owned(&result)
            .expect("valid export fixture operation should succeed"),
        vela_vm::owned_value::OwnedValue::i64(7)
    );
}
