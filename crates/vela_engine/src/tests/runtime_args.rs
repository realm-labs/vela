#[cfg(feature = "serde")]
use std::collections::BTreeMap;

use vela_common::SourceId;
use vela_vm::error::VmErrorKind;
use vela_vm::owned_value::OwnedValue;

use crate::engine::Engine;
use crate::runtime::{
    CallArgs, CallOptions, Runtime, RuntimeImage, SharedRuntime, VelaFunction, VelaMethod,
    VelaValue,
};

mod managed_values;
mod state_reclamation;

#[test]
fn runtime_and_runtime_values_are_send() {
    fn assert_send<T: Send>() {}

    assert_send::<Runtime>();
    assert_send::<SharedRuntime>();
    assert_send::<VelaValue>();
    assert_send::<VelaFunction>();
    assert_send::<VelaMethod>();
}

#[test]
fn runtime_call_executes_program_image_code_view() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            include_str!("../../tests/fixtures/state/program_image.vela"),
        )
        .expect("compile runtime image code-view source");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");

    let value = runtime
        .call("main", CallArgs::new(), CallOptions::unbounded())
        .expect("runtime call should execute");

    assert_eq!(
        runtime.value_to_owned(&value),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(16)))
    );
}

#[test]
fn runtime_call_checks_public_entry_parameter_contracts() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
fn main(value: i64) {
    return value;
}
"#,
        )
        .expect("program should compile");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");

    let error = runtime
        .call(
            "main",
            CallArgs::new().with_value("value", "bad"),
            CallOptions::unbounded(),
        )
        .expect_err("runtime host entry should check parameter guards");

    assert_eq!(
        error.kind(),
        VmErrorKind::TypeContractViolation {
            expected: "i64".to_owned(),
            actual: "String".to_owned(),
            debug_name: "value".to_owned(),
        }
    );
}

#[test]
fn runtime_call_args_accept_positional_values() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
fn main(left, right) {
    return left * 10 + right;
}
"#,
        )
        .expect("program should compile");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");

    let args = CallArgs::from_positional([
        OwnedValue::Scalar(vela_common::ScalarValue::I64(2)),
        OwnedValue::Scalar(vela_common::ScalarValue::I64(7)),
    ]);

    let result = runtime
        .call_args_raw("main", args, CallOptions::unbounded())
        .expect("runtime call args should run");

    assert_eq!(
        result,
        OwnedValue::Scalar(vela_common::ScalarValue::I64(27))
    );
}

#[test]
fn runtime_call_args_reject_duplicate_named_values() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source_with_id(SourceId::new(1), "fn main(value) { return value; }")
        .expect("program should compile");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");

    let args = CallArgs::new()
        .with_value("value", 1_i64)
        .with_value("value", 2_i64);

    let error = runtime
        .call_args_raw("main", args, CallOptions::unbounded())
        .expect_err("duplicate named args should fail");

    assert_eq!(
        error.kind(),
        VmErrorKind::TypeMismatch {
            operation: "duplicate named call argument"
        }
    );
}

#[test]
fn runtime_call_args_reject_unknown_named_values() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source_with_id(SourceId::new(1), "fn main(value) { return value; }")
        .expect("program should compile");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");

    let args = CallArgs::new().with_value("missing", 1_i64);

    let error = runtime
        .call_args_raw("main", args, CallOptions::unbounded())
        .expect_err("unknown named args should fail");

    assert_eq!(
        error.kind(),
        VmErrorKind::TypeMismatch {
            operation: "unknown named call argument"
        }
    );
}

#[test]
fn runtime_call_args_reject_mixed_modes() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source_with_id(SourceId::new(1), "fn main(value) { return value; }")
        .expect("program should compile");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");

    let args = CallArgs::new().with(1_i64).with_value("value", 2_i64);

    let error = runtime
        .call_args_raw("main", args, CallOptions::unbounded())
        .expect_err("mixed args should fail");

    assert_eq!(
        error.kind(),
        VmErrorKind::TypeMismatch {
            operation: "mixed positional and named call arguments"
        }
    );
}

#[test]
fn runtime_cached_entry_calls_function() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
fn main(amount, multiplier = 2) {
    return amount * multiplier;
}
"#,
        )
        .expect("program should compile");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");
    let main = runtime.entry("main").expect("entry should resolve");

    let first = runtime
        .call(
            &main,
            CallArgs::new().with_value("amount", 7_i64),
            CallOptions::unbounded(),
        )
        .expect("cached entry should run with default args");
    let second = runtime
        .call(
            &main,
            CallArgs::new()
                .with_value("amount", 7_i64)
                .with_value("multiplier", 3_i64),
            CallOptions::unbounded(),
        )
        .expect("cached entry should run with named args");

    assert_eq!(main.name(), "main");
    assert_eq!(
        runtime.value_to_owned(&first),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(14)))
    );
    assert_eq!(
        runtime.value_to_owned(&second),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(21)))
    );
}

#[test]
fn runtime_cached_entry_rejects_function_from_another_runtime() {
    let engine = Engine::builder().build().expect("engine should build");
    let source = r#"
fn main(amount) {
    return amount * 2;
}
"#;
    let program_a = engine
        .compile_source_with_id(SourceId::new(1), source)
        .expect("program should compile");
    let program_b = engine
        .compile_source_with_id(SourceId::new(2), source)
        .expect("program should compile");
    let runtime_a = Runtime::new(engine.clone(), program_a).expect("runtime should initialize");
    let mut runtime_b = Runtime::new(engine, program_b).expect("runtime should initialize");
    let main = runtime_a.entry("main").expect("entry should resolve");

    let error = runtime_b
        .call(
            &main,
            CallArgs::from_positional([OwnedValue::Scalar(vela_common::ScalarValue::I64(7))]),
            CallOptions::unbounded(),
        )
        .expect_err("cached entry from another runtime should fail");

    assert_eq!(
        error.kind(),
        VmErrorKind::TypeMismatch {
            operation: "VelaFunction belongs to another Runtime"
        }
    );
}

#[test]
fn runtime_call_method_on_runtime_value_by_name_and_cached_method() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            include_str!("../../tests/fixtures/state/cached_methods.vela"),
        )
        .expect("program should compile");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");
    let reward = runtime
        .call("make_reward", CallArgs::new(), CallOptions::unbounded())
        .expect("factory should return runtime value");
    let score_by_name_target = runtime
        .bind_method(&reward, "score")
        .expect("named method target should bind");
    let score_by_name = runtime
        .call(
            score_by_name_target,
            CallArgs::new().with_value("amount", 5_i64),
            CallOptions::unbounded(),
        )
        .expect("named value method should run");
    let score_method = runtime
        .method(&reward, "score")
        .expect("method should resolve");
    let score_by_cached_target = runtime
        .bind_method(&reward, &score_method)
        .expect("cached method target should bind");
    let score_by_cached_method = runtime
        .call(
            score_by_cached_target,
            CallArgs::new()
                .with_value("amount", 3_i64)
                .with_value("multiplier", 4_i64),
            CallOptions::unbounded(),
        )
        .expect("cached value method should run");

    assert_eq!(score_method.name(), "score");
    assert_eq!(
        score_method.receiver_type(),
        vela_def::script_type_id(
            vela_package::PackageId::anonymous().as_str(),
            "Reward",
            None,
        )
    );
    assert_eq!(
        runtime.value_to_owned(&score_by_name),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(17)))
    );
    assert_eq!(
        runtime.value_to_owned(&score_by_cached_method),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(19)))
    );
}

#[test]
fn runtime_cached_method_rejects_method_from_another_runtime() {
    let engine = Engine::builder().build().expect("engine should build");
    let source = include_str!("../../tests/fixtures/state/method_affinity.vela");
    let program_a = engine
        .compile_source_with_id(SourceId::new(1), source)
        .expect("program should compile");
    let program_b = engine
        .compile_source_with_id(SourceId::new(2), source)
        .expect("program should compile");
    let mut runtime_a = Runtime::new(engine.clone(), program_a).expect("runtime should initialize");
    let mut runtime_b = Runtime::new(engine, program_b).expect("runtime should initialize");
    let reward_a = runtime_a
        .call(
            "make_reward",
            CallArgs::from_positional([OwnedValue::Scalar(vela_common::ScalarValue::I64(7))]),
            CallOptions::unbounded(),
        )
        .expect("first factory should run");
    let reward_b = runtime_b
        .call(
            "make_reward",
            CallArgs::from_positional([OwnedValue::Scalar(vela_common::ScalarValue::I64(11))]),
            CallOptions::unbounded(),
        )
        .expect("second factory should run");
    let score = runtime_a
        .method(&reward_a, "score")
        .expect("method should resolve");

    let error = runtime_b
        .bind_method(&reward_b, &score)
        .expect_err("cached method from another runtime should fail");

    assert_eq!(
        error.kind(),
        VmErrorKind::TypeMismatch {
            operation: "VelaMethod belongs to another Runtime"
        }
    );
}
