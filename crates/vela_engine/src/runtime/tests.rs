use vela_host::access::HostAccess;
use vela_host::mock::MockStateAdapter;
use vela_vm::owned_value::OwnedValue;

use crate::engine::Engine;

use super::{CallOptions, OwnedImage, RuntimeImage, RuntimeImpl, RuntimeState};

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
