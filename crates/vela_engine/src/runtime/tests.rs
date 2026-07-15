use vela_host::access::HostAccess;
use vela_host::mock::MockStateAdapter;
use vela_vm::owned_value::OwnedValue;

use crate::engine::Engine;

use super::{
    CallArgs, CallOptions, OwnedImage, Runtime, RuntimeBuildError, RuntimeImage, RuntimeImpl,
    RuntimeInitializationLimits, RuntimeState,
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
    let first_program = engine.compile_source(source).expect("fixture compiles");
    let second_program = engine.compile_source(source).expect("fixture compiles");
    let mut first = Runtime::new(engine.clone(), first_program).expect("first runtime");
    let mut second = Runtime::new(engine, second_program).expect("second runtime");

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
