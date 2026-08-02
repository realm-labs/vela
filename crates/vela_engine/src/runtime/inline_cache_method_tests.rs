use vela_bytecode::{
    CacheSiteId, DebugNameId, MethodDispatchHandle, ScriptFunctionHandle,
    linked::InstructionKind as LinkedInstructionKind, linked::LinkedMethodDispatchKind,
};
use vela_common::{ScalarValue, SourceId};
use vela_def::MethodId;
use vela_host::object::ScriptHostObject;
use vela_vm::{
    CallbackMethodInlineCacheTarget, DynamicMethodInlineCacheTarget, DynamicReceiverGuard,
    MethodInlineCacheEntry, MethodInlineCacheTarget, StandardMethodInlineCacheTarget,
    StandardMethodReceiver, owned_value::OwnedValue,
};

use crate::engine::Engine;
use crate::permission::Capability;
use crate::reload::{EngineHotReloadSourceError, EngineHotReloadSourceErrorKind};
use crate::runtime::{CallArgs, CallOptions, Runtime};

#[test]
fn linked_script_method_uses_immutable_dispatch_without_mutable_cache() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
struct Counter { amount: i64 }

impl Counter {
    fn add(self, bonus) -> i64 {
        return self.amount + bonus;
    }
}

fn read_bonus() {
    return Counter { amount: 3 }.add(4);
}
"#,
        )
        .expect("program should compile");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");
    let call = method_call_site(&runtime, "read_bonus");

    assert_eq!(
        runtime
            .image
            .execution_data()
            .inline_caches()
            .method_dispatch(call.cache_site),
        None
    );

    let result = runtime
        .call("read_bonus", CallArgs::new(), CallOptions::unbounded())
        .expect("read_bonus should run");
    assert_eq!(
        runtime.value_to_owned(&result),
        Ok(OwnedValue::Scalar(ScalarValue::I64(7)))
    );

    assert_eq!(
        runtime
            .image
            .execution_data()
            .inline_caches()
            .method_dispatch(call.cache_site),
        None
    );
}

#[test]
fn linked_script_method_ignores_mutable_entry_with_wrong_dispatch() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
struct Counter { amount: i64 }

impl Counter {
    fn add(self, bonus) -> i64 {
        return self.amount + bonus;
    }
}

fn read_bonus() {
    return Counter { amount: 3 }.add(4);
}
"#,
        )
        .expect("program should compile");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");
    let call = method_call_site(&runtime, "read_bonus");
    runtime
        .image
        .execution_data()
        .inline_caches()
        .set_method_dispatch(
            call.cache_site,
            MethodInlineCacheEntry {
                dispatch: MethodDispatchHandle::new(call.dispatch.index() + 1),
                debug_name: call.debug_name,
                target: MethodInlineCacheTarget::Value {
                    method_id: MethodId::new(0),
                    standard_method: None,
                },
            },
        );

    let result = runtime
        .call("read_bonus", CallArgs::new(), CallOptions::unbounded())
        .expect("read_bonus should miss stale method cache and run");
    assert_eq!(
        runtime.value_to_owned(&result),
        Ok(OwnedValue::Scalar(ScalarValue::I64(7)))
    );

    let entry = runtime
        .image
        .execution_data()
        .inline_caches()
        .method_dispatch(call.cache_site)
        .expect("injected entry remains outside immutable script dispatch");
    assert_ne!(entry.dispatch, call.dispatch);
}

#[test]
fn linked_script_method_ignores_mutable_entry_with_wrong_target() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
struct Counter { amount: i64 }

impl Counter {
    fn add(self, bonus) -> i64 {
        return self.amount + bonus;
    }
}

fn read_bonus() {
    return Counter { amount: 3 }.add(4);
}
"#,
        )
        .expect("program should compile");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");
    let call = method_call_site(&runtime, "read_bonus");
    let (method_id, function) = script_method_target(&runtime, call.dispatch);
    runtime
        .image
        .execution_data()
        .inline_caches()
        .set_method_dispatch(
            call.cache_site,
            MethodInlineCacheEntry {
                dispatch: call.dispatch,
                debug_name: call.debug_name,
                target: MethodInlineCacheTarget::Script {
                    method_id,
                    function: ScriptFunctionHandle::new(function.index() + 1),
                },
            },
        );

    let result = runtime
        .call("read_bonus", CallArgs::new(), CallOptions::unbounded())
        .expect("read_bonus should miss stale script target and run");
    assert_eq!(
        runtime.value_to_owned(&result),
        Ok(OwnedValue::Scalar(ScalarValue::I64(7)))
    );

    let entry = runtime
        .image
        .execution_data()
        .inline_caches()
        .method_dispatch(call.cache_site)
        .expect("injected entry remains outside immutable script dispatch");
    assert_eq!(entry.dispatch, call.dispatch);
    assert_eq!(entry.debug_name, call.debug_name);
    assert_ne!(
        entry.target,
        MethodInlineCacheTarget::Script {
            method_id,
            function
        }
    );
}

#[test]
fn accepted_hot_reload_publishes_new_immutable_script_method_dispatch() {
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_with_id(
            SourceId::new(1),
            r#"
struct Counter { amount: i64 }

impl Counter {
    fn add(self, bonus) -> i64 {
        return self.amount + bonus;
    }
}

fn read_bonus() {
    return Counter { amount: 3 }.add(4);
}
"#,
        )
        .expect("initial source should compile");
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");
    let initial_call = method_call_site(&runtime, "read_bonus");

    let first = runtime
        .call("read_bonus", CallArgs::new(), CallOptions::unbounded())
        .expect("initial read_bonus should run");
    assert_eq!(
        runtime.value_to_owned(&first),
        Ok(OwnedValue::Scalar(ScalarValue::I64(7)))
    );
    assert_eq!(
        runtime
            .image
            .execution_data()
            .inline_caches()
            .method_dispatch(initial_call.cache_site),
        None
    );

    let update = runtime
        .compile_reload_with_id(
            SourceId::new(2),
            r#"
struct Counter { amount: i64 }

impl Counter {
    fn add(self, bonus) -> i64 {
        return self.amount + bonus;
    }
}

fn read_bonus() {
    return Counter { amount: 5 }.add(4);
}
"#,
        )
        .expect("runtime should compile method hot reload update")
        .expect("method body update should be accepted");
    let report = runtime
        .apply_reload_update_for_test(update)
        .expect("method hot reload update should apply");
    assert!(report.accepted);

    let reloaded_call = method_call_site(&runtime, "read_bonus");
    assert_eq!(
        runtime
            .image
            .execution_data()
            .inline_caches()
            .method_dispatch(reloaded_call.cache_site),
        None
    );

    let second = runtime
        .call("read_bonus", CallArgs::new(), CallOptions::unbounded())
        .expect("reloaded read_bonus should run");
    assert_eq!(
        runtime.value_to_owned(&second),
        Ok(OwnedValue::Scalar(ScalarValue::I64(9)))
    );
    assert_eq!(
        runtime
            .image
            .execution_data()
            .inline_caches()
            .method_dispatch(reloaded_call.cache_site),
        None
    );
}

#[test]
fn rejected_hot_reload_preserves_immutable_script_method_dispatch_generation() {
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_with_id(
            SourceId::new(1),
            r#"
struct Counter { amount: i64 }

impl Counter {
    fn add(self, bonus) -> i64 {
        return self.amount + bonus;
    }
}

pub fn read_bonus() -> i64 {
    return Counter { amount: 3 }.add(4);
}
"#,
        )
        .expect("initial source should compile");
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");
    let initial_call = method_call_site(&runtime, "read_bonus");

    let first = runtime
        .call("read_bonus", CallArgs::new(), CallOptions::unbounded())
        .expect("initial read_bonus should run");
    assert_eq!(
        runtime.value_to_owned(&first),
        Ok(OwnedValue::Scalar(ScalarValue::I64(7)))
    );
    let initial_execution_data = std::sync::Arc::clone(runtime.image.execution_data());
    assert_eq!(
        runtime
            .image
            .execution_data()
            .inline_caches()
            .method_dispatch(initial_call.cache_site),
        None
    );

    let update = runtime
        .compile_reload_with_id(
            SourceId::new(2),
            r#"
struct Counter { amount: i64 }

impl Counter {
    fn add(self, bonus) -> i64 {
        return self.amount + bonus;
    }
}

pub fn read_bonus() -> f64 {
    return 9.0;
}
"#,
        )
        .expect("runtime should compile rejected method hot reload update");
    let update = match update {
        Err(EngineHotReloadSourceError {
            kind: EngineHotReloadSourceErrorKind::HotReload(error),
        }) => Err(error),
        Ok(_) => panic!("method ABI update should be rejected"),
        Err(error) => panic!("source compilation should succeed: {error}"),
    };
    let report = runtime
        .apply_reload_result_for_test(update)
        .expect("rejected method hot reload update should report");
    assert!(!report.accepted);
    assert_eq!(report.to_version, None);

    let active_call = method_call_site(&runtime, "read_bonus");
    assert_eq!(active_call.cache_site, initial_call.cache_site);
    assert!(std::sync::Arc::ptr_eq(
        &initial_execution_data,
        runtime.image.execution_data()
    ));

    let second = runtime
        .call("read_bonus", CallArgs::new(), CallOptions::unbounded())
        .expect("active read_bonus should keep running after rejected reload");
    assert_eq!(
        runtime.value_to_owned(&second),
        Ok(OwnedValue::Scalar(ScalarValue::I64(7)))
    );
    assert_eq!(
        runtime
            .image
            .execution_data()
            .inline_caches()
            .method_dispatch(active_call.cache_site),
        None
    );
}

#[test]
fn accepted_hot_reload_clears_callback_value_method_inline_caches() {
    let engine = Engine::builder()
        .with_standard_natives()
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_with_id(
            SourceId::new(1),
            r#"
fn read_match() {
    return [1, 2, 3].any(|value| value == 2);
}
"#,
        )
        .expect("initial source should compile");
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");
    let initial_call = method_call_site(&runtime, "read_match");

    let first = runtime
        .call("read_match", CallArgs::new(), CallOptions::unbounded())
        .expect("initial read_match should run");
    assert_eq!(runtime.value_to_owned(&first), Ok(OwnedValue::Bool(true)));
    assert_callback_value_method_cache(&runtime, initial_call.cache_site);

    let update = runtime
        .compile_reload_with_id(
            SourceId::new(2),
            r#"
fn read_match() {
    return [1, 2, 3].any(|value| value == 4);
}
"#,
        )
        .expect("runtime should compile callback hot reload update")
        .expect("callback body update should be accepted");
    let report = runtime
        .apply_reload_update_for_test(update)
        .expect("callback hot reload update should apply");
    assert!(report.accepted);

    let reloaded_call = method_call_site(&runtime, "read_match");
    assert_eq!(
        runtime
            .image
            .execution_data()
            .inline_caches()
            .method_dispatch(reloaded_call.cache_site),
        None
    );

    let second = runtime
        .call("read_match", CallArgs::new(), CallOptions::unbounded())
        .expect("reloaded read_match should run");
    assert_eq!(runtime.value_to_owned(&second), Ok(OwnedValue::Bool(false)));
    assert_callback_value_method_cache(&runtime, reloaded_call.cache_site);
}

#[test]
fn host_collection_element_method_uses_host_type_guarded_dynamic_cache() {
    let engine = Engine::builder()
        .capability(Capability::HostRead)
        .install_generated_type::<Vec<Vec<i64>>>()
        .build()
        .expect("nested host collection binding should seal");
    let program = engine
        .compile_source(
            r#"
fn child_len(value) {
    return value.len();
}

fn total(values) {
    let cursor = values.values();
    let first = cursor.next()?;
    let first_length = child_len(first);
    host::release(first);
    let second = cursor.next()?;
    let second_length = child_len(second);
    host::release(second);
    host::release(cursor);
    return first_length + second_length;
}
"#,
        )
        .expect("nested host collection cache fixture should compile");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");
    let site = dynamic_method_call_site_by_name(&runtime, "child_len", "len");
    let values = vec![vec![1_i64, 2], vec![3, 5, 8]];
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);

    let result = runtime
        .call("total", args, CallOptions::unbounded())
        .expect("host-backed child collection methods should run");
    assert_eq!(
        runtime.value_to_owned(&result),
        Ok(OwnedValue::Scalar(ScalarValue::I64(5)))
    );

    let entry = runtime
        .image
        .execution_data()
        .inline_caches()
        .dynamic_method_dispatch(site)
        .expect("host-backed standard method should populate the dynamic cache");
    assert!(matches!(
        entry.receiver_guard,
        DynamicReceiverGuard::HostType {
            type_id,
            lease_kind: vela_host::lease::HostLeaseKind::Shared,
            ..
        } if type_id == Vec::<i64>::new().host_type_id()
    ));
    assert!(matches!(
        entry.target,
        DynamicMethodInlineCacheTarget::StandardValue {
            method_id,
            ..
        } if method_id == vela_stdlib::std_method_id("Array", "len")
            .expect("Array::len method id")
    ));
}

#[test]
fn typed_host_collection_element_method_links_to_dense_method_id() {
    let engine = Engine::builder()
        .capability(Capability::HostRead)
        .install_generated_type::<Vec<Vec<i64>>>()
        .build()
        .expect("nested host collection binding should seal");
    let program = engine
        .compile_source(
            r#"
fn child_len(value: ArrayView<i64>) -> i64 {
    return value.len();
}

fn total(values: ArrayView<ArrayView<i64>>) -> i64 {
    let cursor = values.values();
    let first = cursor.next()?;
    let first_length = child_len(first);
    host::release(first);
    let second = cursor.next()?;
    let second_length = child_len(second);
    host::release(second);
    host::release(cursor);
    return first_length + second_length;
}
"#,
        )
        .expect("typed nested host collection fixture should compile");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");
    let call = method_call_site_matching(&runtime, "child_len", |program, name| {
        program.debug_name(name) == "len"
    });
    let dispatch = runtime
        .image
        .linked_program()
        .method_dispatch(call.dispatch)
        .expect("linked Array::len dispatch should exist");
    assert!(matches!(
        dispatch.kind,
        LinkedMethodDispatchKind::Value { method_id }
            if method_id == vela_stdlib::std_method_id("Array", "len")
                .expect("Array::len method id")
    ));

    let values = vec![vec![1_i64, 2], vec![3, 5, 8]];
    let mut args = CallArgs::new();
    args.push_collection_ref("values", &values);
    let result = runtime
        .call("total", args, CallOptions::unbounded())
        .expect("typed host-backed child collection methods should run");
    assert_eq!(
        runtime.value_to_owned(&result),
        Ok(OwnedValue::Scalar(ScalarValue::I64(5)))
    );
}

#[test]
fn accepted_hot_reload_clears_iterator_adapter_inline_caches() {
    let engine = Engine::builder()
        .with_standard_natives()
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_with_id(
            SourceId::new(1),
            r#"
fn read_total() {
    return [1, 2, 3, 4]
        .iter()
        .take(2)
        .collect_array()
        .sum();
}
"#,
        )
        .expect("initial source should compile");
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");
    let initial_site = dynamic_method_call_site_by_name(&runtime, "read_total", "take");

    let first = runtime
        .call("read_total", CallArgs::new(), CallOptions::unbounded())
        .expect("initial read_total should run");
    assert_eq!(
        runtime.value_to_owned(&first),
        Ok(OwnedValue::Scalar(ScalarValue::I64(3)))
    );
    assert_dynamic_iterator_value_method_cache(
        &runtime,
        initial_site,
        StandardMethodInlineCacheTarget::Take,
    );

    let update = runtime
        .compile_reload_with_id(
            SourceId::new(2),
            r#"
fn read_total() {
    return [1, 2, 3, 4]
        .iter()
        .take(3)
        .collect_array()
        .sum();
}
"#,
        )
        .expect("runtime should compile iterator adapter hot reload update")
        .expect("iterator adapter body update should be accepted");
    let report = runtime
        .apply_reload_update_for_test(update)
        .expect("iterator adapter hot reload update should apply");
    assert!(report.accepted);

    let reloaded_site = dynamic_method_call_site_by_name(&runtime, "read_total", "take");
    assert_eq!(
        runtime
            .image
            .execution_data()
            .inline_caches()
            .dynamic_method_dispatch(reloaded_site),
        None
    );

    let second = runtime
        .call("read_total", CallArgs::new(), CallOptions::unbounded())
        .expect("reloaded read_total should run");
    assert_eq!(
        runtime.value_to_owned(&second),
        Ok(OwnedValue::Scalar(ScalarValue::I64(6)))
    );
    assert_dynamic_iterator_value_method_cache(
        &runtime,
        reloaded_site,
        StandardMethodInlineCacheTarget::Take,
    );
}

#[test]
fn failed_dynamic_cache_population_leaves_slot_available() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            "fn call(value) { return value.starts_with(\"q\"); }",
        )
        .expect("dynamic method fixture should compile");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");
    let site = dynamic_method_call_site_by_name(&runtime, "call", "starts_with");

    runtime
        .call(
            "call",
            CallArgs::from_positional([OwnedValue::i64(1)]),
            CallOptions::unbounded(),
        )
        .expect_err("i64 receiver should fail dynamic method resolution");
    assert!(
        runtime
            .image
            .execution_data()
            .inline_caches()
            .dynamic_method_dispatch(site)
            .is_none(),
        "failed resolution must not publish a partial cache entry"
    );

    let value = runtime
        .call(
            "call",
            CallArgs::from_positional([OwnedValue::String("quest".to_owned())]),
            CallOptions::unbounded(),
        )
        .expect("same cache slot should remain usable after failure");
    assert_eq!(runtime.value_to_owned(&value), Ok(OwnedValue::Bool(true)));
    assert!(
        runtime
            .image
            .execution_data()
            .inline_caches()
            .dynamic_method_dispatch(site)
            .is_some()
    );
}

#[derive(Clone, Copy)]
struct LinkedMethodCallSite {
    cache_site: CacheSiteId,
    dispatch: MethodDispatchHandle,
    debug_name: DebugNameId,
}

fn script_method_target(
    runtime: &Runtime,
    dispatch: MethodDispatchHandle,
) -> (MethodId, ScriptFunctionHandle) {
    let program = runtime.image.linked_program();
    let dispatch = program
        .method_dispatch(dispatch)
        .expect("linked method dispatch should exist");
    let LinkedMethodDispatchKind::Script {
        method_id,
        function,
    } = &dispatch.kind
    else {
        panic!("linked method dispatch should target a script method");
    };
    (*method_id, *function)
}

fn assert_callback_value_method_cache(runtime: &Runtime, site: CacheSiteId) {
    assert_callback_value_method_cache_target(
        runtime,
        site,
        StandardMethodReceiver::Array,
        CallbackMethodInlineCacheTarget::Any,
    );
}

fn assert_callback_value_method_cache_target(
    runtime: &Runtime,
    site: CacheSiteId,
    expected_receiver: StandardMethodReceiver,
    expected_target: CallbackMethodInlineCacheTarget,
) {
    let entry = runtime
        .image
        .execution_data()
        .inline_caches()
        .method_dispatch(site)
        .expect("callback value method call should populate inline cache");
    let MethodInlineCacheTarget::CallbackValue {
        callback_method, ..
    } = entry.target
    else {
        panic!("method cache should store a callback value target");
    };
    assert_eq!(callback_method.receiver, expected_receiver);
    assert_eq!(callback_method.target, expected_target);
}

fn assert_dynamic_iterator_value_method_cache(
    runtime: &Runtime,
    site: CacheSiteId,
    expected_target: StandardMethodInlineCacheTarget,
) {
    let entry = runtime
        .image
        .execution_data()
        .inline_caches()
        .dynamic_method_dispatch(site)
        .expect("dynamic iterator method call should populate inline cache");
    assert!(matches!(
        entry.receiver_guard,
        DynamicReceiverGuard::StdValue {
            receiver: StandardMethodReceiver::Iterator,
        }
    ));
    let DynamicMethodInlineCacheTarget::StandardValue {
        standard_method: Some(standard_method),
        ..
    } = entry.target
    else {
        panic!("dynamic iterator method cache should store a standard value target");
    };
    assert_eq!(standard_method.receiver, StandardMethodReceiver::Iterator);
    assert_eq!(standard_method.target, expected_target);
}

fn method_call_site(runtime: &Runtime, function_name: &str) -> LinkedMethodCallSite {
    method_call_site_matching(runtime, function_name, |_, _| true)
}

fn dynamic_method_call_site_by_name(
    runtime: &Runtime,
    function_name: &str,
    method_name: &str,
) -> CacheSiteId {
    let program = runtime.image.linked_program();
    let function = program
        .entry_point_by_name(function_name)
        .and_then(|handle| program.function(handle))
        .unwrap_or_else(|| panic!("{function_name} should exist"));
    function
        .instructions
        .iter()
        .find_map(|instruction| {
            if let LinkedInstructionKind::CallDynamicMethod {
                method_name: debug_name,
                cache_site: Some(cache_site),
                ..
            } = &instruction.kind
                && program.debug_name(*debug_name) == method_name
            {
                Some(*cache_site)
            } else {
                None
            }
        })
        .unwrap_or_else(|| {
            panic!("{function_name} should have a dynamic method call site named {method_name}")
        })
}

fn method_call_site_matching(
    runtime: &Runtime,
    function_name: &str,
    matches_debug_name: impl Fn(&vela_bytecode::LinkedProgram, DebugNameId) -> bool,
) -> LinkedMethodCallSite {
    let program = runtime.image.linked_program();
    let function = program
        .entry_point_by_name(function_name)
        .and_then(|handle| program.function(handle))
        .unwrap_or_else(|| panic!("{function_name} should exist"));
    function
        .instructions
        .iter()
        .find_map(|instruction| {
            if let LinkedInstructionKind::CallMethod {
                dispatch,
                debug_name,
                cache_site: Some(cache_site),
                ..
            } = &instruction.kind
                && matches_debug_name(program, *debug_name)
            {
                Some(LinkedMethodCallSite {
                    cache_site: *cache_site,
                    dispatch: *dispatch,
                    debug_name: *debug_name,
                })
            } else {
                None
            }
        })
        .unwrap_or_else(|| panic!("{function_name} should have a linked method call site"))
}
