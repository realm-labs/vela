use std::sync::Arc;

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
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");
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
            .image
            .execution_data()
            .inline_caches()
            .record_field(initial_site)
            .is_some(),
        "initial record field read should populate its inline cache"
    );

    let update = runtime
        .compile_reload_with_id(
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
        .apply_reload_update_for_test(update)
        .expect("record field hot reload update should apply");
    assert!(report.accepted);

    let reloaded_site = record_field_read_site(&runtime, "read_value");
    assert_eq!(
        runtime
            .image
            .execution_data()
            .inline_caches()
            .record_field(reloaded_site),
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
            .image
            .execution_data()
            .inline_caches()
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
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");
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
            .image
            .execution_data()
            .inline_caches()
            .dynamic_method_dispatch(initial_site)
            .is_some(),
        "initial dynamic method call should populate its inline cache"
    );

    let update = runtime
        .compile_reload_with_id(
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
        .apply_reload_update_for_test(update)
        .expect("dynamic method hot reload update should apply");
    assert!(report.accepted);

    let reloaded_site = method_call_site(&runtime, "call_dynamic");
    assert_eq!(
        runtime
            .image
            .execution_data()
            .inline_caches()
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
            .image
            .execution_data()
            .inline_caches()
            .dynamic_method_dispatch(reloaded_site)
            .is_some(),
        "reloaded dynamic method call should repopulate its inline cache"
    );
}

#[test]
fn retained_old_closure_uses_exact_cache_profile_and_reclaims_execution_data() {
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_with_id(
            SourceId::new(1),
            r#"
fn make() { return |value: String| value.starts_with("old"); }
fn invoke(callback, value: String) -> bool { return callback(value); }
"#,
        )
        .expect("initial closure source should compile");
    let mut runtime = Runtime::builder_from_hot_reload_version(engine, initial)
        .with_bytecode_profiling()
        .build()
        .expect("runtime should initialize");
    let old_data = Arc::clone(runtime.image.execution_data());
    let old_lifetime = Arc::downgrade(&old_data);
    let old_generation = runtime.image.linked_program().generation();
    let old_site = first_method_call_site(&runtime);
    let old_closure = runtime
        .call("make", CallArgs::new(), CallOptions::unbounded())
        .expect("old closure should be retained");
    assert!(old_data.inline_caches().method_dispatch(old_site).is_none());

    let update = runtime
        .compile_reload_with_id(
            SourceId::new(2),
            r#"
fn make() { return |value: String| value.ends_with("new"); }
fn invoke(callback, value: String) -> bool { return callback(value); }
"#,
        )
        .expect("runtime should compile closure update")
        .expect("closure update should be compatible");
    runtime
        .apply_reload_update_for_test(update)
        .expect("closure update should apply");
    let new_site = first_method_call_site(&runtime);
    assert!(!Arc::ptr_eq(&old_data, runtime.image.execution_data()));
    assert_ne!(old_generation, runtime.image.linked_program().generation());
    assert_eq!(runtime.retained_generation_count(), 2);
    assert!(
        runtime
            .image
            .execution_data()
            .inline_caches()
            .method_dispatch(new_site)
            .is_none()
    );
    let old_profile_before = profile_total(&old_data, old_generation);

    let mut args = CallArgs::from_values([old_closure.clone()]);
    args.push("old-value".to_owned());
    let result = runtime
        .call("invoke", args, CallOptions::unbounded())
        .expect("retained closure should execute its creation generation");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::Bool(true)));
    assert!(
        old_data.inline_caches().method_dispatch(old_site).is_some(),
        "old closure must populate its original generation cache"
    );
    assert!(
        runtime
            .image
            .execution_data()
            .inline_caches()
            .method_dispatch(new_site)
            .is_none(),
        "old closure must not populate the active generation cache"
    );
    assert!(profile_total(&old_data, old_generation) > old_profile_before);

    drop(result);
    drop(old_closure);
    drop(old_data);
    assert!(old_lifetime.upgrade().is_some());
    assert_eq!(
        runtime
            .activate_reload()
            .expect("ordinary safe point succeeds"),
        None
    );
    assert_eq!(runtime.retained_generation_count(), 1);
    assert!(
        old_lifetime.upgrade().is_none(),
        "one ordinary safe point must reclaim dead old execution data"
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

fn first_method_call_site(runtime: &Runtime) -> CacheSiteId {
    runtime
        .image
        .linked_artifact()
        .cache_layout()
        .iter()
        .find(|site| site.kind == CacheSiteKind::MethodCall)
        .expect("fixture should have a method call site")
        .id
}

fn profile_total(
    data: &crate::runtime::execution_data::GenerationExecutionData,
    generation: vela_bytecode::ExecutableGenerationId,
) -> u64 {
    data.bytecode_profile()
        .expect("profiling should be enabled")
        .snapshot(generation)
        .functions()
        .iter()
        .flat_map(|function| function.instruction_hits())
        .copied()
        .sum()
}
