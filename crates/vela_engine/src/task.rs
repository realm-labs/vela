//! Executor-neutral host authority for scoped detached Vela work.
//!
//! This module deliberately owns no executor, global task registry, or
//! script-visible task handle. A host lifecycle accepts an owned task, drives
//! its future, and (when requested) schedules the sealed continuation at a
//! later safe point.

use std::fmt;
use std::future::Future;
use std::num::{NonZeroU64, NonZeroUsize};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use vela_bytecode::{ExecutableGenerationId, LinkedArtifact};
use vela_common::{CapabilitySet, ServiceGenerationId, ServiceSetId, Span};
use vela_def::FunctionId;
use vela_vm::DetachedValueImage;
use vela_vm::budget::ExecutionLimits;

use crate::engine::Engine;

mod observability;
mod runtime_pool;

pub(crate) use observability::TaskTelemetry;
pub use observability::{TaskEvent, TaskEventKind, TaskId, TaskMetricsSnapshot, TaskObserver};

/// Future transferred to the host lifecycle after successful admission.
pub type ScopedTaskFuture = Pin<Box<dyn Future<Output = ScopedTaskOutcome> + Send + 'static>>;

/// Host-polled future that retains exact-generation authority in its result.
pub type ScopedTaskCompletionFuture =
    Pin<Box<dyn Future<Output = ScopedTaskCompletion> + Send + 'static>>;

pub(crate) type ScopedWorkerFuture = Pin<
    Box<dyn Future<Output = Result<DetachedValueImage, (TaskErrorKind, String)>> + Send + 'static>,
>;

/// Host-owned lifecycle boundary for detached work.
///
/// Admission transfers the complete future and execution capsule. Returning
/// `Ok(())` means the host has assumed responsibility for polling or dropping
/// the future and for retaining the capsule until that drop completes.
pub trait ScopedTaskHost: Send + Sync {
    fn admit(&self, task: ScopedTask) -> Result<(), TaskAdmissionError>;
}

/// Explicit task authority installed for one Runtime root call.
#[derive(Clone)]
pub struct TaskScope {
    host: Arc<dyn ScopedTaskHost>,
    policy: TaskPolicy,
    observability: Arc<observability::TaskObservability>,
    runtime_pool: runtime_pool::DetachedRuntimePool,
}

impl TaskScope {
    #[must_use]
    pub fn new(host: Arc<dyn ScopedTaskHost>, policy: TaskPolicy) -> Self {
        let runtime_pool = runtime_pool::DetachedRuntimePool::new(policy.max_active_tasks.get());
        Self {
            host,
            policy,
            observability: Arc::new(observability::TaskObservability::default()),
            runtime_pool,
        }
    }

    /// Installs a host-owned sink for structured detached-task lifecycle events.
    #[must_use]
    pub fn with_observer(mut self, observer: Arc<dyn TaskObserver>) -> Self {
        self.observability = Arc::new(observability::TaskObservability::new(Some(observer)));
        self
    }

    #[must_use]
    pub const fn host(&self) -> &Arc<dyn ScopedTaskHost> {
        &self.host
    }

    #[must_use]
    pub const fn policy(&self) -> &TaskPolicy {
        &self.policy
    }

    /// Returns one saturating, lock-free metrics snapshot for this scope.
    #[must_use]
    pub fn metrics(&self) -> TaskMetricsSnapshot {
        let mut metrics = self.observability.metrics();
        let pool = self.runtime_pool.metrics();
        metrics.runtime_pool_hits = pool.hits;
        metrics.runtime_pool_misses = pool.misses;
        metrics.runtime_pool_returns = pool.returns;
        metrics.runtime_pool_discards = pool.discards;
        metrics
    }

    pub(crate) fn allocate_task_id(&self) -> Option<TaskId> {
        self.observability.allocate_task_id()
    }

    pub(crate) fn begin_task(&self, metadata: &TaskMetadata) -> TaskTelemetry {
        self.observability.begin_task(metadata)
    }

    pub(crate) fn runtime_pool(&self) -> runtime_pool::DetachedRuntimePool {
        self.runtime_pool.clone()
    }
}

impl fmt::Debug for TaskScope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TaskScope")
            .field("policy", &self.policy)
            .field("metrics", &self.metrics())
            .finish_non_exhaustive()
    }
}

impl PartialEq for TaskScope {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.host, &other.host)
            && self.policy == other.policy
            && Arc::ptr_eq(&self.observability, &other.observability)
    }
}

impl Eq for TaskScope {}

/// Finite limits and capability ceiling imposed by one host lifecycle scope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskPolicy {
    max_active_tasks: NonZeroUsize,
    max_queued_completions: NonZeroUsize,
    child_execution_limits: ExecutionLimits,
    max_host_calls: NonZeroU64,
    timeout: Duration,
    capabilities: CapabilitySet,
}

impl TaskPolicy {
    pub fn new(
        max_active_tasks: NonZeroUsize,
        max_queued_completions: NonZeroUsize,
        child_execution_limits: ExecutionLimits,
        max_host_calls: NonZeroU64,
        timeout: Duration,
        capabilities: CapabilitySet,
    ) -> Result<Self, TaskPolicyError> {
        if timeout.is_zero() {
            return Err(TaskPolicyError::ZeroTimeout);
        }
        if timeout == Duration::MAX
            || Instant::now().checked_add(timeout).is_none()
            || child_execution_limits.execution_unit_limit == u64::MAX
            || child_execution_limits.memory_limit_bytes == usize::MAX
            || child_execution_limits.max_call_depth == usize::MAX
            || child_execution_limits.collection_limits.max_array_len == usize::MAX
            || child_execution_limits.collection_limits.max_map_entries == usize::MAX
            || child_execution_limits.collection_limits.max_set_len == usize::MAX
        {
            return Err(TaskPolicyError::UnboundedLimit);
        }
        Ok(Self {
            max_active_tasks,
            max_queued_completions,
            child_execution_limits,
            max_host_calls,
            timeout,
            capabilities,
        })
    }

    #[must_use]
    pub const fn max_active_tasks(&self) -> NonZeroUsize {
        self.max_active_tasks
    }

    #[must_use]
    pub const fn max_queued_completions(&self) -> NonZeroUsize {
        self.max_queued_completions
    }

    #[must_use]
    pub const fn child_execution_limits(&self) -> ExecutionLimits {
        self.child_execution_limits
    }

    #[must_use]
    pub const fn max_host_calls(&self) -> NonZeroU64 {
        self.max_host_calls
    }

    #[must_use]
    pub const fn timeout(&self) -> Duration {
        self.timeout
    }

    #[must_use]
    pub const fn capabilities(&self) -> CapabilitySet {
        self.capabilities
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskPolicyError {
    ZeroTimeout,
    UnboundedLimit,
}

impl fmt::Display for TaskPolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroTimeout => formatter.write_str("task timeout must be non-zero"),
            Self::UnboundedLimit => formatter.write_str("task policy limits must be finite"),
        }
    }
}

impl std::error::Error for TaskPolicyError {}

/// Immutable generation identity retained by a detached child.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskGeneration {
    Ordinary {
        executable: ExecutableGenerationId,
    },
    Service {
        executable: ExecutableGenerationId,
        service_set: ServiceSetId,
        service_generation: ServiceGenerationId,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServiceTaskGeneration {
    pub service_set: ServiceSetId,
    pub service_generation: ServiceGenerationId,
}

/// Generation-local authority ceilings sealed before task admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TaskAuthorityCeilings {
    pub caller: CapabilitySet,
    pub artifact: CapabilitySet,
    pub service: Option<CapabilitySet>,
}

impl TaskAuthorityCeilings {
    #[must_use]
    pub const fn ordinary(caller: CapabilitySet, artifact: CapabilitySet) -> Self {
        Self {
            caller,
            artifact,
            service: None,
        }
    }

    #[must_use]
    pub const fn service(
        caller: CapabilitySet,
        artifact: CapabilitySet,
        service: CapabilitySet,
    ) -> Self {
        Self {
            caller,
            artifact,
            service: Some(service),
        }
    }

    #[must_use]
    const fn effective(self, engine: CapabilitySet, policy: CapabilitySet) -> CapabilitySet {
        let service = match self.service {
            Some(service) => service,
            None => CapabilitySet::all(),
        };
        engine
            .intersection(self.caller)
            .intersection(self.artifact)
            .intersection(service)
            .intersection(policy)
    }
}

impl TaskGeneration {
    #[must_use]
    pub const fn executable(self) -> ExecutableGenerationId {
        match self {
            Self::Ordinary { executable } | Self::Service { executable, .. } => executable,
        }
    }
}

/// All owned authority required to construct an isolated child Runtime.
///
/// The capsule retains immutable code and registry authority but never a
/// caller Runtime, frame, HostRef table, or borrowed host context.
#[derive(Clone)]
pub struct TaskExecutionCapsule {
    engine: Engine,
    artifact: Arc<LinkedArtifact>,
    policy: TaskPolicy,
    generation: TaskGeneration,
    effective_capabilities: CapabilitySet,
    pinned_service: Option<crate::service::PinnedServiceExecution>,
    runtime_pool: runtime_pool::DetachedRuntimePool,
}

impl TaskExecutionCapsule {
    #[must_use]
    pub fn ordinary(
        engine: Engine,
        artifact: Arc<LinkedArtifact>,
        ceilings: TaskAuthorityCeilings,
        policy: TaskPolicy,
    ) -> Self {
        let generation = TaskGeneration::Ordinary {
            executable: artifact.generation(),
        };
        Self::new(engine, artifact, ceilings, policy, generation, None)
    }

    #[doc(hidden)]
    #[must_use]
    pub fn for_service_generation(
        engine: Engine,
        artifact: Arc<LinkedArtifact>,
        caller_ceiling: CapabilitySet,
        artifact_ceiling: CapabilitySet,
        policy: TaskPolicy,
        pinned_service: crate::service::PinnedServiceExecution,
    ) -> Self {
        let service = ServiceTaskGeneration {
            service_set: pinned_service.service_set(),
            service_generation: pinned_service.generation(),
        };
        let generation = TaskGeneration::Service {
            executable: artifact.generation(),
            service_set: service.service_set,
            service_generation: service.service_generation,
        };
        let ceilings = TaskAuthorityCeilings::service(
            caller_ceiling,
            artifact_ceiling,
            pinned_service.patch_effect_ceiling(),
        );
        Self::new(
            engine,
            artifact,
            ceilings,
            policy,
            generation,
            Some(pinned_service),
        )
    }

    fn new(
        engine: Engine,
        artifact: Arc<LinkedArtifact>,
        ceilings: TaskAuthorityCeilings,
        policy: TaskPolicy,
        generation: TaskGeneration,
        pinned_service: Option<crate::service::PinnedServiceExecution>,
    ) -> Self {
        let effective_capabilities =
            ceilings.effective(engine.capabilities(), policy.capabilities());
        let runtime_pool = runtime_pool::DetachedRuntimePool::new(policy.max_active_tasks().get());
        Self {
            engine,
            artifact,
            policy,
            generation,
            effective_capabilities,
            pinned_service,
            runtime_pool,
        }
    }

    pub(crate) fn with_runtime_pool(
        mut self,
        runtime_pool: runtime_pool::DetachedRuntimePool,
    ) -> Self {
        self.runtime_pool = runtime_pool;
        self
    }

    pub(crate) fn lease_runtime(
        &self,
    ) -> Result<runtime_pool::DetachedRuntimeLease, crate::runtime::RuntimeBuildError> {
        self.runtime_pool.lease(&self.engine, &self.artifact)
    }

    #[must_use]
    pub const fn engine(&self) -> &Engine {
        &self.engine
    }

    #[must_use]
    pub const fn artifact(&self) -> &Arc<LinkedArtifact> {
        &self.artifact
    }

    #[must_use]
    pub const fn policy(&self) -> &TaskPolicy {
        &self.policy
    }

    #[must_use]
    pub const fn generation(&self) -> TaskGeneration {
        self.generation
    }

    #[must_use]
    pub const fn effective_capabilities(&self) -> CapabilitySet {
        self.effective_capabilities
    }

    #[must_use]
    pub const fn pinned_service(&self) -> Option<&crate::service::PinnedServiceExecution> {
        self.pinned_service.as_ref()
    }
}

impl fmt::Debug for TaskExecutionCapsule {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TaskExecutionCapsule")
            .field("artifact", &self.artifact.checksum())
            .field("policy", &self.policy)
            .field("generation", &self.generation)
            .field("effective_capabilities", &self.effective_capabilities)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskContinuation {
    pub function: FunctionId,
    pub debug_name: String,
    pub resume_parameters: Box<[vela_bytecode::ArtifactTaskParameter]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskMetadata {
    pub task_id: TaskId,
    pub caller: FunctionId,
    pub worker: FunctionId,
    pub worker_debug_name: String,
    pub continuation: Option<TaskContinuation>,
    pub source_span: Option<Span>,
    pub generation: TaskGeneration,
}

/// One fully owned unit transferred through [`ScopedTaskHost::admit`].
pub struct ScopedTask {
    metadata: TaskMetadata,
    capsule: Arc<TaskExecutionCapsule>,
    future: ScopedTaskFuture,
    telemetry: TaskTelemetry,
}

impl ScopedTask {
    #[must_use]
    pub(crate) fn new(
        metadata: TaskMetadata,
        capsule: Arc<TaskExecutionCapsule>,
        worker: ScopedWorkerFuture,
        telemetry: TaskTelemetry,
    ) -> Self {
        assert_eq!(metadata.generation, capsule.generation());
        let deadline = Instant::now()
            .checked_add(capsule.policy().timeout())
            .expect("TaskPolicy rejected an unrepresentable deadline");
        let future = Box::pin(ContainedTaskFuture {
            worker: Some(worker),
            metadata: metadata.clone(),
            deadline,
            telemetry: telemetry.clone(),
            terminal_reported: false,
        });
        Self {
            metadata,
            capsule,
            future,
            telemetry,
        }
    }

    #[must_use]
    pub const fn metadata(&self) -> &TaskMetadata {
        &self.metadata
    }

    #[must_use]
    pub const fn capsule(&self) -> &Arc<TaskExecutionCapsule> {
        &self.capsule
    }

    #[must_use]
    pub fn into_parts(self) -> (TaskMetadata, Arc<TaskExecutionCapsule>, ScopedTaskFuture) {
        (self.metadata, self.capsule, self.future)
    }

    /// Converts the admitted work into a completion future that pins its exact
    /// executable and Service generation until delivery or cancellation.
    #[must_use]
    pub fn into_completion_future(self) -> ScopedTaskCompletionFuture {
        let Self {
            metadata,
            capsule,
            future,
            telemetry,
        } = self;
        Box::pin(async move {
            let outcome = future.await;
            ScopedTaskCompletion {
                metadata,
                capsule,
                outcome,
                continuation_eligible: true,
                telemetry,
            }
        })
    }

    /// Polls host-owned work without exposing any handle to Vela code.
    pub fn poll(&mut self, context: &mut Context<'_>) -> Poll<ScopedTaskOutcome> {
        self.future.as_mut().poll(context)
    }

    /// Polls work while retaining the task in a host-owned lifecycle slot.
    /// A ready result clones only immutable metadata and the capsule `Arc`;
    /// dropping the slot immediately afterwards leaves the returned completion
    /// as the sole generation pin.
    pub fn poll_completion(&mut self, context: &mut Context<'_>) -> Poll<ScopedTaskCompletion> {
        match self.future.as_mut().poll(context) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(outcome) => Poll::Ready(ScopedTaskCompletion {
                metadata: self.metadata.clone(),
                capsule: Arc::clone(&self.capsule),
                outcome,
                continuation_eligible: true,
                telemetry: self.telemetry.clone(),
            }),
        }
    }

    /// Cancels host-owned work while preserving a structured lifecycle result.
    /// Dropping the worker first tears down its child Runtime and pending native
    /// future before the cancellation can be observed by completion handling.
    #[must_use]
    pub fn cancel(self, reason: TaskCancellationReason) -> ScopedTaskOutcome {
        let Self {
            metadata,
            capsule,
            future,
            telemetry,
        } = self;
        telemetry.worker_cancelled(reason);
        drop(future);
        drop(capsule);
        ScopedTaskOutcome::Failed(TaskError {
            kind: TaskErrorKind::Cancelled(reason),
            metadata,
            detail: format!("host lifecycle cancelled task: {reason:?}"),
        })
    }

    /// Cancels work and returns an observable completion that cannot re-enter
    /// Vela. The capsule remains pinned until the host drops the completion.
    #[must_use]
    pub fn cancel_completion(self, reason: TaskCancellationReason) -> ScopedTaskCompletion {
        let Self {
            metadata,
            capsule,
            future,
            telemetry,
        } = self;
        telemetry.worker_cancelled(reason);
        drop(future);
        ScopedTaskCompletion {
            outcome: cancelled_outcome(&metadata, reason),
            metadata,
            capsule,
            continuation_eligible: false,
            telemetry,
        }
    }
}

impl fmt::Debug for ScopedTask {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ScopedTask")
            .field("metadata", &self.metadata)
            .field("capsule", &self.capsule)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug)]
pub enum ScopedTaskOutcome {
    Completed(DetachedValueImage),
    Failed(TaskError),
}

/// Terminal worker result plus the authority required for optional safe-point
/// continuation delivery.
pub struct ScopedTaskCompletion {
    metadata: TaskMetadata,
    capsule: Arc<TaskExecutionCapsule>,
    outcome: ScopedTaskOutcome,
    continuation_eligible: bool,
    telemetry: TaskTelemetry,
}

impl ScopedTaskCompletion {
    #[must_use]
    pub const fn metadata(&self) -> &TaskMetadata {
        &self.metadata
    }

    #[must_use]
    pub const fn capsule(&self) -> &Arc<TaskExecutionCapsule> {
        &self.capsule
    }

    #[must_use]
    pub const fn outcome(&self) -> &ScopedTaskOutcome {
        &self.outcome
    }

    /// Marks a queued completion cancelled. Consuming it at a later safe point
    /// becomes a no-op, so shutdown cannot cause script re-entry.
    #[must_use]
    pub fn cancel(mut self, reason: TaskCancellationReason) -> Self {
        self.outcome = cancelled_outcome(&self.metadata, reason);
        self.continuation_eligible = false;
        self.telemetry.continuation_suppressed();
        self
    }

    /// Runs the sealed continuation as one fresh synchronous root turn.
    ///
    /// `args` contains only the freshly acquired trailing resume context. The
    /// detached `Result` outcome is inserted as parameter zero by the engine.
    #[must_use]
    pub fn resume<'host>(
        self,
        args: crate::runtime::CallArgs<'host>,
        options: crate::runtime::CallOptions,
    ) -> TaskContinuationOutcome {
        if !self.continuation_eligible || self.metadata.continuation.is_none() {
            if self.metadata.continuation.is_some() {
                self.telemetry.continuation_suppressed();
            }
            return TaskContinuationOutcome::NotRequested;
        }
        let metadata = self.metadata.clone();
        let telemetry = self.telemetry.clone();
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            crate::runtime::resume_task_continuation(self, args, options)
        })) {
            Ok(Ok(())) => {
                telemetry.continuation_completed();
                TaskContinuationOutcome::Completed
            }
            Ok(Err((kind, detail))) => {
                telemetry.continuation_failed(kind.clone());
                TaskContinuationOutcome::Failed(Box::new(TaskError {
                    kind,
                    metadata,
                    detail,
                }))
            }
            Err(payload) => {
                telemetry.continuation_failed(TaskErrorKind::ContinuationPanicked);
                TaskContinuationOutcome::Failed(Box::new(TaskError {
                    kind: TaskErrorKind::ContinuationPanicked,
                    metadata,
                    detail: panic_detail(payload.as_ref()),
                }))
            }
        }
    }

    pub(crate) fn into_resume_parts(
        self,
    ) -> (TaskMetadata, Arc<TaskExecutionCapsule>, ScopedTaskOutcome) {
        (self.metadata, self.capsule, self.outcome)
    }
}

impl fmt::Debug for ScopedTaskCompletion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ScopedTaskCompletion")
            .field("metadata", &self.metadata)
            .field("capsule", &self.capsule)
            .field("outcome", &self.outcome)
            .field("continuation_eligible", &self.continuation_eligible)
            .finish()
    }
}

#[derive(Clone, Debug)]
pub enum TaskContinuationOutcome {
    NotRequested,
    Completed,
    Failed(Box<TaskError>),
}

fn cancelled_outcome(metadata: &TaskMetadata, reason: TaskCancellationReason) -> ScopedTaskOutcome {
    ScopedTaskOutcome::Failed(TaskError {
        kind: TaskErrorKind::Cancelled(reason),
        metadata: metadata.clone(),
        detail: format!("host lifecycle cancelled task: {reason:?}"),
    })
}

struct ContainedTaskFuture {
    worker: Option<ScopedWorkerFuture>,
    metadata: TaskMetadata,
    deadline: Instant,
    telemetry: TaskTelemetry,
    terminal_reported: bool,
}

impl Future for ContainedTaskFuture {
    type Output = ScopedTaskOutcome;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        if Instant::now() >= this.deadline {
            this.worker.take();
            this.terminal_reported = true;
            this.telemetry
                .worker_failed(TaskErrorKind::DeadlineExceeded);
            return Poll::Ready(ScopedTaskOutcome::Failed(TaskError {
                kind: TaskErrorKind::DeadlineExceeded,
                metadata: this.metadata.clone(),
                detail: "detached task deadline exceeded".to_owned(),
            }));
        }
        let Some(worker) = this.worker.as_mut() else {
            this.terminal_reported = true;
            this.telemetry.worker_failed(TaskErrorKind::WorkerTrap);
            return Poll::Ready(ScopedTaskOutcome::Failed(TaskError {
                kind: TaskErrorKind::WorkerTrap,
                metadata: this.metadata.clone(),
                detail: "detached worker was polled after completion".to_owned(),
            }));
        };
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            worker.as_mut().poll(context)
        })) {
            Ok(Poll::Pending) => {
                this.telemetry.worker_pending();
                Poll::Pending
            }
            Ok(Poll::Ready(Ok(image))) => {
                this.worker.take();
                this.terminal_reported = true;
                this.telemetry.worker_completed();
                Poll::Ready(ScopedTaskOutcome::Completed(image))
            }
            Ok(Poll::Ready(Err((kind, detail)))) => {
                this.worker.take();
                this.terminal_reported = true;
                this.telemetry.worker_failed(kind.clone());
                Poll::Ready(ScopedTaskOutcome::Failed(TaskError {
                    kind,
                    metadata: this.metadata.clone(),
                    detail,
                }))
            }
            Err(payload) => {
                this.worker.take();
                this.terminal_reported = true;
                this.telemetry.worker_failed(TaskErrorKind::WorkerPanicked);
                Poll::Ready(ScopedTaskOutcome::Failed(TaskError {
                    kind: TaskErrorKind::WorkerPanicked,
                    metadata: this.metadata.clone(),
                    detail: panic_detail(payload.as_ref()),
                }))
            }
        }
    }
}

impl Drop for ContainedTaskFuture {
    fn drop(&mut self) {
        if !self.terminal_reported {
            self.telemetry.worker_dropped();
            self.worker.take();
            self.terminal_reported = true;
        }
    }
}

fn panic_detail(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| {
            payload
                .downcast_ref::<&str>()
                .map(|value| (*value).to_owned())
        })
        .unwrap_or_else(|| "detached worker panicked with a non-string payload".to_owned())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskCancellationReason {
    ScopeClosed,
    Deadline,
    HostShutdown,
    CompletionQueueFull,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TaskErrorKind {
    ScopeUnavailable,
    AdmissionDenied,
    CapacityExceeded,
    TargetNotStatic,
    TargetNotAsync,
    ContinuationInvalid,
    ValueNotDetachable,
    EffectDenied,
    CapabilityDenied,
    BudgetExceeded,
    DeadlineExceeded,
    Cancelled(TaskCancellationReason),
    WorkerError,
    WorkerTrap,
    WorkerPanicked,
    ContinuationError,
    ContinuationPanicked,
    GenerationUnavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskError {
    pub kind: TaskErrorKind,
    pub metadata: TaskMetadata,
    pub detail: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TaskAdmissionError {
    ScopeClosed,
    CapacityExceeded { maximum: usize },
    EffectDenied { denied: CapabilitySet },
    CapabilityDenied { denied: CapabilitySet },
    GenerationUnavailable { generation: TaskGeneration },
}

impl fmt::Display for TaskAdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ScopeClosed => formatter.write_str("task scope is closed"),
            Self::CapacityExceeded { maximum } => {
                write!(formatter, "task scope capacity {maximum} is exhausted")
            }
            Self::EffectDenied { denied } => {
                write!(formatter, "task effects exceed scope policy: {denied:?}")
            }
            Self::CapabilityDenied { denied } => {
                write!(formatter, "task capabilities are unavailable: {denied:?}")
            }
            Self::GenerationUnavailable { generation } => {
                write!(formatter, "task generation is unavailable: {generation:?}")
            }
        }
    }
}

impl std::error::Error for TaskAdmissionError {}

#[cfg(test)]
mod tests {
    use std::num::{NonZeroU64, NonZeroUsize};
    use std::sync::Arc;
    use std::time::Duration;

    use vela_common::{Capability, CapabilitySet};
    use vela_vm::budget::{CollectionLimits, ExecutionLimits};

    use super::{TaskAuthorityCeilings, TaskExecutionCapsule, TaskPolicy, TaskPolicyError};
    use crate::engine::Engine;

    fn finite_policy(capabilities: CapabilitySet) -> TaskPolicy {
        TaskPolicy::new(
            NonZeroUsize::new(8).expect("non-zero"),
            NonZeroUsize::new(16).expect("non-zero"),
            ExecutionLimits::new(10_000, 1 << 20, 64).with_collection_limits(CollectionLimits {
                max_array_len: 1_024,
                max_map_entries: 1_024,
                max_set_len: 1_024,
            }),
            NonZeroU64::new(64).expect("non-zero"),
            Duration::from_secs(5),
            capabilities,
        )
        .expect("finite policy")
    }

    #[test]
    fn task_policy_rejects_unbounded_limits_and_zero_timeout() {
        assert_eq!(
            TaskPolicy::new(
                NonZeroUsize::MIN,
                NonZeroUsize::MIN,
                ExecutionLimits::unbounded(),
                NonZeroU64::MIN,
                Duration::from_secs(1),
                CapabilitySet::all(),
            ),
            Err(TaskPolicyError::UnboundedLimit)
        );
        assert_eq!(
            TaskPolicy::new(
                NonZeroUsize::MIN,
                NonZeroUsize::MIN,
                ExecutionLimits::new(1, 1, 1).with_collection_limits(CollectionLimits {
                    max_array_len: 1,
                    max_map_entries: 1,
                    max_set_len: 1,
                }),
                NonZeroU64::MIN,
                Duration::ZERO,
                CapabilitySet::all(),
            ),
            Err(TaskPolicyError::ZeroTimeout)
        );
    }

    #[test]
    fn capsule_intersects_every_ordinary_authority_ceiling() {
        let engine = Engine::builder()
            .capabilities(
                CapabilitySet::new()
                    .with(Capability::TaskSpawn)
                    .with(Capability::IoRead)
                    .with(Capability::HostWrite),
            )
            .build()
            .expect("engine");
        let compiled = engine
            .compile_source("fn main() { return (); }")
            .expect("compile");
        let artifact = engine.link_compiled_program(compiled).expect("link");
        let policy = finite_policy(
            CapabilitySet::new()
                .with(Capability::TaskSpawn)
                .with(Capability::IoRead),
        );
        let capsule = TaskExecutionCapsule::ordinary(
            engine,
            Arc::clone(&artifact),
            TaskAuthorityCeilings::ordinary(
                CapabilitySet::new()
                    .with(Capability::TaskSpawn)
                    .with(Capability::IoRead),
                CapabilitySet::new()
                    .with(Capability::TaskSpawn)
                    .with(Capability::HostWrite),
            ),
            policy,
        );

        assert_eq!(capsule.artifact().generation(), artifact.generation());
        assert_eq!(
            capsule.effective_capabilities(),
            CapabilitySet::new().with(Capability::TaskSpawn)
        );
    }
}
