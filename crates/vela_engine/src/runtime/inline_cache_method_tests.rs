use vela_bytecode::{
    CacheSiteId, DebugNameId, MethodDispatchHandle,
    linked::InstructionKind as LinkedInstructionKind,
};
use vela_common::{ScalarValue, SourceId};
use vela_def::MethodId;
use vela_vm::{MethodInlineCacheEntry, MethodInlineCacheTarget, owned_value::OwnedValue};

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

#[derive(Clone, Copy)]
struct LinkedMethodCallSite {
    cache_site: CacheSiteId,
    dispatch: MethodDispatchHandle,
    debug_name: DebugNameId,
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
