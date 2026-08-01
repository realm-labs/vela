use std::sync::Arc;
use std::sync::Mutex;

use vela_bytecode::CacheSiteKind;
use vela_host::access::HostAccess;
use vela_host::mock::MockStateAdapter;
use vela_hot_reload::error::HotReloadErrorKind;
use vela_vm::owned_value::OwnedValue;
use vela_vm::value::Value;

use crate::engine::Engine;
use vela_common::SourceId;
use vela_vm::budget::{CollectionLimits, ExecutionLimits};

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
fn scoped_task_without_host_scope_fails_deterministically() {
    let engine = Engine::builder()
        .capability(vela_common::Capability::TaskSpawn)
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            r#"
async fn repair(value: i64) -> i64 { return value + 1; }
fn main() { task::spawn_scoped(repair(41)); }
"#,
        )
        .expect("task fixture compiles");
    let mut runtime = Runtime::new_compiled(engine, program).expect("runtime builds");

    let error = runtime
        .call("main", CallArgs::new(), CallOptions::unbounded())
        .expect_err("ordinary call has no installed host task scope");
    assert_eq!(
        error.kind(),
        vela_vm::error::VmErrorKind::TaskScopeUnavailable
    );
}

#[derive(Default)]
struct RecordingTaskHost {
    admitted: Mutex<Vec<crate::task::ScopedTask>>,
}

impl crate::task::ScopedTaskHost for RecordingTaskHost {
    fn admit(&self, task: crate::task::ScopedTask) -> Result<(), crate::task::TaskAdmissionError> {
        self.admitted.lock().expect("task host lock").push(task);
        Ok(())
    }
}

#[test]
fn scoped_task_admission_runs_worker_in_fresh_runtime() {
    let engine = Engine::builder()
        .capability(vela_common::Capability::TaskSpawn)
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            r#"
state counter: i64 = 0;
async fn repair(value: i64) -> i64 {
    counter += value;
    return counter;
}
fn main() {
    counter = 100;
    task::spawn_scoped(repair(2));
    return counter;
}
"#,
        )
        .expect("task fixture compiles");
    let mut runtime = Runtime::new_compiled(engine, program).expect("runtime builds");
    let host = Arc::new(RecordingTaskHost::default());
    let limits =
        ExecutionLimits::new(10_000, 1 << 20, 64).with_collection_limits(CollectionLimits {
            max_array_len: 1_024,
            max_map_entries: 1_024,
            max_set_len: 1_024,
        });
    let policy = crate::task::TaskPolicy::new(
        std::num::NonZeroUsize::new(4).expect("non-zero"),
        std::num::NonZeroUsize::new(4).expect("non-zero"),
        limits,
        std::num::NonZeroU64::new(16).expect("non-zero"),
        std::time::Duration::from_secs(1),
        vela_common::CapabilitySet::new().with(vela_common::Capability::TaskSpawn),
    )
    .expect("finite task policy");
    let scope = crate::task::TaskScope::new(host.clone(), policy);

    let parent = runtime
        .call(
            "main",
            CallArgs::new(),
            CallOptions::unbounded().with_task_scope(scope),
        )
        .expect("caller admits detached worker");
    assert_eq!(runtime.value_to_owned(&parent), Ok(OwnedValue::i64(100)));
    let task = host
        .admitted
        .lock()
        .expect("task host lock")
        .pop()
        .expect("one admitted task");
    let (_, _, mut future) = task.into_parts();
    let waker = std::task::Waker::noop();
    let mut context = std::task::Context::from_waker(waker);
    assert_eq!(
        future.as_mut().poll(&mut context),
        std::task::Poll::Ready(crate::task::ScopedTaskOutcome::Completed(OwnedValue::i64(
            2
        )))
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
fn shared_actor_runtimes_share_execution_data_but_isolate_mutable_owners() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source(
            r#"
state counter: i64 = 0;
fn make_value() { return [1, 2, 3]; }
fn echo(value) { return value; }
fn cached(value) { return value.starts_with("q"); }
"#,
        )
        .expect("fixture compiles");
    let image = RuntimeImage::new_compiled(engine, program).into_shared();
    let mut first = SharedRuntime::from_shared_image(image.clone()).expect("first runtime");
    let mut second = SharedRuntime::from_shared_image(image).expect("second runtime");

    assert!(Arc::ptr_eq(
        first.image.execution_data(),
        second.image.execution_data()
    ));
    assert!(!std::ptr::eq(
        &first.state.vm_states.heap,
        &second.state.vm_states.heap
    ));
    assert!(!Arc::ptr_eq(
        &first.state.vm_states.retained_values,
        &second.state.vm_states.retained_values
    ));
    assert!(!std::ptr::eq(
        &first.state.extern_states,
        &second.state.extern_states
    ));
    assert_ne!(first.state.id, second.state.id);

    let first_value = first
        .call("make_value", CallArgs::new(), CallOptions::unbounded())
        .expect("first Runtime retains its heap value");
    let error = second
        .call(
            "echo",
            CallArgs::from_values([first_value]),
            CallOptions::unbounded(),
        )
        .expect_err("retained values must not cross Actor Runtime ownership");
    assert!(matches!(
        error.kind(),
        vela_vm::error::VmErrorKind::TypeMismatch { .. }
    ));

    let site = first
        .image
        .linked_artifact()
        .cache_layout()
        .iter()
        .find(|site| site.kind == CacheSiteKind::MethodCall)
        .expect("cached fixture should have a method site")
        .id;
    first
        .call(
            "cached",
            CallArgs::from_positional([OwnedValue::String("quest".to_owned())]),
            CallOptions::unbounded(),
        )
        .expect("first Runtime populates shared cache metadata");
    assert!(
        second
            .image
            .execution_data()
            .inline_caches()
            .dynamic_method_dispatch(site)
            .is_some()
    );
    assert_eq!(
        second.state("main::counter"),
        Ok(Some(OwnedValue::from(0_i64)))
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

#[test]
fn runtime_state_initializers_share_one_transaction_allocation_budget() {
    let engine = Engine::builder().build().expect("engine should build");
    let array_bytes = std::mem::size_of::<Vec<Value>>() + 4 * std::mem::size_of::<Value>();
    let limits = RuntimeInitializationLimits::new(100, array_bytes + array_bytes / 2, 8);
    let one = engine
        .compile_source("state first: Array<i64> = [1, 2, 3, 4];")
        .expect("single initializer compiles");
    Runtime::builder(engine.clone(), one)
        .expect("single-state runtime image links")
        .with_initialization_limits(limits)
        .build()
        .expect("one initializer fits the transaction budget");

    let two = engine
        .compile_source(
            "state first: Array<i64> = [1, 2, 3, 4]; state second: Array<i64> = [5, 6, 7, 8];",
        )
        .expect("two initializers compile");
    let error = match Runtime::builder(engine, two)
        .expect("two-state runtime image links")
        .with_initialization_limits(limits)
        .build()
    {
        Ok(_) => panic!("initializers must share the transaction allocation budget"),
        Err(error) => error,
    };

    assert!(matches!(
        error,
        RuntimeBuildError::Initializer { state, .. }
            if state == "main::first" || state == "main::second"
    ));
}

#[test]
fn runtime_state_initializers_share_one_transaction_execution_budget() {
    let engine = Engine::builder().build().expect("engine should build");
    let one = engine
        .compile_source(
            "fn compute() -> i64 { let first = 1; let second = 2; return first + second; } state first: i64 = compute();",
        )
        .expect("single initializer compiles");
    let two = engine
        .compile_source(
            "fn compute() -> i64 { let first = 1; let second = 2; return first + second; } state first: i64 = compute(); state second: i64 = compute();",
        )
        .expect("two initializers compile");
    let one = RuntimeImage::new_compiled(engine.clone(), one).into_shared();
    let two = RuntimeImage::new_compiled(engine, two).into_shared();

    let aggregate_failure = (1..=64).find_map(|execution_units| {
        let limits = RuntimeInitializationLimits::new(execution_units, 1024, 8);
        let one_result = SharedRuntime::builder_from_shared_image(one.clone())
            .with_initialization_limits(limits)
            .build();
        let two_result = SharedRuntime::builder_from_shared_image(two.clone())
            .with_initialization_limits(limits)
            .build();
        match (one_result, two_result) {
            (Ok(_), Err(error @ RuntimeBuildError::Initializer { .. })) => Some(error),
            _ => None,
        }
    });

    assert!(
        aggregate_failure.is_some(),
        "two individually valid initializers must exhaust a shared execution budget"
    );
}

#[test]
fn reload_charges_live_heap_staging_to_the_initializer_transaction() {
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_with_id(
            SourceId::new(801),
            "state existing: i64 = 3; fn read() { return existing; }",
        )
        .expect("initial generation compiles");
    let update = engine
        .compile_reload_with_id(
            &initial,
            SourceId::new(802),
            "state existing: i64 = 3; state added: Array<i64> = [1, 2, 3, 4]; fn read() { return existing; }",
        )
        .expect("update compiles");
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime initializes");
    let array_bytes = std::mem::size_of::<Vec<Value>>() + 4 * std::mem::size_of::<Value>();
    let limits = RuntimeInitializationLimits::new(100, array_bytes + array_bytes / 2, 8);

    let error = match runtime.prepare_hot_update_state(&update, limits) {
        Ok(_) => panic!("live-heap staging must consume the shared transaction budget"),
        Err(error) => error,
    };

    assert!(matches!(
        error.kind,
        HotReloadErrorKind::StateInitializerFailed { ref state, .. }
            if state == "main::added"
    ));
    assert_eq!(
        runtime.state("main::existing"),
        Ok(Some(OwnedValue::from(3_i64)))
    );
    assert_eq!(runtime.state("main::added"), Ok(None));
}

#[test]
fn reload_staging_preserves_initializer_aliases_and_cycles() {
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial_with_id(
            SourceId::new(803),
            "state existing: i64 = 3; fn graph_ok() { return false; }",
        )
        .expect("initial generation compiles");
    let update = engine
        .compile_reload_with_id(
            &initial,
            SourceId::new(804),
            r#"
fn build_graph() -> Array {
    let shared = [7];
    let root = [];
    root.push(shared);
    root.push(shared);
    root.push(root);
    return root;
}

state existing: i64 = 3;
state graph: Array = build_graph();

fn graph_ok() {
    return graph[0] === graph[1] && graph[2] === graph;
}
"#,
        )
        .expect("cyclic state update compiles");
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime initializes");

    let report = runtime
        .apply_reload_update_for_test(update)
        .expect("reload applies");
    assert!(report.accepted, "{report:?}");
    let result = runtime
        .call("graph_ok", CallArgs::new(), CallOptions::unbounded())
        .expect("copied graph remains usable");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::Bool(true)));
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
