//! Fresh-root delivery for one host-observed detached task completion.

use std::sync::Arc;

use super::{CallArgs, CallOptions, RuntimeServiceCall, handles};
use crate::task::{ScopedTaskCompletion, TaskErrorKind};

pub(crate) fn resume_task_continuation<'host>(
    completion: ScopedTaskCompletion,
    args: CallArgs<'host>,
    options: CallOptions,
) -> Result<(), (TaskErrorKind, String)> {
    let (metadata, capsule, outcome) = completion.into_resume_parts();
    let continuation = metadata.continuation.ok_or_else(|| {
        (
            TaskErrorKind::ContinuationError,
            "completion has no sealed continuation".to_owned(),
        )
    })?;
    let mut runtime_lease = capsule
        .lease_runtime()
        .map_err(|error| (TaskErrorKind::GenerationUnavailable, error.to_string()))?;
    let runtime = runtime_lease.runtime();
    let service = match capsule.pinned_service().cloned() {
        Some(execution) => RuntimeServiceCall {
            dispatcher: Some(Arc::clone(execution.dispatcher())),
            pinned: Some(execution),
            scoped_return: None,
        },
        None => RuntimeServiceCall::default(),
    };
    let options = options.narrow_to_task_policy(capsule.policy());
    runtime
        .call_impl_with_service_egress(
            handles::StableVelaFunction {
                function: continuation.function,
                diagnostic_name: continuation.debug_name,
            },
            args.with_task_outcome(outcome),
            options,
            false,
            service,
        )
        .map(|_| ())
        .map_err(|error| (TaskErrorKind::ContinuationError, error.to_string()))
}
