use vela_bytecode::{
    CacheSiteId, DebugNameId, MethodDispatchHandle, ScriptFunctionHandle,
    linked::InstructionKind as LinkedInstructionKind, linked::LinkedMethodDispatchKind,
};
use vela_common::{ScalarValue, SourceId};
use vela_def::MethodId;
use vela_vm::{
    CallbackMethodInlineCacheTarget, MethodInlineCacheEntry, MethodInlineCacheTarget,
    StandardMethodReceiver, owned_value::OwnedValue,
};

use crate::engine::Engine;
use crate::runtime::{CallArgs, CallOptions, Runtime};

#[test]
fn linked_method_dispatch_inline_cache_populates_for_script_methods() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source(
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
    let mut runtime = Runtime::new(engine, program);
    let call = method_call_site(&runtime, "read_bonus");

    assert_eq!(
        runtime.state.inline_caches.method_dispatch(call.cache_site),
        None
    );

    let result = runtime
        .call("read_bonus", CallArgs::new(), CallOptions::unbounded())
        .expect("read_bonus should run");
    assert_eq!(
        runtime.value_to_owned(&result),
        Ok(OwnedValue::Scalar(ScalarValue::I64(7)))
    );

    let entry = runtime
        .state
        .inline_caches
        .method_dispatch(call.cache_site)
        .expect("method call should populate inline cache");
    assert_eq!(entry.dispatch, call.dispatch);
    assert_eq!(entry.debug_name, call.debug_name);
    assert!(matches!(
        entry.target,
        MethodInlineCacheTarget::Script { .. }
    ));
}

#[test]
fn linked_method_dispatch_inline_cache_misses_wrong_dispatch_guard() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source(
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
    let mut runtime = Runtime::new(engine, program);
    let call = method_call_site(&runtime, "read_bonus");
    runtime.state.inline_caches.set_method_dispatch(
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
        .state
        .inline_caches
        .method_dispatch(call.cache_site)
        .expect("wrong-dispatch entry should be replaced");
    assert_eq!(entry.dispatch, call.dispatch);
    assert_eq!(entry.debug_name, call.debug_name);
    assert!(matches!(
        entry.target,
        MethodInlineCacheTarget::Script { .. }
    ));
}

#[test]
fn linked_method_dispatch_inline_cache_misses_wrong_script_target_guard() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source(
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
    let mut runtime = Runtime::new(engine, program);
    let call = method_call_site(&runtime, "read_bonus");
    let (method_id, function) = script_method_target(&runtime, call.dispatch);
    runtime.state.inline_caches.set_method_dispatch(
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
        .state
        .inline_caches
        .method_dispatch(call.cache_site)
        .expect("wrong-target entry should be replaced");
    assert_eq!(entry.dispatch, call.dispatch);
    assert_eq!(entry.debug_name, call.debug_name);
    assert_eq!(
        entry.target,
        MethodInlineCacheTarget::Script {
            method_id,
            function,
        }
    );
}

#[test]
fn accepted_hot_reload_clears_linked_method_dispatch_inline_caches() {
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial(
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
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let initial_call = method_call_site(&runtime, "read_bonus");

    let first = runtime
        .call("read_bonus", CallArgs::new(), CallOptions::unbounded())
        .expect("initial read_bonus should run");
    assert_eq!(
        runtime.value_to_owned(&first),
        Ok(OwnedValue::Scalar(ScalarValue::I64(7)))
    );
    assert!(
        runtime
            .state
            .inline_caches
            .method_dispatch(initial_call.cache_site)
            .is_some(),
        "initial method call should populate its inline cache"
    );

    let update = runtime
        .compile_hot_reload_update(
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
        .apply_hot_update(update)
        .expect("method hot reload update should apply");
    assert!(report.accepted);

    let reloaded_call = method_call_site(&runtime, "read_bonus");
    assert_eq!(
        runtime
            .state
            .inline_caches
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
    assert!(
        runtime
            .state
            .inline_caches
            .method_dispatch(reloaded_call.cache_site)
            .is_some(),
        "reloaded method call should repopulate its inline cache"
    );
}

#[test]
fn rejected_hot_reload_preserves_linked_method_dispatch_inline_caches() {
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial(
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
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let initial_call = method_call_site(&runtime, "read_bonus");

    let first = runtime
        .call("read_bonus", CallArgs::new(), CallOptions::unbounded())
        .expect("initial read_bonus should run");
    assert_eq!(
        runtime.value_to_owned(&first),
        Ok(OwnedValue::Scalar(ScalarValue::I64(7)))
    );
    let initial_entry = runtime
        .state
        .inline_caches
        .method_dispatch(initial_call.cache_site)
        .expect("initial method call should populate its inline cache");

    let update = runtime
        .compile_hot_reload_update(
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
    let report = runtime
        .apply_hot_update_result_report(update)
        .expect("rejected method hot reload update should report");
    assert!(!report.accepted);
    assert_eq!(report.to_version, None);

    let active_call = method_call_site(&runtime, "read_bonus");
    assert_eq!(active_call.cache_site, initial_call.cache_site);
    assert_eq!(
        runtime
            .state
            .inline_caches
            .method_dispatch(active_call.cache_site),
        Some(initial_entry)
    );

    let second = runtime
        .call("read_bonus", CallArgs::new(), CallOptions::unbounded())
        .expect("active read_bonus should keep running after rejected reload");
    assert_eq!(
        runtime.value_to_owned(&second),
        Ok(OwnedValue::Scalar(ScalarValue::I64(7)))
    );
    assert_eq!(
        runtime
            .state
            .inline_caches
            .method_dispatch(active_call.cache_site),
        Some(initial_entry)
    );
}

#[test]
fn accepted_hot_reload_clears_callback_value_method_inline_caches() {
    let engine = Engine::builder()
        .with_standard_natives()
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial(
            SourceId::new(1),
            r#"
fn read_match() {
    return [1, 2, 3].any(|value| value == 2);
}
"#,
        )
        .expect("initial source should compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let initial_call = method_call_site(&runtime, "read_match");

    let first = runtime
        .call("read_match", CallArgs::new(), CallOptions::unbounded())
        .expect("initial read_match should run");
    assert_eq!(runtime.value_to_owned(&first), Ok(OwnedValue::Bool(true)));
    assert_callback_value_method_cache(&runtime, initial_call.cache_site);

    let update = runtime
        .compile_hot_reload_update(
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
        .apply_hot_update(update)
        .expect("callback hot reload update should apply");
    assert!(report.accepted);

    let reloaded_call = method_call_site(&runtime, "read_match");
    assert_eq!(
        runtime
            .state
            .inline_caches
            .method_dispatch(reloaded_call.cache_site),
        None
    );

    let second = runtime
        .call("read_match", CallArgs::new(), CallOptions::unbounded())
        .expect("reloaded read_match should run");
    assert_eq!(runtime.value_to_owned(&second), Ok(OwnedValue::Bool(false)));
    assert_callback_value_method_cache(&runtime, reloaded_call.cache_site);
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
    let program = runtime
        .image
        .linked_program()
        .expect("runtime image should have a linked program");
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
    let entry = runtime
        .state
        .inline_caches
        .method_dispatch(site)
        .expect("callback value method call should populate inline cache");
    let MethodInlineCacheTarget::CallbackValue {
        callback_method, ..
    } = entry.target
    else {
        panic!("method cache should store a callback value target");
    };
    assert_eq!(callback_method.receiver, StandardMethodReceiver::Array);
    assert_eq!(callback_method.target, CallbackMethodInlineCacheTarget::Any);
}

fn method_call_site(runtime: &Runtime, function_name: &str) -> LinkedMethodCallSite {
    let program = runtime
        .image
        .linked_program()
        .expect("runtime image should have a linked program");
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
