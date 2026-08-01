//! Runtime-side admission of one VM-prepared detached task.

use std::sync::Arc;

use vela_common::{Capability, CapabilitySet};
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::{DetachedValueImage, PreparedTaskCall};

use super::{CallArgs, CallOptions, handles};
use crate::engine::Engine;
use crate::task::{
    ScopedTask, TaskAuthorityCeilings, TaskCancellationReason, TaskContinuation, TaskErrorKind,
    TaskExecutionCapsule, TaskMetadata, TaskScope,
};

pub(super) fn admit(
    engine: &Engine,
    scope: Option<&TaskScope>,
    pinned_service: Option<&crate::service::PinnedServiceExecution>,
    prepared: PreparedTaskCall,
    parent_heap: &vela_vm::heap::ScriptHeap,
    parent_budget: &mut vela_vm::budget::ExecutionBudget,
) -> VmResult<()> {
    let scope = scope.ok_or_else(|| {
        VmError::new(VmErrorKind::TaskScopeUnavailable).with_source_span(prepared.source_span())
    })?;
    let continuation_target = prepared.continuation().map(|value| value.function.get());
    let target = prepared
        .owner()
        .task_targets()
        .iter()
        .find(|target| {
            target.worker_target == prepared.worker().get()
                && target.continuation.as_ref().map(|value| value.target) == continuation_target
        })
        .ok_or_else(|| {
            VmError::new(VmErrorKind::TaskAdmissionDenied {
                reason: format!(
                    "sealed metadata for worker `{}` is unavailable",
                    prepared.worker_name()
                ),
            })
            .with_source_span(prepared.source_span())
        })?;
    let generation = match pinned_service {
        Some(service) => crate::task::TaskGeneration::Service {
            executable: prepared.owner().generation(),
            service_set: service.service_set(),
            service_generation: service.generation(),
        },
        None => crate::task::TaskGeneration::Ordinary {
            executable: prepared.owner().generation(),
        },
    };
    if let Some(service) = pinned_service
        && (service.artifact().generation() != prepared.owner().generation()
            || service.artifact().checksum() != prepared.owner().checksum())
    {
        return Err(VmError::new(VmErrorKind::TaskAdmissionDenied {
            reason: "active Service execution artifact does not own the sealed task target"
                .to_owned(),
        })
        .with_source_span(prepared.source_span()));
    }
    let task_id = scope.allocate_task_id().ok_or_else(|| {
        VmError::new(VmErrorKind::TaskAdmissionDenied {
            reason: "task scope diagnostic identifier space is exhausted".to_owned(),
        })
        .with_source_span(prepared.source_span())
    })?;
    let metadata = TaskMetadata {
        task_id,
        caller: target.caller,
        worker: target.worker,
        worker_debug_name: target.worker_debug_name.clone(),
        continuation: target
            .continuation
            .as_ref()
            .map(|continuation| TaskContinuation {
                function: continuation.function,
                debug_name: continuation.debug_name.clone(),
                resume_parameters: continuation.resume_parameters.clone(),
            }),
        source_span: prepared.source_span(),
        generation,
    };
    let detached_effects = target
        .continuation
        .as_ref()
        .map_or(target.worker_signature.effects, |continuation| {
            target.worker_signature.effects.union(continuation.effects)
        });
    let required = required_capabilities(detached_effects);
    let artifact_ceiling = required.with(Capability::TaskSpawn);
    let capsule = match pinned_service {
        Some(service) => TaskExecutionCapsule::for_service_generation(
            engine.clone(),
            Arc::clone(service.artifact()),
            engine.capabilities(),
            artifact_ceiling,
            scope.policy().clone(),
            service.clone(),
        ),
        None => TaskExecutionCapsule::ordinary(
            engine.clone(),
            Arc::clone(prepared.owner()),
            TaskAuthorityCeilings::ordinary(engine.capabilities(), artifact_ceiling),
            scope.policy().clone(),
        ),
    }
    .with_runtime_pool(scope.runtime_pool());
    let capsule = Arc::new(capsule);
    let available = capsule.effective_capabilities();
    if !available.contains(Capability::TaskSpawn) || !available.contains_all(required) {
        let denied = required.with(Capability::TaskSpawn).difference(available);
        return Err(VmError::new(VmErrorKind::TaskAdmissionDenied {
            reason: format!("capabilities denied: {denied:?}"),
        })
        .with_source_span(prepared.source_span()));
    }
    let source_span = prepared.source_span();
    let worker = target.worker;
    let worker_name = target.worker_debug_name.clone();
    let args = DetachedValueImage::export_arguments(
        prepared.arguments(),
        Some(parent_heap),
        parent_budget,
    )?;
    let child_capsule = Arc::clone(&capsule);
    let child_scope = scope.clone();
    let future = Box::pin(run_child(
        child_capsule,
        child_scope,
        worker,
        worker_name,
        args,
    ));
    let telemetry = scope.begin_task(&metadata);
    match scope.host().admit(ScopedTask::new(
        metadata,
        capsule,
        future,
        telemetry.clone(),
    )) {
        Ok(()) => {
            telemetry.admitted();
            Ok(())
        }
        Err(error) => {
            telemetry.rejected(error.clone());
            Err(VmError::new(VmErrorKind::TaskAdmissionDenied {
                reason: error.to_string(),
            })
            .with_source_span(source_span))
        }
    }
}

async fn run_child(
    capsule: Arc<TaskExecutionCapsule>,
    scope: TaskScope,
    worker: vela_def::FunctionId,
    worker_name: String,
    args: DetachedValueImage,
) -> Result<DetachedValueImage, (TaskErrorKind, String)> {
    let mut runtime_lease = capsule
        .lease_runtime()
        .map_err(|error| (TaskErrorKind::GenerationUnavailable, error.to_string()))?;
    let runtime = runtime_lease.runtime();
    let limits = capsule.policy().child_execution_limits();
    let options = CallOptions::new(
        limits.execution_unit_limit,
        limits.memory_limit_bytes,
        limits.max_call_depth,
    )
    .with_collection_limits(limits.collection_limits)
    .with_host_call_budget(capsule.policy().max_host_calls().get())
    .with_task_scope(scope);
    let (value, mut budget) = runtime
        .call_impl_async_with_budget(
            handles::StableVelaFunction {
                function: worker,
                diagnostic_name: worker_name,
            },
            CallArgs::from_detached_image(args),
            options,
            capsule.pinned_service().cloned(),
        )
        .await
        .map_err(worker_failure)?;
    DetachedValueImage::export_result(value.value(), &runtime.state.vm_states.heap, &mut budget)
        .map_err(worker_failure)
}

fn worker_failure(error: VmError) -> (TaskErrorKind, String) {
    let kind = match error.kind() {
        VmErrorKind::TaskValueNotDetachable { .. } => TaskErrorKind::ValueNotDetachable,
        VmErrorKind::BudgetExceeded { .. } | VmErrorKind::CollectionLimitExceeded { .. } => {
            TaskErrorKind::BudgetExceeded
        }
        VmErrorKind::DeadlineExceeded => TaskErrorKind::DeadlineExceeded,
        VmErrorKind::CallCancelled => TaskErrorKind::Cancelled(TaskCancellationReason::ScopeClosed),
        VmErrorKind::UnsupportedLinkedInstruction { .. } => TaskErrorKind::WorkerTrap,
        _ => TaskErrorKind::WorkerError,
    };
    (kind, error.to_string())
}

fn required_capabilities(effect: vela_mir::MirEffect) -> CapabilitySet {
    let mut required = CapabilitySet::new();
    for (active, capability) in [
        (effect.host_read && !effect.host_write, Capability::HostRead),
        (effect.host_write || effect.host_call, Capability::HostWrite),
        (effect.reflection_read, Capability::ReflectionRead),
        (effect.reflection_write, Capability::ReflectionWrite),
        (effect.reflection_call, Capability::ReflectionCall),
        (effect.emits_event, Capability::EventEmit),
        (effect.reads_time, Capability::Time),
        (effect.uses_random, Capability::Random),
        (effect.reads_io, Capability::IoRead),
        (effect.writes_io, Capability::IoWrite),
        (effect.task_spawn, Capability::TaskSpawn),
    ] {
        if active {
            required.insert(capability);
        }
    }
    required
}
