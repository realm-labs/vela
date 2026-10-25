use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use vela_def::FunctionId;
use vela_vm::budget::ExecutionBudgetKind;
use vela_vm::error::VmErrorKind;
use vela_vm::owned_value::OwnedValue;

use crate::engine::Engine;
use crate::native::{NativeFunctionDesc, TypeHint};
use crate::runtime::{CallArgs, CallControl, CallOptions, CallStatus, Runtime};

fn runtime() -> Runtime {
    let engine = Engine::builder().build().expect("engine");
    let program = engine
        .compile_source("fn main() { return 42; } fn long_loop() { let total = 0; for value in 0..1000000 { total += value; } return total; }")
        .expect("program");
    Runtime::new(engine, program).expect("runtime")
}

#[test]
fn synchronous_control_records_success_and_failure() {
    let mut runtime = runtime();
    let completed = CallControl::new();
    let value = runtime
        .call(
            "main",
            CallArgs::new(),
            CallOptions::new(1_000, 1 << 20, 32).with_control(completed.clone()),
        )
        .expect("call");
    assert_eq!(runtime.value_to_owned(&value), Ok(OwnedValue::i64(42)));
    assert_eq!(completed.snapshot().status, CallStatus::Completed);
    assert!(!completed.cancel());

    let failed = CallControl::new();
    runtime
        .call(
            "missing",
            CallArgs::new(),
            CallOptions::new(1_000, 1 << 20, 32).with_control(failed.clone()),
        )
        .expect_err("missing entry");
    assert_eq!(failed.snapshot().status, CallStatus::Failed);
    assert!(!failed.cancel());
}

#[test]
fn cancelled_and_expired_calls_reject_before_native_execution() {
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&calls);
    let engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("test::count", FunctionId::new(0xAC01)),
            move |_| {
                counter.fetch_add(1, Ordering::Relaxed);
                Ok(OwnedValue::Unit)
            },
        )
        .build()
        .expect("engine");
    let program = engine
        .compile_source("fn main() { test::count(); }")
        .expect("program");
    let mut runtime = Runtime::new(engine, program).expect("runtime");

    let cancelled = CallControl::new();
    assert!(cancelled.cancel());
    let error = runtime
        .call(
            "main",
            CallArgs::new(),
            CallOptions::new(1_000, 1 << 20, 32).with_control(cancelled.clone()),
        )
        .expect_err("pre-cancelled call");
    assert_eq!(error.kind(), VmErrorKind::CallCancelled);
    assert_eq!(cancelled.snapshot().status, CallStatus::Cancelled);

    let expired = CallControl::new();
    let error = runtime
        .call(
            "main",
            CallArgs::new(),
            CallOptions::new(1_000, 1 << 20, 32)
                .with_deadline(Instant::now())
                .with_control(expired.clone()),
        )
        .expect_err("expired call");
    assert_eq!(error.kind(), VmErrorKind::DeadlineExceeded);
    assert_eq!(expired.snapshot().status, CallStatus::DeadlineExceeded);
    assert_eq!(calls.load(Ordering::Relaxed), 0);
}

#[test]
fn cancellation_during_native_call_rejects_result_and_runtime_remains_reusable() {
    let control = CallControl::new();
    let cancel = control.clone();
    let engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("test::cancel", FunctionId::new(0xAC02)),
            move |_| {
                assert!(cancel.cancel());
                Ok(OwnedValue::Unit)
            },
        )
        .build()
        .expect("engine");
    let program = engine
        .compile_source("fn main() { test::cancel(); } fn reusable() { return 9; }")
        .expect("program");
    let mut runtime = Runtime::new(engine, program).expect("runtime");
    let error = runtime
        .call(
            "main",
            CallArgs::new(),
            CallOptions::new(1_000, 1 << 20, 32).with_control(control.clone()),
        )
        .expect_err("cancelled call");
    assert_eq!(error.kind(), VmErrorKind::CallCancelled);
    assert_eq!(control.snapshot().status, CallStatus::Cancelled);
    let value = runtime
        .call(
            "reusable",
            CallArgs::new(),
            CallOptions::new(1_000, 1 << 20, 32),
        )
        .expect("reuse");
    assert_eq!(runtime.value_to_owned(&value), Ok(OwnedValue::i64(9)));
}

#[test]
fn deadline_crossed_during_native_call_rejects_result() {
    let engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("test::delay", FunctionId::new(0xAC03))
                .returns(TypeHint::unit()),
            |_| {
                std::thread::sleep(Duration::from_millis(10));
                Ok(OwnedValue::Unit)
            },
        )
        .build()
        .expect("engine");
    let program = engine
        .compile_source("fn main() { test::delay(); }")
        .expect("program");
    let mut runtime = Runtime::new(engine, program).expect("runtime");
    let control = CallControl::new();
    let error = runtime
        .call(
            "main",
            CallArgs::new(),
            CallOptions::new(1_000, 1 << 20, 32)
                .with_timeout(Duration::from_millis(1))
                .with_control(control.clone()),
        )
        .expect_err("deadline elapsed");
    assert_eq!(error.kind(), VmErrorKind::DeadlineExceeded);
    assert_eq!(control.snapshot().status, CallStatus::DeadlineExceeded);
}

#[test]
fn native_result_materialization_respects_memory_limit_and_releases_runtime() {
    let engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("large_result", FunctionId::new(0xA535))
                .returns(TypeHint::string()),
            |_| Ok(OwnedValue::String("x".repeat(1024))),
        )
        .build()
        .expect("engine");
    let program = engine
        .compile_source(
            "fn main() -> String { return large_result(); } fn reusable() { return 9; }",
        )
        .expect("memory limit fixture");
    let mut runtime = Runtime::new(engine, program).expect("runtime");
    let error = runtime
        .call("main", CallArgs::new(), CallOptions::new(1_000, 64, 32))
        .expect_err("native result conversion must enforce the memory limit");
    assert!(matches!(
        error.kind(),
        VmErrorKind::BudgetExceeded {
            budget: ExecutionBudgetKind::MemoryBytes,
            limit: 64
        }
    ));
    let value = runtime
        .call(
            "reusable",
            CallArgs::new(),
            CallOptions::new(1_000, 1 << 20, 32),
        )
        .expect("runtime reusable after materialization failure");
    assert_eq!(runtime.value_to_owned(&value), Ok(OwnedValue::i64(9)));
}

#[test]
fn native_error_propagates_and_releases_runtime() {
    let engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("test::fail", FunctionId::new(0xA524)),
            |_| {
                Err(vela_vm::error::VmError::new(VmErrorKind::TypeMismatch {
                    operation: "synchronous native failure",
                }))
            },
        )
        .build()
        .expect("engine");
    let program = engine
        .compile_source("fn failing() { return test::fail(); } fn healthy() { return 42; }")
        .expect("native error fixture");
    let mut runtime = Runtime::new(engine, program).expect("runtime");
    let control = CallControl::new();
    let error = runtime
        .call(
            "failing",
            CallArgs::new(),
            CallOptions::new(1_000, 1 << 20, 32).with_control(control.clone()),
        )
        .expect_err("native error must propagate");
    assert_eq!(
        error.kind(),
        VmErrorKind::TypeMismatch {
            operation: "synchronous native failure"
        }
    );
    assert_eq!(control.snapshot().status, CallStatus::Failed);
    let value = runtime
        .call(
            "healthy",
            CallArgs::new(),
            CallOptions::new(1_000, 1 << 20, 32),
        )
        .expect("runtime reusable after native error");
    assert_eq!(runtime.value_to_owned(&value), Ok(OwnedValue::i64(42)));
}

#[test]
fn synchronous_range_loop_stops_at_finite_execution_budget() {
    let mut runtime = runtime();
    let error = runtime
        .call(
            "long_loop",
            CallArgs::new(),
            CallOptions::new(100, 1 << 20, 32),
        )
        .expect_err("bounded execution");
    assert!(matches!(
        error.kind(),
        VmErrorKind::BudgetExceeded {
            budget: ExecutionBudgetKind::ExecutionUnits,
            ..
        }
    ));
    let value = runtime
        .call(
            "main",
            CallArgs::new(),
            CallOptions::new(1_000, 1 << 20, 32),
        )
        .expect("reuse");
    assert_eq!(runtime.value_to_owned(&value), Ok(OwnedValue::i64(42)));
}
