//! Runtime-side admission of one VM-prepared detached task.

use std::sync::Arc;

use vela_common::{Capability, CapabilitySet};
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::{PreparedTaskArgument, PreparedTaskCall};

use super::{CallArgs, CallOptions, Runtime, handles};
use crate::engine::Engine;
use crate::task::{
    ScopedTask, ScopedTaskOutcome, TaskAuthorityCeilings, TaskContinuation, TaskError,
    TaskErrorKind, TaskExecutionCapsule, TaskMetadata, TaskScope,
};

pub(super) fn admit(
    engine: &Engine,
    scope: Option<&TaskScope>,
    prepared: PreparedTaskCall,
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
    let metadata = TaskMetadata {
        caller: target.caller,
        worker: target.worker,
        worker_debug_name: target.worker_debug_name.clone(),
        continuation: target
            .continuation
            .as_ref()
            .map(|continuation| TaskContinuation {
                function: continuation.function,
                debug_name: continuation.debug_name.clone(),
            }),
        source_span: prepared.source_span(),
        generation: crate::task::TaskGeneration::Ordinary {
            executable: prepared.owner().generation(),
        },
    };
    let capsule = Arc::new(TaskExecutionCapsule::ordinary(
        engine.clone(),
        Arc::clone(prepared.owner()),
        TaskAuthorityCeilings::ordinary(CapabilitySet::all(), CapabilitySet::all()),
        scope.policy().clone(),
    ));
    let required = required_capabilities(target.worker_signature.effects);
    let available = capsule.effective_capabilities();
    if !available.contains(Capability::TaskSpawn) || !available.contains_all(required) {
        let denied = required.with(Capability::TaskSpawn).difference(available);
        return Err(VmError::new(VmErrorKind::TaskAdmissionDenied {
            reason: format!("capabilities denied: {denied:?}"),
        })
        .with_source_span(prepared.source_span()));
    }
    let args = prepare_args(prepared.args()).map_err(|reason| {
        VmError::new(VmErrorKind::TaskAdmissionDenied { reason })
            .with_source_span(prepared.source_span())
    })?;
    let worker = target.worker;
    let worker_name = target.worker_debug_name.clone();
    let child_capsule = Arc::clone(&capsule);
    let child_scope = scope.clone();
    let outcome_metadata = metadata.clone();
    let future = Box::pin(async move {
        let result = run_child(child_capsule, child_scope, worker, worker_name, args).await;
        match result {
            Ok(value) => ScopedTaskOutcome::Completed(value),
            Err((kind, detail)) => ScopedTaskOutcome::Failed(TaskError {
                kind,
                metadata: outcome_metadata,
                detail,
            }),
        }
    });
    scope
        .host()
        .admit(ScopedTask::new(metadata, capsule, future))
        .map_err(|error| {
            VmError::new(VmErrorKind::TaskAdmissionDenied {
                reason: error.to_string(),
            })
            .with_source_span(prepared.source_span())
        })
}

fn prepare_args(
    args: &[PreparedTaskArgument],
) -> Result<Vec<vela_vm::owned_value::OwnedValue>, String> {
    let mut values = Vec::with_capacity(args.len());
    let mut omitted = false;
    for argument in args {
        match argument {
            PreparedTaskArgument::Missing => omitted = true,
            PreparedTaskArgument::Value(value) if omitted => {
                return Err("non-trailing omitted task arguments are not executable".to_owned());
            }
            PreparedTaskArgument::Value(value) => values.push(value.clone()),
        }
    }
    Ok(values)
}

async fn run_child(
    capsule: Arc<TaskExecutionCapsule>,
    scope: TaskScope,
    worker: vela_def::FunctionId,
    worker_name: String,
    args: Vec<vela_vm::owned_value::OwnedValue>,
) -> Result<vela_vm::owned_value::OwnedValue, (TaskErrorKind, String)> {
    let mut runtime =
        Runtime::from_linked_artifact(capsule.engine().clone(), Arc::clone(capsule.artifact()))
            .map_err(|error| (TaskErrorKind::GenerationUnavailable, error.to_string()))?;
    let limits = capsule.policy().child_execution_limits();
    let options = CallOptions::new(
        limits.execution_unit_limit,
        limits.memory_limit_bytes,
        limits.max_call_depth,
    )
    .with_collection_limits(limits.collection_limits)
    .with_timeout(capsule.policy().timeout())
    .with_task_scope(scope);
    let value = runtime
        .call_impl_async(
            handles::StableVelaFunction {
                function: worker,
                diagnostic_name: worker_name,
            },
            CallArgs::from_positional(args),
            options,
            None,
        )
        .await
        .map_err(|error| (TaskErrorKind::WorkerError, error.to_string()))?;
    runtime
        .value_to_owned(&value)
        .map_err(|error| (TaskErrorKind::WorkerError, error.to_string()))
}

fn required_capabilities(effect: vela_mir::MirEffect) -> CapabilitySet {
    let mut required = CapabilitySet::new();
    for (active, capability) in [
        (effect.host_read, Capability::HostRead),
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
