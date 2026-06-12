use vela_bytecode::{CacheSiteId, CacheSiteKind};
use vela_common::{ScalarValue, SourceId};
use vela_vm::owned_value::OwnedValue;

use crate::engine::Engine;
use crate::native::{NativeFunctionDesc, NativeFunctionId, TypeHint};
use crate::runtime::{CallArgs, CallOptions, Runtime};

#[test]
fn accepted_hot_reload_clears_native_call_inline_caches() {
    let native_id = NativeFunctionId::new(91);
    let engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::answer", native_id).returns(TypeHint::i64()),
            |_| Ok(OwnedValue::Scalar(ScalarValue::I64(41))),
        )
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial(
            SourceId::new(1),
            r#"
fn main() {
    return game::answer();
}
"#,
        )
        .expect("initial source should compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let initial_site = native_call_site(&runtime, "main");

    let first = runtime
        .call("main", CallArgs::new(), CallOptions::unbounded())
        .expect("initial main should run");
    assert_eq!(
        runtime.value_to_owned(&first),
        Ok(OwnedValue::Scalar(ScalarValue::I64(41)))
    );
    assert_eq!(
        runtime
            .state
            .inline_caches
            .native_call(initial_site)
            .expect("initial native call should populate inline cache")
            .native_id(),
        native_id
    );

    let update = runtime
        .compile_hot_reload_update(
            SourceId::new(2),
            r#"
fn main() {
    return game::answer() + 1;
}
"#,
        )
        .expect("runtime should compile native hot reload update")
        .expect("native call body update should be accepted");
    let report = runtime
        .apply_hot_update(update)
        .expect("native hot reload update should apply");
    assert!(report.accepted);

    let reloaded_site = native_call_site(&runtime, "main");
    assert!(
        runtime
            .state
            .inline_caches
            .native_call(reloaded_site)
            .is_none()
    );

    let second = runtime
        .call("main", CallArgs::new(), CallOptions::unbounded())
        .expect("reloaded main should run");
    assert_eq!(
        runtime.value_to_owned(&second),
        Ok(OwnedValue::Scalar(ScalarValue::I64(42)))
    );
    assert_eq!(
        runtime
            .state
            .inline_caches
            .native_call(reloaded_site)
            .expect("reloaded native call should repopulate inline cache")
            .native_id(),
        native_id
    );
}

fn native_call_site(runtime: &Runtime, function_name: &str) -> CacheSiteId {
    runtime
        .image
        .program_image()
        .function_by_name(function_name)
        .unwrap_or_else(|| panic!("{function_name} should exist"))
        .cache_sites
        .sites()
        .iter()
        .find(|site| site.kind == CacheSiteKind::NativeCall)
        .unwrap_or_else(|| panic!("{function_name} should have a native call site"))
        .id
}
