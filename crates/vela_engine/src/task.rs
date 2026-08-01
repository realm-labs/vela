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
use std::time::Duration;

use vela_bytecode::{ExecutableGenerationId, LinkedArtifact};
use vela_common::{CapabilitySet, ServiceGenerationId, ServiceSetId, Span};
use vela_def::FunctionId;
use vela_vm::budget::ExecutionLimits;
use vela_vm::owned_value::OwnedValue;

use crate::engine::Engine;

/// Future transferred to the host lifecycle after successful admission.
pub type ScopedTaskFuture = Pin<Box<dyn Future<Output = ScopedTaskOutcome> + Send + 'static>>;

/// Host-owned lifecycle boundary for detached work.
///
/// Admission transfers the complete future and execution capsule. Returning
/// `Ok(())` means the host has assumed responsibility for polling or dropping
/// the future and for retaining the capsule until that drop completes.
pub trait ScopedTaskHost: Send + Sync {
    fn admit(&self, task: ScopedTask) -> Result<(), TaskAdmissionError>;
}

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
        Self::new(engine, artifact, ceilings, policy, generation)
    }

    #[doc(hidden)]
    #[must_use]
    pub fn for_service_generation(
        engine: Engine,
        artifact: Arc<LinkedArtifact>,
        ceilings: TaskAuthorityCeilings,
        policy: TaskPolicy,
        service: ServiceTaskGeneration,
    ) -> Self {
        let generation = TaskGeneration::Service {
            executable: artifact.generation(),
            service_set: service.service_set,
            service_generation: service.service_generation,
        };
        debug_assert!(ceilings.service.is_some());
        Self::new(engine, artifact, ceilings, policy, generation)
    }

    fn new(
        engine: Engine,
        artifact: Arc<LinkedArtifact>,
        ceilings: TaskAuthorityCeilings,
        policy: TaskPolicy,
        generation: TaskGeneration,
    ) -> Self {
        let effective_capabilities =
            ceilings.effective(engine.capabilities(), policy.capabilities());
        Self {
            engine,
            artifact,
            policy,
            generation,
            effective_capabilities,
        }
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskMetadata {
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
}

impl ScopedTask {
    #[must_use]
    pub fn new(
        metadata: TaskMetadata,
        capsule: Arc<TaskExecutionCapsule>,
        future: ScopedTaskFuture,
    ) -> Self {
        assert_eq!(metadata.generation, capsule.generation());
        Self {
            metadata,
            capsule,
            future,
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

#[derive(Clone, Debug, PartialEq)]
pub enum ScopedTaskOutcome {
    Completed(OwnedValue),
    Failed(TaskError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskCancellationReason {
    ScopeClosed,
    Deadline,
    HostShutdown,
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
