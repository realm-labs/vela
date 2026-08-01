use std::collections::BTreeSet;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use vela_common::{Capability, CapabilitySet};
use vela_def::FunctionId;
use vela_engine::engine::Engine;
use vela_engine::native::{FunctionAccess, NativeFunctionDesc, TypeHint};
use vela_engine::runtime::{CallArgs, CallOptions, Runtime};
use vela_engine::task::{
    ScopedTask, ScopedTaskHost, ScopedTaskOutcome, TaskAdmissionError, TaskErrorKind, TaskPolicy,
    TaskScope,
};
use vela_vm::budget::{CollectionLimits, ExecutionLimits};
use vela_vm::owned_value::OwnedValue;

const TASK_COUNT: usize = 32;

#[derive(Default)]
struct BoundedTaskHost {
    active: AtomicUsize,
    tasks: Mutex<Vec<ScopedTask>>,
}

impl ScopedTaskHost for BoundedTaskHost {
    fn admit(&self, task: ScopedTask) -> Result<(), TaskAdmissionError> {
        let maximum = task.capsule().policy().max_active_tasks().get();
        let mut tasks = self.tasks.lock().expect("bounded task host lock");
        if self.active.load(Ordering::Acquire) >= maximum {
            return Err(TaskAdmissionError::CapacityExceeded { maximum });
        }
        self.active.fetch_add(1, Ordering::AcqRel);
        tasks.push(task);
        Ok(())
    }
}

impl BoundedTaskHost {
    fn take_all(&self) -> Vec<ScopedTask> {
        std::mem::take(&mut *self.tasks.lock().expect("bounded task host lock"))
    }

    fn take_one(&self) -> ScopedTask {
        self.tasks
            .lock()
            .expect("bounded task host lock")
            .pop()
            .expect("one admitted task")
    }

    fn release_slot(&self) {
        let previous = self.active.fetch_sub(1, Ordering::AcqRel);
        assert_ne!(previous, 0, "task slot count cannot underflow");
    }
}

struct YieldOnce {
    yielded: bool,
}

impl std::future::Future for YieldOnce {
    type Output = vela_vm::error::VmResult<OwnedValue>;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        if self.yielded {
            Poll::Ready(Ok(OwnedValue::Unit))
        } else {
            self.yielded = true;
            context.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

fn engine() -> Engine {
    Engine::builder()
        .capability(Capability::TaskSpawn)
        .register_async_fn(
            NativeFunctionDesc::new("stress::yield_once", FunctionId::new(0x57_0001))
                .returns(TypeHint::unit())
                .access(FunctionAccess::public()),
            |_args| Box::pin(YieldOnce { yielded: false }),
        )
        .register_async_fn(
            NativeFunctionDesc::new("stress::panic_worker", FunctionId::new(0x57_0002))
                .returns(TypeHint::i64())
                .access(FunctionAccess::public()),
            |_args| {
                Box::pin(async {
                    panic!("detached stress worker panic");
                })
            },
        )
        .build()
        .expect("stress engine")
}

fn policy(maximum: usize) -> TaskPolicy {
    TaskPolicy::new(
        std::num::NonZeroUsize::new(maximum).expect("positive task capacity"),
        std::num::NonZeroUsize::new(maximum).expect("positive completion capacity"),
        ExecutionLimits::new(100_000, 1 << 20, 64).with_collection_limits(CollectionLimits {
            max_array_len: 1_024,
            max_map_entries: 1_024,
            max_set_len: 1_024,
        }),
        std::num::NonZeroU64::new(64).expect("positive host call budget"),
        Duration::from_secs(5),
        CapabilitySet::new().with(Capability::TaskSpawn),
    )
    .expect("finite stress task policy")
}

fn poll_twice(mut task: ScopedTask) -> ScopedTaskOutcome {
    let mut context = Context::from_waker(Waker::noop());
    assert!(task.poll(&mut context).is_pending());
    let Poll::Ready(outcome) = task.poll(&mut context) else {
        panic!("yield-once task should finish on its second poll");
    };
    outcome
}

#[test]
fn concurrent_tasks_share_artifact_and_cleanup_every_terminal_path() {
    let engine = engine();
    let artifact = engine
        .link_compiled_program(
            engine
                .compile_source(
                    r#"
async fn normal_worker(value: i64) -> i64 {
    stress::yield_once().await;
    return value + 1;
}
async fn panic_worker() -> i64 {
    stress::yield_once().await;
    return stress::panic_worker().await;
}
fn spawn_normal(value: i64) { task::spawn_scoped(normal_worker(value)); }
fn spawn_panic() { task::spawn_scoped(panic_worker()); }
"#,
                )
                .expect("stress task source compiles"),
        )
        .expect("stress task source links");
    let host = Arc::new(BoundedTaskHost::default());
    let scope = TaskScope::new(host.clone(), policy(TASK_COUNT));

    let joins = (0..TASK_COUNT)
        .map(|index| {
            let engine = engine.clone();
            let artifact = artifact.clone();
            let scope = scope.clone();
            std::thread::spawn(move || {
                let mut runtime = Runtime::from_linked_artifact(engine, artifact)
                    .expect("concurrent runtime builds");
                let (entry, args) = if index < 4 {
                    ("spawn_panic", CallArgs::new())
                } else {
                    (
                        "spawn_normal",
                        CallArgs::from_positional([OwnedValue::i64(index as i64)]),
                    )
                };
                runtime
                    .call(entry, args, CallOptions::unbounded().with_task_scope(scope))
                    .expect("concurrent parent admits task");
            })
        })
        .collect::<Vec<_>>();
    for join in joins {
        join.join().expect("concurrent parent thread");
    }

    let mut capacity_runtime = Runtime::from_linked_artifact(engine.clone(), artifact.clone())
        .expect("capacity runtime builds");
    let error = capacity_runtime
        .call(
            "spawn_normal",
            CallArgs::from_positional([OwnedValue::i64(99)]),
            CallOptions::unbounded().with_task_scope(scope.clone()),
        )
        .expect_err("bounded host rejects the extra task");
    assert!(
        matches!(
            error.kind(),
            vela_vm::error::VmErrorKind::TaskAdmissionDenied { .. }
        ),
        "{error:?}"
    );
    drop(capacity_runtime);

    let tasks = host.take_all();
    assert_eq!(tasks.len(), TASK_COUNT);
    assert!(
        tasks
            .iter()
            .all(|task| Arc::ptr_eq(task.capsule().artifact(), &artifact)),
        "every child must reuse the exact linked artifact/cache owner"
    );
    let ids = tasks
        .iter()
        .map(|task| task.metadata().task_id.get())
        .collect::<BTreeSet<_>>();
    assert_eq!(ids.len(), TASK_COUNT);

    let (panicking, mut normal): (Vec<_>, Vec<_>) = tasks
        .into_iter()
        .partition(|task| task.metadata().worker_debug_name == "panic_worker");
    assert_eq!(panicking.len(), 4);
    normal.sort_by_key(|task| task.metadata().task_id);
    let dropped = normal.drain(..4).collect::<Vec<_>>();
    let cancelled = normal.drain(..4).collect::<Vec<_>>();
    drop(dropped);
    for task in cancelled {
        let _ = task.cancel(vela_engine::task::TaskCancellationReason::HostShutdown);
    }
    for _ in 0..8 {
        host.release_slot();
    }

    let joins = normal
        .into_iter()
        .chain(panicking)
        .map(|task| std::thread::spawn(move || poll_twice(task)))
        .collect::<Vec<_>>();
    let mut completed = 0;
    let mut panicked = 0;
    for join in joins {
        match join.join().expect("worker polling thread") {
            ScopedTaskOutcome::Completed(_) => completed += 1,
            ScopedTaskOutcome::Failed(error) if error.kind == TaskErrorKind::WorkerPanicked => {
                panicked += 1;
            }
            outcome => panic!("unexpected stress task outcome: {outcome:?}"),
        }
        host.release_slot();
    }

    assert_eq!(completed, 20);
    assert_eq!(panicked, 4);
    assert_eq!(host.active.load(Ordering::Acquire), 0);
    let metrics = scope.metrics();
    assert_eq!(metrics.admission_attempts, 33);
    assert_eq!(metrics.admitted, 32);
    assert_eq!(metrics.admission_rejections, 1);
    assert_eq!(metrics.active, 0);
    assert_eq!(metrics.peak_active, 32);
    assert_eq!(metrics.worker_pending_polls, 24);
    assert_eq!(metrics.workers_completed, 20);
    assert_eq!(metrics.workers_failed, 8);
    assert_eq!(metrics.workers_cancelled, 4);
    assert_eq!(metrics.workers_dropped, 4);
    let artifact_lifetime = Arc::downgrade(&artifact);
    drop(scope);
    assert_eq!(Arc::strong_count(&artifact), 1);
    drop(artifact);
    assert!(
        artifact_lifetime.upgrade().is_none(),
        "scope teardown must drain every pooled Runtime and artifact pin"
    );
}

#[test]
fn recursive_spawn_exhausts_the_same_bounded_scope_quota() {
    let engine = engine();
    let artifact = engine
        .link_compiled_program(
            engine
                .compile_source(
                    r#"
async fn child() {}
async fn parent() { task::spawn_scoped(child()); }
fn main() { task::spawn_scoped(parent()); }
"#,
                )
                .expect("recursive task source compiles"),
        )
        .expect("recursive task source links");
    let host = Arc::new(BoundedTaskHost::default());
    let scope = TaskScope::new(host.clone(), policy(1));
    let mut runtime = Runtime::from_linked_artifact(engine, artifact).expect("runtime builds");
    runtime
        .call(
            "main",
            CallArgs::new(),
            CallOptions::unbounded().with_task_scope(scope.clone()),
        )
        .expect("outer task is admitted");

    let mut parent = host.take_one();
    let mut context = Context::from_waker(Waker::noop());
    let Poll::Ready(ScopedTaskOutcome::Failed(error)) = parent.poll(&mut context) else {
        panic!("recursive admission should fail the parent worker");
    };
    assert_eq!(error.kind, TaskErrorKind::WorkerError);
    drop(parent);
    host.release_slot();
    assert!(
        host.tasks
            .lock()
            .expect("bounded task host lock")
            .is_empty()
    );

    let metrics = scope.metrics();
    assert_eq!(metrics.admission_attempts, 2);
    assert_eq!(metrics.admitted, 1);
    assert_eq!(metrics.admission_rejections, 1);
    assert_eq!(metrics.active, 0);
    assert_eq!(metrics.peak_active, 1);
    assert_eq!(metrics.workers_failed, 1);
}
