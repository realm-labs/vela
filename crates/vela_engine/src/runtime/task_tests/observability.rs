use super::*;

#[derive(Default)]
struct RecordingObserver {
    events: Mutex<Vec<crate::task::TaskEvent>>,
}

impl crate::task::TaskObserver for RecordingObserver {
    fn observe(&self, event: &crate::task::TaskEvent) {
        self.events
            .lock()
            .expect("task observer lock")
            .push(event.clone());
    }
}

struct RejectingTaskHost;

impl crate::task::ScopedTaskHost for RejectingTaskHost {
    fn admit(&self, _task: crate::task::ScopedTask) -> Result<(), crate::task::TaskAdmissionError> {
        Err(crate::task::TaskAdmissionError::CapacityExceeded { maximum: 1 })
    }
}

struct PanickingObserver;

impl crate::task::TaskObserver for PanickingObserver {
    fn observe(&self, _event: &crate::task::TaskEvent) {
        panic!("observer failure must not cross the task boundary");
    }
}

#[test]
fn task_scope_traces_unique_ids_and_terminal_metrics() {
    let engine = Engine::builder()
        .capability(vela_common::Capability::TaskSpawn)
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            r#"
async fn repair(value: i64) -> i64 { return value + 1; }
fn finish(outcome: Result<i64, task::Error>) {}
fn main(value: i64) { task::spawn_scoped_then(repair(value), finish); }
"#,
        )
        .expect("task telemetry fixture compiles");
    let mut runtime = Runtime::new_compiled(engine, program).expect("runtime builds");
    let host = Arc::new(RecordingTaskHost::default());
    let observer = Arc::new(RecordingObserver::default());
    let scope = crate::task::TaskScope::new(
        host.clone(),
        finite_task_policy(std::time::Duration::from_secs(1)),
    )
    .with_observer(observer.clone());

    runtime
        .call(
            "main",
            CallArgs::from_positional([OwnedValue::i64(40)]),
            CallOptions::unbounded().with_task_scope(scope.clone()),
        )
        .expect("caller admits observed worker");
    let metrics = scope.metrics();
    assert_eq!(metrics.admission_attempts, 1);
    assert_eq!(metrics.admitted, 1);
    assert_eq!(metrics.active, 1);
    assert_eq!(metrics.peak_active, 1);

    let mut task = take_task(&host);
    assert_eq!(task.metadata().task_id.get(), 1);
    let mut context = Context::from_waker(Waker::noop());
    let Poll::Ready(completion) = task.poll_completion(&mut context) else {
        panic!("observed worker should complete");
    };
    drop(task);
    assert!(matches!(
        completion.resume(CallArgs::new(), CallOptions::unbounded()),
        crate::task::TaskContinuationOutcome::Completed
    ));

    runtime
        .call(
            "main",
            CallArgs::from_positional([OwnedValue::i64(41)]),
            CallOptions::unbounded().with_task_scope(scope.clone()),
        )
        .expect("caller admits second observed worker");
    let task = take_task(&host);
    assert_eq!(task.metadata().task_id.get(), 2);
    let _ = task.cancel(crate::task::TaskCancellationReason::HostShutdown);

    let metrics = scope.metrics();
    assert_eq!(metrics.admission_attempts, 2);
    assert_eq!(metrics.admitted, 2);
    assert_eq!(metrics.active, 0);
    assert_eq!(metrics.workers_completed, 1);
    assert_eq!(metrics.workers_failed, 1);
    assert_eq!(metrics.workers_cancelled, 1);
    assert_eq!(metrics.workers_dropped, 0);
    assert_eq!(metrics.continuations_completed, 1);

    let events = observer.events.lock().expect("task observer lock");
    let first = events
        .iter()
        .filter(|event| event.task_id.get() == 1)
        .map(|event| event.kind.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        first,
        [
            crate::task::TaskEventKind::AdmissionAttempt,
            crate::task::TaskEventKind::Admitted,
            crate::task::TaskEventKind::WorkerCompleted,
            crate::task::TaskEventKind::ContinuationCompleted,
        ]
    );
    assert!(
        events
            .iter()
            .all(|event| event.worker_debug_name == "repair")
    );
}

#[test]
fn rejected_admission_discards_precommit_drop_telemetry() {
    let engine = Engine::builder()
        .capability(vela_common::Capability::TaskSpawn)
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source("async fn repair() {} fn main() { task::spawn_scoped(repair()); }")
        .expect("task rejection fixture compiles");
    let mut runtime = Runtime::new_compiled(engine, program).expect("runtime builds");
    let observer = Arc::new(RecordingObserver::default());
    let scope = crate::task::TaskScope::new(
        Arc::new(RejectingTaskHost),
        finite_task_policy(std::time::Duration::from_secs(1)),
    )
    .with_observer(observer.clone());

    let error = runtime
        .call(
            "main",
            CallArgs::new(),
            CallOptions::unbounded().with_task_scope(scope.clone()),
        )
        .expect_err("host rejects task admission");
    assert!(matches!(
        error.kind(),
        VmErrorKind::TaskAdmissionDenied { .. }
    ));
    let metrics = scope.metrics();
    assert_eq!(metrics.admission_attempts, 1);
    assert_eq!(metrics.admitted, 0);
    assert_eq!(metrics.admission_rejections, 1);
    assert_eq!(metrics.active, 0);
    assert_eq!(metrics.workers_dropped, 0);
    assert_eq!(
        observer
            .events
            .lock()
            .expect("task observer lock")
            .iter()
            .map(|event| event.kind.clone())
            .collect::<Vec<_>>(),
        [
            crate::task::TaskEventKind::AdmissionAttempt,
            crate::task::TaskEventKind::AdmissionRejected(
                crate::task::TaskAdmissionError::CapacityExceeded { maximum: 1 }
            ),
        ]
    );
}

#[test]
fn observer_panics_do_not_change_task_execution_or_metrics() {
    let engine = Engine::builder()
        .capability(vela_common::Capability::TaskSpawn)
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            "async fn repair() -> i64 { return 42; } fn main() { task::spawn_scoped(repair()); }",
        )
        .expect("observer containment fixture compiles");
    let mut runtime = Runtime::new_compiled(engine, program).expect("runtime builds");
    let host = Arc::new(RecordingTaskHost::default());
    let scope = crate::task::TaskScope::new(
        host.clone(),
        finite_task_policy(std::time::Duration::from_secs(1)),
    )
    .with_observer(Arc::new(PanickingObserver));

    runtime
        .call(
            "main",
            CallArgs::new(),
            CallOptions::unbounded().with_task_scope(scope.clone()),
        )
        .expect("observer panic does not reject admission");
    let mut task = take_task(&host);
    let mut context = Context::from_waker(Waker::noop());
    let Poll::Ready(crate::task::ScopedTaskOutcome::Completed(image)) = task.poll(&mut context)
    else {
        panic!("observer panic does not fail worker execution");
    };
    assert_eq!(import_task_owned_result(&image), OwnedValue::i64(42));
    let metrics = scope.metrics();
    assert_eq!(metrics.admission_attempts, 1);
    assert_eq!(metrics.admitted, 1);
    assert_eq!(metrics.workers_completed, 1);
    assert_eq!(metrics.active, 0);
}

#[test]
fn detached_runtime_pool_reinitializes_vm_state_before_reuse() {
    let engine = Engine::builder()
        .capability(vela_common::Capability::TaskSpawn)
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source(
            r#"
state counter: i64 = 0;
async fn repair() -> i64 { counter += 1; return counter; }
fn main() { task::spawn_scoped(repair()); }
"#,
        )
        .expect("pooled task fixture compiles");
    let mut runtime = Runtime::new_compiled(engine, program).expect("runtime builds");
    let host = Arc::new(RecordingTaskHost::default());
    let scope = crate::task::TaskScope::new(
        host.clone(),
        finite_task_policy(std::time::Duration::from_secs(1)),
    );

    for _ in 0..2 {
        runtime
            .call(
                "main",
                CallArgs::new(),
                CallOptions::unbounded().with_task_scope(scope.clone()),
            )
            .expect("caller admits pooled worker");
        let mut task = take_task(&host);
        let mut context = Context::from_waker(Waker::noop());
        let Poll::Ready(crate::task::ScopedTaskOutcome::Completed(image)) = task.poll(&mut context)
        else {
            panic!("pooled worker should complete");
        };
        assert_eq!(import_task_owned_result(&image), OwnedValue::i64(1));
    }

    let metrics = scope.metrics();
    assert_eq!(metrics.runtime_pool_misses, 1);
    assert_eq!(metrics.runtime_pool_hits, 1);
    assert_eq!(metrics.runtime_pool_returns, 2);
    assert_eq!(metrics.runtime_pool_discards, 0);
    assert_eq!(metrics.active, 0);
}
