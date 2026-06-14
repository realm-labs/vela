use vela_bytecode::{CacheSiteId, CacheSiteKind};
use vela_common::SourceId;
use vela_vm::owned_value::OwnedValue;

use crate::engine::Engine;
use crate::runtime::{CallArgs, CallOptions, Runtime};

#[test]
fn accepted_hot_reload_clears_record_field_inline_caches() {
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_with_id(
            SourceId::new(1),
            r#"
struct Reward {
    count: i64,
    bonus: i64,
}

fn read_value() {
    let reward = Reward { count: 4, bonus: 7 };
    return reward.count;
}
"#,
        )
        .expect("initial record field source should compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let initial_site = record_field_read_site(&runtime, "read_value");

    let first = runtime
        .call("read_value", CallArgs::new(), CallOptions::unbounded())
        .expect("initial read_value should run");
    assert_eq!(
        runtime.value_to_owned(&first),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(4)))
    );
    assert!(
        runtime
            .state
            .inline_caches
            .record_field(initial_site)
            .is_some(),
        "initial record field read should populate its inline cache"
    );

    let update = runtime
        .compile_hot_reload_update_with_id(
            SourceId::new(2),
            r#"
struct Reward {
    count: i64,
    bonus: i64,
}

fn read_value() {
    let reward = Reward { count: 4, bonus: 7 };
    return reward.bonus;
}
"#,
        )
        .expect("runtime should compile record field hot reload update")
        .expect("record field target change should be accepted");
    let report = runtime
        .apply_hot_update(update)
        .expect("record field hot reload update should apply");
    assert!(report.accepted);

    let reloaded_site = record_field_read_site(&runtime, "read_value");
    assert_eq!(
        runtime.state.inline_caches.record_field(reloaded_site),
        None
    );

    let second = runtime
        .call("read_value", CallArgs::new(), CallOptions::unbounded())
        .expect("reloaded read_value should run");
    assert_eq!(
        runtime.value_to_owned(&second),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(7)))
    );
    assert!(
        runtime
            .state
            .inline_caches
            .record_field(reloaded_site)
            .is_some(),
        "reloaded record field read should repopulate its inline cache"
    );
}

#[test]
fn accepted_hot_reload_clears_dynamic_method_inline_caches() {
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_with_id(
            SourceId::new(1),
            r#"
fn call_dynamic(value) {
    return value.starts_with("q");
}
"#,
        )
        .expect("initial dynamic method source should compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let initial_site = method_call_site(&runtime, "call_dynamic");

    let first = runtime
        .call(
            "call_dynamic",
            CallArgs::from_positional([OwnedValue::String("quest".to_owned())]),
            CallOptions::unbounded(),
        )
        .expect("initial dynamic call should run");
    assert_eq!(runtime.value_to_owned(&first), Ok(OwnedValue::Bool(true)));
    assert!(
        runtime
            .state
            .inline_caches
            .dynamic_method_dispatch(initial_site)
            .is_some(),
        "initial dynamic method call should populate its inline cache"
    );

    let update = runtime
        .compile_hot_reload_update_with_id(
            SourceId::new(2),
            r#"
fn call_dynamic(value) {
    return value.ends_with("t");
}
"#,
        )
        .expect("runtime should compile dynamic method hot reload update")
        .expect("dynamic method body update should be accepted");
    let report = runtime
        .apply_hot_update(update)
        .expect("dynamic method hot reload update should apply");
    assert!(report.accepted);

    let reloaded_site = method_call_site(&runtime, "call_dynamic");
    assert_eq!(
        runtime
            .state
            .inline_caches
            .dynamic_method_dispatch(reloaded_site),
        None
    );

    let second = runtime
        .call(
            "call_dynamic",
            CallArgs::from_positional([OwnedValue::String("quest".to_owned())]),
            CallOptions::unbounded(),
        )
        .expect("reloaded dynamic call should run");
    assert_eq!(runtime.value_to_owned(&second), Ok(OwnedValue::Bool(true)));
    assert!(
        runtime
            .state
            .inline_caches
            .dynamic_method_dispatch(reloaded_site)
            .is_some(),
        "reloaded dynamic method call should repopulate its inline cache"
    );
}

fn record_field_read_site(runtime: &Runtime, function_name: &str) -> CacheSiteId {
    runtime
        .image
        .program_image()
        .function_by_name(function_name)
        .unwrap_or_else(|| panic!("{function_name} should exist"))
        .cache_sites
        .sites()
        .iter()
        .find(|site| site.kind == CacheSiteKind::RecordFieldRead)
        .unwrap_or_else(|| panic!("{function_name} should have a record field read site"))
        .id
}

fn method_call_site(runtime: &Runtime, function_name: &str) -> CacheSiteId {
    runtime
        .image
        .program_image()
        .function_by_name(function_name)
        .unwrap_or_else(|| panic!("{function_name} should exist"))
        .cache_sites
        .sites()
        .iter()
        .find(|site| site.kind == CacheSiteKind::MethodCall)
        .unwrap_or_else(|| panic!("{function_name} should have a method call site"))
        .id
}
