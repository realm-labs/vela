use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::task::{Context, Poll, Waker};

use vela_bytecode::CacheSiteKind;
use vela_host::access::HostAccess;
use vela_host::mock::MockStateAdapter;
use vela_hot_reload::error::HotReloadErrorKind;
use vela_vm::owned_value::OwnedValue;
use vela_vm::value::Value;

use crate::engine::Engine;
use vela_common::SourceId;
use vela_vm::budget::{CollectionLimits, ExecutionLimits};
use vela_vm::error::{VmError, VmErrorKind};

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

#[test]
fn scoped_task_scope_validation_precedes_runtime_value_detachment() {
    let engine = Engine::builder()
        .capability(vela_common::Capability::TaskSpawn)
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            r#"
async fn repair(value: Any) { return; }
fn main(value: Any) { task::spawn_scoped(repair(value)); }
"#,
        )
        .expect("runtime-checked task fixture compiles");
    let mut runtime = Runtime::new_compiled(engine, program).expect("runtime builds");
    let host_ref = vela_host::path::HostRef::new(
        vela_common::HostTypeId::new(71),
        vela_common::HostObjectId::new(8),
        1,
    );

    let error = runtime
        .call(
            "main",
            CallArgs::from_positional([OwnedValue::HostRef(host_ref)]),
            CallOptions::unbounded(),
        )
        .expect_err("missing scope wins before runtime detachment validation");

    assert_eq!(error.kind(), VmErrorKind::TaskScopeUnavailable);
}

#[test]
fn scoped_task_runtime_check_rejects_host_ref_hidden_behind_any() {
    let engine = Engine::builder()
        .capability(vela_common::Capability::TaskSpawn)
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            r#"
async fn repair(value: Any) { return; }
fn main(value: Any) { task::spawn_scoped(repair(value)); }
"#,
        )
        .expect("runtime-checked task fixture compiles");
    let mut runtime = Runtime::new_compiled(engine, program).expect("runtime builds");
    let host = Arc::new(RecordingTaskHost::default());
    let host_ref = vela_host::path::HostRef::new(
        vela_common::HostTypeId::new(71),
        vela_common::HostObjectId::new(8),
        1,
    );

    let error = runtime
        .call(
            "main",
            CallArgs::from_positional([OwnedValue::HostRef(host_ref)]),
            CallOptions::unbounded().with_task_scope(crate::task::TaskScope::new(
                host.clone(),
                finite_task_policy(std::time::Duration::from_secs(1)),
            )),
        )
        .expect_err("runtime check rejects hidden HostRef");

    assert!(matches!(
        error.kind(),
        VmErrorKind::TaskValueNotDetachable { path, kind }
            if path == "argument[0]"
                && kind == vela_common::NonDetachableValueKind::HostReference
    ));
    assert!(host.admitted.lock().expect("task host lock").is_empty());
}

#[derive(Default)]
struct RecordingTaskHost {
    admitted: Mutex<Vec<crate::task::ScopedTask>>,
}

fn finite_task_policy(timeout: std::time::Duration) -> crate::task::TaskPolicy {
    finite_task_policy_with_host_calls(timeout, 16)
}

fn finite_task_policy_with_host_calls(
    timeout: std::time::Duration,
    max_host_calls: u64,
) -> crate::task::TaskPolicy {
    let limits =
        ExecutionLimits::new(10_000, 1 << 20, 64).with_collection_limits(CollectionLimits {
            max_array_len: 1_024,
            max_map_entries: 1_024,
            max_set_len: 1_024,
        });
    crate::task::TaskPolicy::new(
        std::num::NonZeroUsize::new(4).expect("non-zero"),
        std::num::NonZeroUsize::new(4).expect("non-zero"),
        limits,
        std::num::NonZeroU64::new(max_host_calls).expect("non-zero"),
        timeout,
        vela_common::CapabilitySet::new().with(vela_common::Capability::TaskSpawn),
    )
    .expect("finite task policy")
}

fn take_task(host: &RecordingTaskHost) -> crate::task::ScopedTask {
    host.admitted
        .lock()
        .expect("task host lock")
        .pop()
        .expect("one admitted task")
}

fn import_task_result(image: &vela_vm::DetachedValueImage) -> Vec<Value> {
    let mut heap = vela_vm::heap::ScriptHeap::new();
    let mut budget = vela_vm::budget::ExecutionBudget::new(100, 4096, 8);
    image
        .import_into(&mut heap, &mut budget)
        .expect("host can import detached outcome")
}

fn import_task_owned_result(image: &vela_vm::DetachedValueImage) -> OwnedValue {
    let mut heap = vela_vm::heap::ScriptHeap::new();
    let mut budget = vela_vm::budget::ExecutionBudget::new(100, 4096, 8);
    let roots = image
        .import_into(&mut heap, &mut budget)
        .expect("host can import detached outcome");
    vela_vm::persistent_value_to_owned(
        roots.first().expect("task result image has one root"),
        &mut heap,
    )
    .expect("acyclic task fixture result converts to owned value")
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
    let policy = finite_task_policy(std::time::Duration::from_secs(1));
    let scope = crate::task::TaskScope::new(host.clone(), policy);

    let parent = runtime
        .call(
            "main",
            CallArgs::new(),
            CallOptions::unbounded().with_task_scope(scope),
        )
        .expect("caller admits detached worker");
    assert_eq!(runtime.value_to_owned(&parent), Ok(OwnedValue::i64(100)));
    let task = take_task(&host);
    let (_, _, mut future) = task.into_parts();
    let waker = std::task::Waker::noop();
    let mut context = std::task::Context::from_waker(waker);
    let std::task::Poll::Ready(crate::task::ScopedTaskOutcome::Completed(image)) =
        future.as_mut().poll(&mut context)
    else {
        panic!("detached worker should complete with an owned image");
    };
    assert_eq!(import_task_result(&image), [Value::i64(2)]);
}

#[test]
fn scoped_task_transfer_preserves_cross_argument_aliases_and_cycles() {
    let engine = Engine::builder()
        .capability(vela_common::Capability::TaskSpawn)
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            r#"
async fn inspect(root: Array, shared: Array) -> bool {
    return root[0] === shared && root[1] === root;
}

fn main() {
    let shared = [7];
    let root = [];
    root.push(shared);
    root.push(root);
    task::spawn_scoped(inspect(root, shared));
}
"#,
        )
        .expect("cyclic task fixture compiles");
    let mut runtime = Runtime::new_compiled(engine, program).expect("runtime builds");
    let host = Arc::new(RecordingTaskHost::default());
    let policy = finite_task_policy(std::time::Duration::from_secs(1));
    let scope = crate::task::TaskScope::new(host.clone(), policy);

    runtime
        .call(
            "main",
            CallArgs::new(),
            CallOptions::unbounded().with_task_scope(scope),
        )
        .expect("caller admits cyclic graph");
    let task = take_task(&host);
    let (_, _, mut future) = task.into_parts();
    let waker = std::task::Waker::noop();
    let mut context = std::task::Context::from_waker(waker);
    let std::task::Poll::Ready(crate::task::ScopedTaskOutcome::Completed(image)) =
        future.as_mut().poll(&mut context)
    else {
        panic!("graph inspection worker should complete");
    };
    assert_eq!(import_task_result(&image), [Value::Bool(true)]);
}

struct PendingOnce {
    polls: Arc<AtomicUsize>,
}

impl std::future::Future for PendingOnce {
    type Output = vela_vm::error::VmResult<OwnedValue>;

    fn poll(self: std::pin::Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        if self.polls.fetch_add(1, Ordering::SeqCst) == 0 {
            context.waker().wake_by_ref();
            Poll::Pending
        } else {
            Poll::Ready(Ok(OwnedValue::i64(9)))
        }
    }
}

struct NeverReady {
    dropped: Arc<AtomicBool>,
}

impl std::future::Future for NeverReady {
    type Output = vela_vm::error::VmResult<OwnedValue>;

    fn poll(self: std::pin::Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Pending
    }
}

impl Drop for NeverReady {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::SeqCst);
    }
}

fn pending_task_fixture(
    function: impl for<'call> Fn(&'call [OwnedValue]) -> vela_vm::NativeCallFuture<'call>
    + Send
    + Sync
    + 'static,
) -> (Engine, vela_bytecode::compiler::CompiledProgram) {
    let engine = Engine::builder()
        .capability(vela_common::Capability::TaskSpawn)
        .register_async_fn(
            crate::native::NativeFunctionDesc::new(
                "test::pending_task",
                vela_def::FunctionId::new(0x7A51),
            )
            .returns(crate::native::TypeHint::i64())
            .access(crate::native::FunctionAccess::public()),
            function,
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            r#"
async fn repair() -> Array {
    let value = test::pending_task().await;
    return [value, [value + 1]];
}
fn main() { task::spawn_scoped(repair()); }
"#,
        )
        .expect("pending task fixture compiles");
    (engine, program)
}

#[test]
fn scoped_task_worker_uses_existing_pending_async_driver() {
    let polls = Arc::new(AtomicUsize::new(0));
    let fixture_polls = Arc::clone(&polls);
    let (engine, program) = pending_task_fixture(move |_args| {
        Box::pin(PendingOnce {
            polls: Arc::clone(&fixture_polls),
        })
    });
    let mut runtime = Runtime::new_compiled(engine, program).expect("runtime builds");
    let host = Arc::new(RecordingTaskHost::default());
    runtime
        .call(
            "main",
            CallArgs::new(),
            CallOptions::unbounded().with_task_scope(crate::task::TaskScope::new(
                host.clone(),
                finite_task_policy(std::time::Duration::from_secs(1)),
            )),
        )
        .expect("sync caller admits pending worker");
    assert_eq!(polls.load(Ordering::SeqCst), 0);

    let (_, _, mut future) = take_task(&host).into_parts();
    let mut context = Context::from_waker(Waker::noop());
    assert!(matches!(future.as_mut().poll(&mut context), Poll::Pending));
    assert_eq!(polls.load(Ordering::SeqCst), 1);
    let Poll::Ready(crate::task::ScopedTaskOutcome::Completed(image)) =
        future.as_mut().poll(&mut context)
    else {
        panic!("second poll should finish pending-once native");
    };
    assert_eq!(
        import_task_owned_result(&image),
        OwnedValue::Array(vec![
            OwnedValue::i64(9),
            OwnedValue::Array(vec![OwnedValue::i64(10)]),
        ])
    );
}

#[test]
fn scoped_task_explicit_scope_close_cancels_after_cleanup() {
    let dropped = Arc::new(AtomicBool::new(false));
    let fixture_dropped = Arc::clone(&dropped);
    let (engine, program) = pending_task_fixture(move |_args| {
        Box::pin(NeverReady {
            dropped: Arc::clone(&fixture_dropped),
        })
    });
    let mut runtime = Runtime::new_compiled(engine, program).expect("runtime builds");
    let host = Arc::new(RecordingTaskHost::default());
    runtime
        .call(
            "main",
            CallArgs::new(),
            CallOptions::unbounded().with_task_scope(crate::task::TaskScope::new(
                host.clone(),
                finite_task_policy(std::time::Duration::from_secs(1)),
            )),
        )
        .expect("sync caller admits pending worker");
    let mut task = take_task(&host);
    let mut context = Context::from_waker(Waker::noop());
    assert!(matches!(task.poll(&mut context), Poll::Pending));
    let outcome = task.cancel(crate::task::TaskCancellationReason::ScopeClosed);
    assert!(dropped.load(Ordering::SeqCst));
    assert!(matches!(
        outcome,
        crate::task::ScopedTaskOutcome::Failed(crate::task::TaskError {
            kind: crate::task::TaskErrorKind::Cancelled(
                crate::task::TaskCancellationReason::ScopeClosed
            ),
            ..
        })
    ));
}

#[test]
fn scoped_task_dropped_future_cleans_up_pending_native() {
    let dropped = Arc::new(AtomicBool::new(false));
    let fixture_dropped = Arc::clone(&dropped);
    let (engine, program) = pending_task_fixture(move |_args| {
        Box::pin(NeverReady {
            dropped: Arc::clone(&fixture_dropped),
        })
    });
    let mut runtime = Runtime::new_compiled(engine, program).expect("runtime builds");
    let host = Arc::new(RecordingTaskHost::default());
    runtime
        .call(
            "main",
            CallArgs::new(),
            CallOptions::unbounded().with_task_scope(crate::task::TaskScope::new(
                host.clone(),
                finite_task_policy(std::time::Duration::from_secs(1)),
            )),
        )
        .expect("sync caller admits pending worker");
    let task = take_task(&host);
    let (_, _, mut future) = task.into_parts();
    let mut context = Context::from_waker(Waker::noop());
    assert!(matches!(future.as_mut().poll(&mut context), Poll::Pending));
    drop(future);
    assert!(dropped.load(Ordering::SeqCst));
}

#[test]
fn scoped_task_deadline_contains_pending_worker_and_cleans_up() {
    let dropped = Arc::new(AtomicBool::new(false));
    let fixture_dropped = Arc::clone(&dropped);
    let (engine, program) = pending_task_fixture(move |_args| {
        Box::pin(NeverReady {
            dropped: Arc::clone(&fixture_dropped),
        })
    });
    let mut runtime = Runtime::new_compiled(engine, program).expect("runtime builds");
    let host = Arc::new(RecordingTaskHost::default());
    runtime
        .call(
            "main",
            CallArgs::new(),
            CallOptions::unbounded().with_task_scope(crate::task::TaskScope::new(
                host.clone(),
                finite_task_policy(std::time::Duration::from_millis(1)),
            )),
        )
        .expect("sync caller admits deadline-bound worker");
    let mut task = take_task(&host);
    let mut context = Context::from_waker(Waker::noop());
    assert!(matches!(task.poll(&mut context), Poll::Pending));
    std::thread::sleep(std::time::Duration::from_millis(5));

    assert!(matches!(
        task.poll(&mut context),
        Poll::Ready(crate::task::ScopedTaskOutcome::Failed(
            crate::task::TaskError {
                kind: crate::task::TaskErrorKind::DeadlineExceeded,
                ..
            }
        ))
    ));
    assert!(dropped.load(Ordering::SeqCst));
}

#[test]
fn scoped_task_worker_error_becomes_structured_failure() {
    let (engine, program) = pending_task_fixture(move |_args| {
        Box::pin(async {
            Err(VmError::new(VmErrorKind::TypeMismatch {
                operation: "detached worker fixture",
            }))
        })
    });
    let mut runtime = Runtime::new_compiled(engine, program).expect("runtime builds");
    let host = Arc::new(RecordingTaskHost::default());
    runtime
        .call(
            "main",
            CallArgs::new(),
            CallOptions::unbounded().with_task_scope(crate::task::TaskScope::new(
                host.clone(),
                finite_task_policy(std::time::Duration::from_secs(1)),
            )),
        )
        .expect("sync caller admits failing worker");
    let mut task = take_task(&host);
    let mut context = Context::from_waker(Waker::noop());

    assert!(matches!(
        task.poll(&mut context),
        Poll::Ready(crate::task::ScopedTaskOutcome::Failed(
            crate::task::TaskError {
                kind: crate::task::TaskErrorKind::WorkerError,
                ..
            }
        ))
    ));
}

struct PanicOnPoll;

impl std::future::Future for PanicOnPoll {
    type Output = vela_vm::error::VmResult<OwnedValue>;

    fn poll(self: std::pin::Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<Self::Output> {
        panic!("detached native panic fixture");
    }
}

#[test]
fn scoped_task_worker_panic_is_contained() {
    let (engine, program) = pending_task_fixture(move |_args| Box::pin(PanicOnPoll));
    let mut runtime = Runtime::new_compiled(engine, program).expect("runtime builds");
    let host = Arc::new(RecordingTaskHost::default());
    runtime
        .call(
            "main",
            CallArgs::new(),
            CallOptions::unbounded().with_task_scope(crate::task::TaskScope::new(
                host.clone(),
                finite_task_policy(std::time::Duration::from_secs(1)),
            )),
        )
        .expect("sync caller admits panicking worker");
    let mut task = take_task(&host);
    let mut context = Context::from_waker(Waker::noop());

    let Poll::Ready(crate::task::ScopedTaskOutcome::Failed(error)) = task.poll(&mut context) else {
        panic!("panic must become a task failure");
    };
    assert_eq!(error.kind, crate::task::TaskErrorKind::WorkerPanicked);
    assert!(error.detail.contains("detached native panic fixture"));
}

struct RejectingTaskHost {
    attempts: AtomicUsize,
}

impl crate::task::ScopedTaskHost for RejectingTaskHost {
    fn admit(&self, _task: crate::task::ScopedTask) -> Result<(), crate::task::TaskAdmissionError> {
        self.attempts.fetch_add(1, Ordering::SeqCst);
        Err(crate::task::TaskAdmissionError::CapacityExceeded { maximum: 4 })
    }
}

#[test]
fn scoped_task_capacity_rejection_publishes_no_worker() {
    let polls = Arc::new(AtomicUsize::new(0));
    let fixture_polls = Arc::clone(&polls);
    let (engine, program) = pending_task_fixture(move |_args| {
        Box::pin(PendingOnce {
            polls: Arc::clone(&fixture_polls),
        })
    });
    let mut runtime = Runtime::new_compiled(engine, program).expect("runtime builds");
    let host = Arc::new(RejectingTaskHost {
        attempts: AtomicUsize::new(0),
    });

    let error = runtime
        .call(
            "main",
            CallArgs::new(),
            CallOptions::unbounded().with_task_scope(crate::task::TaskScope::new(
                host.clone(),
                finite_task_policy(std::time::Duration::from_secs(1)),
            )),
        )
        .expect_err("host capacity refusal rejects admission");

    assert!(matches!(
        error.kind(),
        VmErrorKind::TaskAdmissionDenied { reason } if reason.contains("capacity 4")
    ));
    assert_eq!(host.attempts.load(Ordering::SeqCst), 1);
    assert_eq!(polls.load(Ordering::SeqCst), 0);
}

#[test]
fn scoped_task_host_call_budget_stops_before_second_native_invocation() {
    let calls = Arc::new(AtomicUsize::new(0));
    let fixture_calls = Arc::clone(&calls);
    let engine = Engine::builder()
        .capability(vela_common::Capability::TaskSpawn)
        .register_async_fn(
            crate::native::NativeFunctionDesc::new(
                "test::counted_task_call",
                vela_def::FunctionId::new(0x7A52),
            )
            .returns(crate::native::TypeHint::i64())
            .access(crate::native::FunctionAccess::public()),
            move |_args| {
                fixture_calls.fetch_add(1, Ordering::SeqCst);
                Box::pin(async { Ok(OwnedValue::i64(1)) })
            },
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            r#"
async fn repair() -> i64 {
    let first = test::counted_task_call().await;
    let second = test::counted_task_call().await;
    return first + second;
}
fn main() { task::spawn_scoped(repair()); }
"#,
        )
        .expect("host-call task fixture compiles");
    let mut runtime = Runtime::new_compiled(engine, program).expect("runtime builds");
    let host = Arc::new(RecordingTaskHost::default());
    runtime
        .call(
            "main",
            CallArgs::new(),
            CallOptions::unbounded().with_task_scope(crate::task::TaskScope::new(
                host.clone(),
                finite_task_policy_with_host_calls(std::time::Duration::from_secs(1), 1),
            )),
        )
        .expect("sync caller admits host-call-limited worker");
    let mut task = take_task(&host);
    let mut context = Context::from_waker(Waker::noop());

    assert!(matches!(
        task.poll(&mut context),
        Poll::Ready(crate::task::ScopedTaskOutcome::Failed(
            crate::task::TaskError {
                kind: crate::task::TaskErrorKind::BudgetExceeded,
                ..
            }
        ))
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
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
