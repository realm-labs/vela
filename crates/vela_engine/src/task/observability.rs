use std::num::NonZeroU64;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use vela_common::Span;
use vela_def::FunctionId;

use super::{
    TaskAdmissionError, TaskCancellationReason, TaskErrorKind, TaskGeneration, TaskMetadata,
};

/// Scope-local identity used only by host diagnostics and tracing.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskId(NonZeroU64);

impl TaskId {
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

/// Structured host lifecycle event. It grants no execution authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskEvent {
    pub task_id: TaskId,
    pub caller: FunctionId,
    pub worker: FunctionId,
    pub worker_debug_name: String,
    pub continuation: Option<FunctionId>,
    pub source_span: Option<Span>,
    pub generation: TaskGeneration,
    pub kind: TaskEventKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TaskEventKind {
    AdmissionAttempt,
    Admitted,
    AdmissionRejected(TaskAdmissionError),
    WorkerPending { polls: u64 },
    WorkerCompleted,
    WorkerFailed(TaskErrorKind),
    WorkerDropped,
    ContinuationCompleted,
    ContinuationFailed(TaskErrorKind),
    ContinuationSuppressed,
}

/// Optional host sink for tracing task lifecycle events.
pub trait TaskObserver: Send + Sync {
    fn observe(&self, event: &TaskEvent);
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TaskMetricsSnapshot {
    pub admission_attempts: u64,
    pub admitted: u64,
    pub admission_rejections: u64,
    pub active: u64,
    pub peak_active: u64,
    pub worker_pending_polls: u64,
    pub workers_completed: u64,
    pub workers_failed: u64,
    pub workers_cancelled: u64,
    pub workers_dropped: u64,
    pub continuations_completed: u64,
    pub continuations_failed: u64,
    pub continuations_suppressed: u64,
    pub runtime_pool_hits: u64,
    pub runtime_pool_misses: u64,
    pub runtime_pool_returns: u64,
    pub runtime_pool_discards: u64,
}

#[derive(Default)]
struct TaskMetrics {
    admission_attempts: AtomicU64,
    admitted: AtomicU64,
    admission_rejections: AtomicU64,
    active: AtomicU64,
    peak_active: AtomicU64,
    worker_pending_polls: AtomicU64,
    workers_completed: AtomicU64,
    workers_failed: AtomicU64,
    workers_cancelled: AtomicU64,
    workers_dropped: AtomicU64,
    continuations_completed: AtomicU64,
    continuations_failed: AtomicU64,
    continuations_suppressed: AtomicU64,
}

pub(super) struct TaskObservability {
    next_id: Mutex<u64>,
    observer: Option<Arc<dyn TaskObserver>>,
    metrics: TaskMetrics,
}

impl Default for TaskObservability {
    fn default() -> Self {
        Self::new(None)
    }
}

impl TaskObservability {
    pub(super) fn new(observer: Option<Arc<dyn TaskObserver>>) -> Self {
        Self {
            next_id: Mutex::new(1),
            observer,
            metrics: TaskMetrics::default(),
        }
    }

    pub(super) fn allocate_task_id(&self) -> Option<TaskId> {
        let mut next = self.next_id.lock().expect("task ID allocator lock");
        let id = NonZeroU64::new(*next)?;
        *next = next.checked_add(1)?;
        Some(TaskId(id))
    }

    pub(super) fn begin_task(self: &Arc<Self>, metadata: &TaskMetadata) -> TaskTelemetry {
        let telemetry = TaskTelemetry {
            inner: Arc::new(TaskTelemetryInner {
                observability: Arc::clone(self),
                context: TaskEventContext::from(metadata),
                state: Mutex::new(TaskTelemetryState::default()),
            }),
        };
        telemetry.emit(TaskEventKind::AdmissionAttempt);
        telemetry
    }

    pub(super) fn metrics(&self) -> TaskMetricsSnapshot {
        TaskMetricsSnapshot {
            admission_attempts: load(&self.metrics.admission_attempts),
            admitted: load(&self.metrics.admitted),
            admission_rejections: load(&self.metrics.admission_rejections),
            active: load(&self.metrics.active),
            peak_active: load(&self.metrics.peak_active),
            worker_pending_polls: load(&self.metrics.worker_pending_polls),
            workers_completed: load(&self.metrics.workers_completed),
            workers_failed: load(&self.metrics.workers_failed),
            workers_cancelled: load(&self.metrics.workers_cancelled),
            workers_dropped: load(&self.metrics.workers_dropped),
            continuations_completed: load(&self.metrics.continuations_completed),
            continuations_failed: load(&self.metrics.continuations_failed),
            continuations_suppressed: load(&self.metrics.continuations_suppressed),
            runtime_pool_hits: 0,
            runtime_pool_misses: 0,
            runtime_pool_returns: 0,
            runtime_pool_discards: 0,
        }
    }

    fn emit(&self, context: &TaskEventContext, kind: TaskEventKind) {
        self.record_metrics(&kind);
        let Some(observer) = &self.observer else {
            return;
        };
        let event = context.event(kind);
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            observer.observe(&event);
        }));
    }

    fn record_metrics(&self, kind: &TaskEventKind) {
        let metrics = &self.metrics;
        match kind {
            TaskEventKind::AdmissionAttempt => {
                increment(&metrics.admission_attempts, 1);
            }
            TaskEventKind::Admitted => {
                increment(&metrics.admitted, 1);
            }
            TaskEventKind::AdmissionRejected(_) => {
                increment(&metrics.admission_rejections, 1);
            }
            TaskEventKind::WorkerPending { polls } => {
                increment(&metrics.worker_pending_polls, *polls);
            }
            TaskEventKind::WorkerCompleted => {
                increment(&metrics.workers_completed, 1);
            }
            TaskEventKind::WorkerFailed(kind) => {
                increment(&metrics.workers_failed, 1);
                if matches!(kind, TaskErrorKind::Cancelled(_)) {
                    increment(&metrics.workers_cancelled, 1);
                }
            }
            TaskEventKind::WorkerDropped => {
                increment(&metrics.workers_dropped, 1);
            }
            TaskEventKind::ContinuationCompleted => {
                increment(&metrics.continuations_completed, 1);
            }
            TaskEventKind::ContinuationFailed(_) => {
                increment(&metrics.continuations_failed, 1);
            }
            TaskEventKind::ContinuationSuppressed => {
                increment(&metrics.continuations_suppressed, 1);
            }
        }
    }

    fn activate(&self) {
        let active = increment(&self.metrics.active, 1);
        let mut peak = self.metrics.peak_active.load(Ordering::Relaxed);
        while active > peak {
            match self.metrics.peak_active.compare_exchange_weak(
                peak,
                active,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(observed) => peak = observed,
            }
        }
    }

    fn deactivate(&self) {
        let _ = self
            .metrics
            .active
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                Some(value.saturating_sub(1))
            });
    }
}

#[derive(Clone)]
pub struct TaskTelemetry {
    inner: Arc<TaskTelemetryInner>,
}

impl std::fmt::Debug for TaskTelemetry {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TaskTelemetry")
            .field("task_id", &self.inner.context.task_id)
            .finish_non_exhaustive()
    }
}

struct TaskTelemetryInner {
    observability: Arc<TaskObservability>,
    context: TaskEventContext,
    state: Mutex<TaskTelemetryState>,
}

#[derive(Default)]
struct TaskTelemetryState {
    admission: AdmissionState,
    pending_polls_before_admission: u64,
    pending_worker_terminal: Option<TaskEventKind>,
    worker_terminal: bool,
    continuation_terminal: bool,
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
enum AdmissionState {
    #[default]
    Attempt,
    Admitted,
    Rejected,
}

impl TaskTelemetry {
    pub(crate) fn admitted(&self) {
        let (pending_polls, pending_terminal) = {
            let mut state = self.inner.state.lock().expect("task telemetry lock");
            if state.admission != AdmissionState::Attempt {
                return;
            }
            state.admission = AdmissionState::Admitted;
            let pending_polls = std::mem::take(&mut state.pending_polls_before_admission);
            let terminal = state.pending_worker_terminal.take();
            if terminal.is_none() {
                self.inner.observability.activate();
            }
            (pending_polls, terminal)
        };
        self.emit(TaskEventKind::Admitted);
        if pending_polls != 0 {
            self.emit(TaskEventKind::WorkerPending {
                polls: pending_polls,
            });
        }
        if let Some(kind) = pending_terminal {
            self.emit(kind);
        }
    }

    pub(crate) fn rejected(&self, error: TaskAdmissionError) {
        let mut state = self.inner.state.lock().expect("task telemetry lock");
        if state.admission != AdmissionState::Attempt {
            return;
        }
        state.admission = AdmissionState::Rejected;
        state.pending_worker_terminal = None;
        state.pending_polls_before_admission = 0;
        drop(state);
        self.emit(TaskEventKind::AdmissionRejected(error));
    }

    pub(crate) fn worker_pending(&self) {
        let mut state = self.inner.state.lock().expect("task telemetry lock");
        match state.admission {
            AdmissionState::Attempt => {
                state.pending_polls_before_admission =
                    state.pending_polls_before_admission.saturating_add(1);
            }
            AdmissionState::Admitted if !state.worker_terminal => {
                drop(state);
                self.emit(TaskEventKind::WorkerPending { polls: 1 });
            }
            AdmissionState::Admitted | AdmissionState::Rejected => {}
        }
    }

    pub(crate) fn worker_completed(&self) {
        self.worker_terminal(TaskEventKind::WorkerCompleted);
    }

    pub(crate) fn worker_failed(&self, kind: TaskErrorKind) {
        self.worker_terminal(TaskEventKind::WorkerFailed(kind));
    }

    pub(crate) fn worker_cancelled(&self, reason: TaskCancellationReason) {
        self.worker_failed(TaskErrorKind::Cancelled(reason));
    }

    pub(crate) fn worker_dropped(&self) {
        self.worker_terminal(TaskEventKind::WorkerDropped);
    }

    pub(crate) fn continuation_completed(&self) {
        self.continuation_terminal(TaskEventKind::ContinuationCompleted);
    }

    pub(crate) fn continuation_failed(&self, kind: TaskErrorKind) {
        self.continuation_terminal(TaskEventKind::ContinuationFailed(kind));
    }

    pub(crate) fn continuation_suppressed(&self) {
        self.continuation_terminal(TaskEventKind::ContinuationSuppressed);
    }

    fn worker_terminal(&self, kind: TaskEventKind) {
        let mut state = self.inner.state.lock().expect("task telemetry lock");
        if state.worker_terminal || state.admission == AdmissionState::Rejected {
            return;
        }
        state.worker_terminal = true;
        if state.admission == AdmissionState::Attempt {
            state.pending_worker_terminal = Some(kind);
            return;
        }
        self.inner.observability.deactivate();
        drop(state);
        self.emit(kind);
    }

    fn continuation_terminal(&self, kind: TaskEventKind) {
        let mut state = self.inner.state.lock().expect("task telemetry lock");
        if state.continuation_terminal || state.admission == AdmissionState::Rejected {
            return;
        }
        state.continuation_terminal = true;
        drop(state);
        self.emit(kind);
    }

    fn emit(&self, kind: TaskEventKind) {
        self.inner.observability.emit(&self.inner.context, kind);
    }
}

#[derive(Clone)]
struct TaskEventContext {
    task_id: TaskId,
    caller: FunctionId,
    worker: FunctionId,
    worker_debug_name: String,
    continuation: Option<FunctionId>,
    source_span: Option<Span>,
    generation: TaskGeneration,
}

impl From<&TaskMetadata> for TaskEventContext {
    fn from(metadata: &TaskMetadata) -> Self {
        Self {
            task_id: metadata.task_id,
            caller: metadata.caller,
            worker: metadata.worker,
            worker_debug_name: metadata.worker_debug_name.clone(),
            continuation: metadata
                .continuation
                .as_ref()
                .map(|continuation| continuation.function),
            source_span: metadata.source_span,
            generation: metadata.generation,
        }
    }
}

impl TaskEventContext {
    fn event(&self, kind: TaskEventKind) -> TaskEvent {
        TaskEvent {
            task_id: self.task_id,
            caller: self.caller,
            worker: self.worker,
            worker_debug_name: self.worker_debug_name.clone(),
            continuation: self.continuation,
            source_span: self.source_span,
            generation: self.generation,
            kind,
        }
    }
}

fn load(counter: &AtomicU64) -> u64 {
    counter.load(Ordering::Relaxed)
}

fn increment(counter: &AtomicU64, amount: u64) -> u64 {
    counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            Some(value.saturating_add(amount))
        })
        .unwrap_or_else(|value| value)
        .saturating_add(amount)
}
