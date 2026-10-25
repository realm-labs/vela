use crate::engine::Engine;
use crate::native::{FunctionAccess, NativeFunctionDesc, TypeHint};
use crate::runtime::{CallArgs, CallOptions, Runtime};
use vela_common::ScalarValue;
use vela_def::FunctionId;
use vela_vm::owned_value::OwnedValue;

#[test]
fn context_native_reenters_same_session_across_nested_native_boundary() {
    let engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("test::nested", FunctionId::new(0xA530))
                .param("value", TypeHint::i64())
                .returns(TypeHint::i64())
                .access(FunctionAccess::public()),
            |args| {
                let value = match args.first() {
                    Some(OwnedValue::Scalar(ScalarValue::I64(value))) => *value,
                    _ => 0,
                };
                Ok(OwnedValue::i64(value + 1))
            },
        )
        .register_context_host_native_fn(
            NativeFunctionDesc::new("test::outer", FunctionId::new(0xA531))
                .returns(TypeHint::i64())
                .access(FunctionAccess::public()),
            |_args, context| {
                let receiver = context.call("counter", CallArgs::new())?;
                let target = context.bind_method(&receiver, "add")?;
                let _ = context.call(target, CallArgs::new().with(2_i64))?;
                let _ = context.call("child", CallArgs::new().with(41_i64))?;
                Ok(OwnedValue::i64(7))
            },
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            r#"
struct Counter { value: i64 }

fn counter() { return Counter { value: 40 }; }

impl Counter {
    fn add(self, amount: i64) { return self.value + amount; }
}

fn child(value: i64) {
    return test::nested(value);
}

fn main() {
    return test::outer();
}
"#,
        )
        .expect("reentry fixture should compile");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");

    let value = runtime
        .call("main", CallArgs::new(), CallOptions::unbounded())
        .expect("nested reentry should complete");

    assert_eq!(runtime.value_to_owned(&value), Ok(OwnedValue::i64(7)));
}

#[test]
fn parent_heap_roots_survive_gc_during_nested_reentry() {
    let engine = Engine::builder()
        .register_context_host_native_fn(
            NativeFunctionDesc::new("test::nested_gc", FunctionId::new(0xA533))
                .returns(TypeHint::unit())
                .access(FunctionAccess::public()),
            |_args, context| {
                let _ = context.call("collect_garbage", CallArgs::new())?;
                Ok(OwnedValue::Unit)
            },
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            r#"
struct Keep { text: String }

fn collect_garbage() {
    let garbage = ["one", "two", "three", "four"];
    return garbage.len();
}

fn main() {
    let keep = Keep { text: "alive" };
    test::nested_gc();
    return keep.text;
}
"#,
        )
        .expect("nested GC fixture should compile");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");
    runtime
        .state
        .vm_states
        .heap
        .set_gc_config(vela_vm::heap::GcConfig {
            max_pause_micros: 500,
            heap_growth_factor: 1.0,
        });
    let value = runtime
        .call("main", CallArgs::new(), CallOptions::unbounded())
        .expect("parent roots should survive nested GC");

    assert_eq!(
        runtime.value_to_owned(&value),
        Ok(OwnedValue::String("alive".into()))
    );
}

#[test]
fn reentry_returned_value_survives_nested_incremental_gc() {
    let engine = Engine::builder()
        .register_context_host_native_fn(
            NativeFunctionDesc::new("test::hold_reentry_value", FunctionId::new(0xA534))
                .returns(TypeHint::i64())
                .access(FunctionAccess::public()),
            |_args, context| {
                let held = context.call("make_held", CallArgs::new())?;
                let held_clone = held.clone();
                drop(held);
                let _ = context.call("collect_garbage", CallArgs::new())?;
                let method = context.bind_method(&held_clone, "read")?;
                let _ = context.call(method, CallArgs::new())?;
                let _ =
                    context.call("consume_held", CallArgs::new().with_vela_value(held_clone))?;
                Ok(OwnedValue::i64(7))
            },
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            r#"
struct Held { text: String }

fn make_held() { return Held { text: "alive" }; }
fn collect_garbage() {
    let garbage = ["one", "two", "three", "four", "five"];
    return garbage.len();
}
fn consume_held(value: Held) { return value.text.len(); }

impl Held {
    fn read(self) { return self.text; }
}

fn main() { return test::hold_reentry_value(); }
"#,
        )
        .expect("dynamic reentry root fixture should compile");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");
    runtime
        .state
        .vm_states
        .heap
        .set_gc_config(vela_vm::heap::GcConfig {
            max_pause_micros: 1,
            heap_growth_factor: 1.0,
        });

    let value = runtime
        .call("main", CallArgs::new(), CallOptions::unbounded())
        .expect("reentry-returned value should remain rooted through nested GC");

    assert_eq!(runtime.value_to_owned(&value), Ok(OwnedValue::i64(7)));
}

#[test]
fn reentry_call_depth_error_unwinds_child_and_runtime_remains_reusable() {
    let engine = Engine::builder()
        .register_context_host_native_fn(
            NativeFunctionDesc::new("test::enter_deep", FunctionId::new(0xA532))
                .returns(TypeHint::unit())
                .access(FunctionAccess::public()),
            |_args, context| {
                let _ = context.call("deep", CallArgs::new().with(100_i64))?;
                Ok(OwnedValue::Unit)
            },
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            r#"
fn deep(remaining: i64) {
    if remaining == 0 { return 0; }
    return deep(remaining - 1);
}

fn main() { return test::enter_deep(); }
fn reusable() { return 9; }
"#,
        )
        .expect("depth fixture should compile");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");

    let error = runtime
        .call(
            "main",
            CallArgs::new(),
            CallOptions::new(u64::MAX, usize::MAX, 4),
        )
        .expect_err("nested reentry should inherit the outer call-depth budget");
    assert!(matches!(
        error.kind(),
        vela_vm::error::VmErrorKind::BudgetExceeded {
            budget: vela_vm::budget::ExecutionBudgetKind::CallDepth,
            ..
        }
    ));

    let value = runtime
        .call("reusable", CallArgs::new(), CallOptions::unbounded())
        .expect("reentry failure should leave Runtime reusable");
    assert_eq!(runtime.value_to_owned(&value), Ok(OwnedValue::i64(9)));
}
