use vela_host::access::HostAccess;
use vela_host::mock::MockStateAdapter;
use vela_vm::owned_value::OwnedValue;

use crate::engine::Engine;

use super::{
    CallArgs, CallOptions, OwnedImage, Runtime, RuntimeBuildError, RuntimeImage, RuntimeImpl,
    RuntimeInitializationLimits, RuntimeState, SharedRuntime,
};

#[test]
fn call_raw_executes_linked_program_image() {
    for options in [
        CallOptions::unbounded(),
        CallOptions::unbounded().with_managed_heap(false),
    ] {
        let mut runtime = linked_only_runtime();
        let mut adapter = MockStateAdapter::new();
        let mut access = HostAccess::new();

        let result = runtime.call_raw("main", &[], options, &mut adapter, &mut access);

        assert_eq!(
            result,
            Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(7)))
        );
    }
}

#[test]
fn runtime_program_rejects_unresolved_natives_before_image_construction() {
    let engine = Engine::builder().build().expect("engine should build");
    assert!(
        engine
            .compile_source("fn main() { return test::answer(); }")
            .is_err()
    );
}

#[test]
fn runtime_initializes_vm_state_once_and_shared_programs_remain_isolated() {
    let engine = Engine::builder().build().expect("engine should build");
    let source = r#"
state counter: i64 = 7;
fn increment() { counter += 1; return counter; }
"#;
    let program = engine.compile_source(source).expect("fixture compiles");
    let image = RuntimeImage::new_compiled(engine, program).into_shared();
    let mut first = SharedRuntime::from_shared_image(image.clone()).expect("first runtime");
    let mut second = SharedRuntime::from_shared_image(image).expect("second runtime");

    assert_eq!(
        first.state("main::counter"),
        Ok(Some(OwnedValue::from(7_i64)))
    );
    assert_eq!(
        second.state("main::counter"),
        Ok(Some(OwnedValue::from(7_i64)))
    );
    first
        .call("increment", CallArgs::new(), CallOptions::unbounded())
        .expect("increment call");
    assert_eq!(
        first.state("main::counter"),
        Ok(Some(OwnedValue::from(8_i64)))
    );
    assert_eq!(
        second.state("main::counter"),
        Ok(Some(OwnedValue::from(7_i64)))
    );
}

#[test]
fn runtime_state_initialization_enforces_bounded_call_depth() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source(
            r#"
fn recurse() -> i64 { return recurse(); }
state value: i64 = recurse();
"#,
        )
        .expect("recursive pure initializer compiles");
    let error = match Runtime::builder(engine, program)
        .expect("runtime image links")
        .with_initialization_limits(RuntimeInitializationLimits::new(100, 1024, 4))
        .build()
    {
        Ok(_) => panic!("initializer must exhaust its bounded call depth"),
        Err(error) => error,
    };

    assert!(matches!(
        error,
        RuntimeBuildError::Initializer { state, .. } if state == "main::value"
    ));
}

#[test]
fn runtime_state_initializers_construct_managed_aggregate_categories() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source(
            r#"
struct Snapshot {
    tuple: (i64, String),
    array: Array<i64>,
    map: Map<String, i64>,
    set: Set<i64>,
    maybe: Option<i64>,
    outcome: Result<i64, String>,
    bytes: Bytes,
}
state snapshot: Snapshot = Snapshot {
    tuple: (1, "tuple"),
    array: [2, 3],
    map: {"score": 4},
    set: set::from_array([5, 6]),
    maybe: Option::Some(7),
    outcome: Result::Ok(8),
    bytes: b"bytes",
};
"#,
        )
        .expect("aggregate initializer compiles");
    let mut runtime = Runtime::new(engine, program).expect("aggregate initializer runs");
    let value = runtime
        .state("main::snapshot")
        .expect("state read")
        .expect("snapshot cell");
    let OwnedValue::Record { type_name, fields } = value else {
        panic!("snapshot should be a managed record");
    };

    assert_eq!(type_name, "Snapshot");
    assert!(matches!(fields.get("tuple"), Some(OwnedValue::Tuple(_))));
    assert!(matches!(fields.get("array"), Some(OwnedValue::Array(_))));
    assert!(matches!(fields.get("map"), Some(OwnedValue::Map(_))));
    assert!(matches!(fields.get("set"), Some(OwnedValue::Set(_))));
    assert!(matches!(fields.get("maybe"), Some(OwnedValue::Enum { .. })));
    assert!(matches!(
        fields.get("outcome"),
        Some(OwnedValue::Enum { .. })
    ));
    assert!(matches!(fields.get("bytes"), Some(OwnedValue::Bytes(_))));
}

#[test]
fn runtime_state_initialization_enforces_execution_and_allocation_budgets() {
    for (case, source, limits) in [
        (
            "execution",
            "fn compute() -> i64 { let first = 1; let second = 2; return first + second; } state value: i64 = compute();",
            RuntimeInitializationLimits::new(0, 1024, 8),
        ),
        (
            "allocation",
            "state value: Array<i64> = [1, 2, 3, 4];",
            RuntimeInitializationLimits::new(100, 1, 8),
        ),
    ] {
        let engine = Engine::builder().build().expect("engine should build");
        let program = engine.compile_source(source).expect("initializer compiles");
        let error = match Runtime::builder(engine, program)
            .expect("runtime image links")
            .with_initialization_limits(limits)
            .build()
        {
            Ok(_) => panic!("{case} initializer must exhaust its configured budget"),
            Err(error) => error,
        };

        assert!(matches!(
            error,
            RuntimeBuildError::Initializer { state, .. } if state == "main::value"
        ));
    }
}

fn linked_only_runtime() -> RuntimeImpl<OwnedImage> {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source("fn main() { return 7; }")
        .expect("fixture compiles");
    let image = RuntimeImage::new_compiled(engine, program);
    let image = OwnedImage::from_image(image);
    let state = RuntimeState::for_image(&image);
    RuntimeImpl {
        image,
        hot_reload: None,
        state,
    }
}
