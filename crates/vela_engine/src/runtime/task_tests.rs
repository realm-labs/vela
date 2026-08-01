use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use vela_vm::budget::{CollectionLimits, ExecutionLimits};
use vela_vm::error::{VmError, VmErrorKind};
use vela_vm::owned_value::OwnedValue;
use vela_vm::value::Value;

use crate::engine::Engine;

use super::{CallArgs, CallOptions, Runtime};

mod observability;

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
    finite_task_policy_with_capabilities(
        timeout,
        max_host_calls,
        vela_common::CapabilitySet::new().with(vela_common::Capability::TaskSpawn),
    )
}

fn finite_task_policy_with_capabilities(
    timeout: std::time::Duration,
    max_host_calls: u64,
    capabilities: vela_common::CapabilitySet,
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
        capabilities,
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

#[derive(Default)]
struct RequestLifecycleHost {
    state: Mutex<RequestLifecycleState>,
}

#[derive(Default)]
struct RequestLifecycleState {
    closed: bool,
    active: Vec<crate::task::ScopedTask>,
    completions: std::collections::VecDeque<crate::task::ScopedTaskCompletion>,
    terminal_errors: Vec<crate::task::TaskError>,
}

impl crate::task::ScopedTaskHost for RequestLifecycleHost {
    fn admit(&self, task: crate::task::ScopedTask) -> Result<(), crate::task::TaskAdmissionError> {
        let mut state = self.state.lock().expect("request lifecycle lock");
        if state.closed {
            return Err(crate::task::TaskAdmissionError::ScopeClosed);
        }
        let maximum = task.capsule().policy().max_active_tasks().get();
        if state.active.len() >= maximum {
            return Err(crate::task::TaskAdmissionError::CapacityExceeded { maximum });
        }
        state.active.push(task);
        Ok(())
    }
}

impl RequestLifecycleHost {
    fn poll_one(&self) -> bool {
        let mut state = self.state.lock().expect("request lifecycle lock");
        let Some(task) = state.active.first_mut() else {
            return false;
        };
        let mut context = Context::from_waker(Waker::noop());
        let Poll::Ready(completion) = task.poll_completion(&mut context) else {
            return false;
        };
        drop(state.active.swap_remove(0));
        let maximum = completion.capsule().policy().max_queued_completions().get();
        if state.completions.len() >= maximum {
            let cancelled =
                completion.cancel(crate::task::TaskCancellationReason::CompletionQueueFull);
            let crate::task::ScopedTaskOutcome::Failed(error) = cancelled.outcome() else {
                unreachable!("cancelled completion must be a failure");
            };
            state.terminal_errors.push(error.clone());
        } else {
            state.completions.push_back(completion);
        }
        true
    }

    fn shutdown(&self) {
        let mut state = self.state.lock().expect("request lifecycle lock");
        if state.closed {
            return;
        }
        state.closed = true;
        for task in std::mem::take(&mut state.active) {
            let cancelled =
                task.cancel_completion(crate::task::TaskCancellationReason::HostShutdown);
            let crate::task::ScopedTaskOutcome::Failed(error) = cancelled.outcome() else {
                unreachable!("cancelled task must be a failure");
            };
            state.terminal_errors.push(error.clone());
        }
        while let Some(completion) = state.completions.pop_front() {
            let cancelled = completion.cancel(crate::task::TaskCancellationReason::HostShutdown);
            let crate::task::ScopedTaskOutcome::Failed(error) = cancelled.outcome() else {
                unreachable!("cancelled completion must be a failure");
            };
            state.terminal_errors.push(error.clone());
        }
    }

    fn counts(&self) -> (usize, usize, usize) {
        let state = self.state.lock().expect("request lifecycle lock");
        (
            state.active.len(),
            state.completions.len(),
            state.terminal_errors.len(),
        )
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
fn scoped_task_continuation_runs_only_when_host_resumes_completion() {
    let observations = Arc::new(Mutex::new(Vec::new()));
    let native_observations = Arc::clone(&observations);
    let engine = Engine::builder()
        .capabilities(vela_common::CapabilitySet::all())
        .register_native_fn(
            crate::native::NativeFunctionDesc::new(
                "test::observe_continuation",
                vela_def::FunctionId::new(0x7A53),
            )
            .param("aliases_preserved", crate::native::TypeHint::boolean())
            .param("is_ok", crate::native::TypeHint::boolean())
            .param("turn", crate::native::TypeHint::i64())
            .returns(crate::native::TypeHint::unit())
            .effects(crate::native::EffectSet::host_write())
            .access(crate::native::FunctionAccess::public()),
            move |args| {
                native_observations
                    .lock()
                    .expect("observation lock")
                    .push((std::thread::current().id(), args.to_vec()));
                Ok(OwnedValue::Unit)
            },
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            r#"
async fn repair() -> Array {
    let shared = [42];
    return [shared, shared];
}
fn finish(outcome: Result<Array, task::Error>, turn: i64) {
    let graph = outcome.unwrap_or([]);
    test::observe_continuation(graph[0] === graph[1], outcome.is_ok(), turn);
}
fn main() { task::spawn_scoped_then(repair(), finish); }
"#,
        )
        .expect("continuation fixture compiles");
    let mut runtime = Runtime::new_compiled(engine, program).expect("runtime builds");
    let host = Arc::new(RecordingTaskHost::default());
    let scope = crate::task::TaskScope::new(
        host.clone(),
        finite_task_policy_with_capabilities(
            std::time::Duration::from_secs(1),
            16,
            vela_common::CapabilitySet::all(),
        ),
    );
    runtime
        .call(
            "main",
            CallArgs::new(),
            CallOptions::unbounded().with_task_scope(scope.clone()),
        )
        .expect("caller admits detached worker");

    let task = take_task(&host);
    assert!(observations.lock().expect("observation lock").is_empty());
    let worker = std::thread::spawn(move || {
        let mut completion = task.into_completion_future();
        let mut context = Context::from_waker(Waker::noop());
        let Poll::Ready(completion) = completion.as_mut().poll(&mut context) else {
            panic!("worker completion should be host-observable");
        };
        completion
    });
    let completion = worker.join().expect("background worker thread");
    assert!(observations.lock().expect("observation lock").is_empty());
    let resume_thread = std::thread::current().id();
    let resumed = completion.resume(
        CallArgs::from_positional([OwnedValue::i64(77)]),
        CallOptions::unbounded(),
    );
    assert!(
        matches!(resumed, crate::task::TaskContinuationOutcome::Completed),
        "unexpected continuation result: {resumed:?}"
    );
    assert_eq!(
        observations.lock().expect("observation lock").as_slice(),
        [(
            resume_thread,
            vec![
                OwnedValue::Bool(true),
                OwnedValue::Bool(true),
                OwnedValue::i64(77),
            ],
        )]
    );

    runtime
        .call(
            "main",
            CallArgs::new(),
            CallOptions::unbounded().with_task_scope(scope),
        )
        .expect("caller admits a second detached worker");
    let mut cancelled = take_task(&host).into_completion_future();
    let mut context = Context::from_waker(Waker::noop());
    let Poll::Ready(cancelled) = cancelled.as_mut().poll(&mut context) else {
        panic!("second worker completes before queued cancellation");
    };
    let pinned_artifact = Arc::downgrade(cancelled.capsule().artifact());
    drop(runtime);
    assert!(
        pinned_artifact.upgrade().is_some(),
        "queued continuation must pin its exact executable generation"
    );
    assert!(matches!(
        cancelled
            .cancel(crate::task::TaskCancellationReason::HostShutdown)
            .resume(CallArgs::new(), CallOptions::unbounded()),
        crate::task::TaskContinuationOutcome::NotRequested
    ));
    assert!(
        pinned_artifact.upgrade().is_none(),
        "cancelling the queued continuation releases its final generation pin"
    );
    assert_eq!(observations.lock().expect("observation lock").len(), 1);
}

#[test]
fn scoped_task_continuation_receives_structured_worker_error() {
    let observations = Arc::new(Mutex::new(Vec::new()));
    let native_observations = Arc::clone(&observations);
    let engine = Engine::builder()
        .capabilities(vela_common::CapabilitySet::all())
        .register_async_fn(
            crate::native::NativeFunctionDesc::new(
                "test::fail_task",
                vela_def::FunctionId::new(0x7A54),
            )
            .returns(crate::native::TypeHint::i64())
            .access(crate::native::FunctionAccess::public()),
            |_args| {
                Box::pin(async {
                    Err(VmError::new(VmErrorKind::TypeMismatch {
                        operation: "task failure fixture",
                    }))
                })
            },
        )
        .register_native_fn(
            crate::native::NativeFunctionDesc::new(
                "test::observe_failure",
                vela_def::FunctionId::new(0x7A55),
            )
            .param("is_err", crate::native::TypeHint::boolean())
            .param("turn", crate::native::TypeHint::i64())
            .returns(crate::native::TypeHint::unit())
            .effects(crate::native::EffectSet::host_write())
            .access(crate::native::FunctionAccess::public()),
            move |args| {
                native_observations
                    .lock()
                    .expect("failure observation lock")
                    .push(args.to_vec());
                Ok(OwnedValue::Unit)
            },
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            r#"
async fn repair() -> i64 { return test::fail_task().await; }
fn finish(outcome: Result<i64, task::Error>, turn: i64) {
    test::observe_failure(outcome.is_err(), turn);
}
fn main() { task::spawn_scoped_then(repair(), finish); }
"#,
        )
        .expect("failure continuation fixture compiles");
    let mut runtime = Runtime::new_compiled(engine, program).expect("runtime builds");
    let host = Arc::new(RecordingTaskHost::default());
    runtime
        .call(
            "main",
            CallArgs::new(),
            CallOptions::unbounded().with_task_scope(crate::task::TaskScope::new(
                host.clone(),
                finite_task_policy_with_capabilities(
                    std::time::Duration::from_secs(1),
                    16,
                    vela_common::CapabilitySet::all(),
                ),
            )),
        )
        .expect("caller admits failing worker");

    let mut completion = take_task(&host).into_completion_future();
    let mut context = Context::from_waker(Waker::noop());
    let Poll::Ready(completion) = completion.as_mut().poll(&mut context) else {
        panic!("failing worker should publish completion");
    };
    assert!(matches!(
        completion.outcome(),
        crate::task::ScopedTaskOutcome::Failed(crate::task::TaskError {
            kind: crate::task::TaskErrorKind::WorkerError,
            ..
        })
    ));
    let resumed = completion.resume(
        CallArgs::new().with_value("turn", 88_i64),
        CallOptions::unbounded(),
    );
    assert!(
        matches!(resumed, crate::task::TaskContinuationOutcome::Completed),
        "unexpected failure continuation result: {resumed:?}"
    );
    assert_eq!(
        observations
            .lock()
            .expect("failure observation lock")
            .as_slice(),
        [vec![OwnedValue::Bool(true), OwnedValue::i64(88)]]
    );
}

#[test]
fn scoped_task_admission_includes_continuation_effects() {
    let calls = Arc::new(AtomicUsize::new(0));
    let native_calls = Arc::clone(&calls);
    let engine = Engine::builder()
        .capability(vela_common::Capability::TaskSpawn)
        .capability(vela_common::Capability::HostWrite)
        .register_native_fn(
            crate::native::NativeFunctionDesc::new(
                "test::continuation_write",
                vela_def::FunctionId::new(0x7A56),
            )
            .returns(crate::native::TypeHint::unit())
            .effects(crate::native::EffectSet::host_write())
            .access(crate::native::FunctionAccess::public()),
            move |_args| {
                native_calls.fetch_add(1, Ordering::SeqCst);
                Ok(OwnedValue::Unit)
            },
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            r#"
async fn repair() -> i64 { return 1; }
fn write_helper() { test::continuation_write(); }
fn finish(outcome: Result<i64, task::Error>) {
    write_helper();
}
fn main() { task::spawn_scoped_then(repair(), finish); }
"#,
        )
        .expect("continuation effect fixture compiles");
    let mut runtime = Runtime::new_compiled(engine, program).expect("runtime builds");
    let host = Arc::new(RecordingTaskHost::default());

    let error = runtime
        .call(
            "main",
            CallArgs::new(),
            CallOptions::unbounded().with_task_scope(crate::task::TaskScope::new(
                host.clone(),
                finite_task_policy(std::time::Duration::from_secs(1)),
            )),
        )
        .expect_err("continuation host write exceeds task scope policy");
    assert!(
        matches!(error.kind(), VmErrorKind::TaskAdmissionDenied { .. }),
        "unexpected admission failure: {error:?}"
    );
    assert!(host.admitted.lock().expect("task host lock").is_empty());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
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
fn request_scope_shutdown_cancels_pending_work_before_context_reclamation() {
    let dropped = Arc::new(AtomicBool::new(false));
    let fixture_dropped = Arc::clone(&dropped);
    let (engine, program) = pending_task_fixture(move |_args| {
        Box::pin(NeverReady {
            dropped: Arc::clone(&fixture_dropped),
        })
    });
    let mut runtime = Runtime::new_compiled(engine, program).expect("runtime builds");
    let host = Arc::new(RequestLifecycleHost::default());
    runtime
        .call(
            "main",
            CallArgs::new(),
            CallOptions::unbounded().with_task_scope(crate::task::TaskScope::new(
                host.clone(),
                finite_task_policy(std::time::Duration::from_secs(1)),
            )),
        )
        .expect("request admits pending worker");

    assert!(!host.poll_one());
    assert!(!dropped.load(Ordering::SeqCst));
    host.shutdown();
    assert!(dropped.load(Ordering::SeqCst));
    assert_eq!(host.counts(), (0, 0, 1));
    let state = host.state.lock().expect("request lifecycle lock");
    assert!(matches!(
        state.terminal_errors[0].kind,
        crate::task::TaskErrorKind::Cancelled(crate::task::TaskCancellationReason::HostShutdown)
    ));
}

#[test]
fn request_scope_completion_shutdown_race_has_one_terminal_outcome() {
    let engine = Engine::builder()
        .capability(vela_common::Capability::TaskSpawn)
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            r#"
async fn repair() -> i64 { return 42; }
fn finish(outcome: Result<i64, task::Error>) { return; }
fn main() { task::spawn_scoped_then(repair(), finish); }
"#,
        )
        .expect("request race fixture compiles");
    let mut runtime = Runtime::new_compiled(engine, program).expect("runtime builds");
    let host = Arc::new(RequestLifecycleHost::default());
    runtime
        .call(
            "main",
            CallArgs::new(),
            CallOptions::unbounded().with_task_scope(crate::task::TaskScope::new(
                host.clone(),
                finite_task_policy(std::time::Duration::from_secs(1)),
            )),
        )
        .expect("request admits worker");

    let barrier = Arc::new(std::sync::Barrier::new(3));
    let poll_host = Arc::clone(&host);
    let poll_barrier = Arc::clone(&barrier);
    let poller = std::thread::spawn(move || {
        poll_barrier.wait();
        poll_host.poll_one();
    });
    let shutdown_host = Arc::clone(&host);
    let shutdown_barrier = Arc::clone(&barrier);
    let shutdown = std::thread::spawn(move || {
        shutdown_barrier.wait();
        shutdown_host.shutdown();
    });
    barrier.wait();
    poller.join().expect("completion racer");
    shutdown.join().expect("shutdown racer");

    assert_eq!(host.counts(), (0, 0, 1));
    host.shutdown();
    assert_eq!(host.counts(), (0, 0, 1));
}

#[test]
fn request_scope_bounds_its_completion_queue() {
    let engine = Engine::builder()
        .capability(vela_common::Capability::TaskSpawn)
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            r#"
async fn repair(value: i64) -> i64 { return value; }
fn main() {
    task::spawn_scoped(repair(1));
    task::spawn_scoped(repair(2));
}
"#,
        )
        .expect("bounded completion fixture compiles");
    let mut runtime = Runtime::new_compiled(engine, program).expect("runtime builds");
    let host = Arc::new(RequestLifecycleHost::default());
    let base = finite_task_policy(std::time::Duration::from_secs(1));
    let policy = crate::task::TaskPolicy::new(
        base.max_active_tasks(),
        std::num::NonZeroUsize::MIN,
        base.child_execution_limits(),
        base.max_host_calls(),
        base.timeout(),
        base.capabilities(),
    )
    .expect("one-slot completion policy");
    runtime
        .call(
            "main",
            CallArgs::new(),
            CallOptions::unbounded()
                .with_task_scope(crate::task::TaskScope::new(host.clone(), policy)),
        )
        .expect("request admits two workers");

    assert!(host.poll_one());
    assert!(host.poll_one());
    assert_eq!(host.counts(), (0, 1, 1));
    let state = host.state.lock().expect("request lifecycle lock");
    assert!(matches!(
        state.terminal_errors[0].kind,
        crate::task::TaskErrorKind::Cancelled(
            crate::task::TaskCancellationReason::CompletionQueueFull
        )
    ));
    drop(state);
    host.shutdown();
    assert_eq!(host.counts(), (0, 0, 2));
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
