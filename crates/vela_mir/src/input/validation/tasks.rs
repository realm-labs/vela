use vela_common::{CallableAsyncness, Detachability};

use crate::{CompileCalleeTarget, CompileTaskOperation, MirBuildError};

use super::SnapshotValidator;

pub(super) fn validate(validator: &SnapshotValidator<'_>) -> Result<(), MirBuildError> {
    for ((root, expression), task) in &validator.snapshot.tasks {
        let origin =
            validator.retained_origin(&validator.snapshot.origins.tasks, &(*root, *expression));
        validator.require_root(*root, origin, "task placement")?;
        let worker = validator.require_script_function(task.worker, origin, "task worker")?;
        if worker.signature.asyncness != CallableAsyncness::Async {
            return Err(validator.error(origin, "task worker descriptor must be asynchronous"));
        }
        if task.detachability.parameters.len() != worker.signature.parameters.len() {
            return Err(validator.error(
                origin,
                "task detachability parameters must match the worker signature",
            ));
        }
        for (parameter, fact) in worker
            .signature
            .parameters
            .iter()
            .zip(&task.detachability.parameters)
        {
            if fact.rejection().is_some() {
                return Err(validator.error(
                    origin,
                    "task parameter detachability cannot contain a static rejection",
                ));
            }
            let expected = crate::contract_detachability(
                validator.snapshot.target_table(),
                parameter.contract.as_ref(),
            )
            .fact;
            if expected.rejection().is_some() || expected.union(*fact) != *fact {
                return Err(validator.error(
                    origin,
                    "task parameter detachability is weaker than its worker contract",
                ));
            }
        }
        let expected_result = crate::contract_detachability(
            validator.snapshot.target_table(),
            worker.signature.return_contract.as_ref(),
        )
        .fact;
        if task.detachability.result != expected_result
            || matches!(expected_result, Detachability::NonDetachable(_))
        {
            return Err(validator.error(
                origin,
                "task result detachability disagrees with its worker contract",
            ));
        }
        let worker_call = validator
            .snapshot
            .call(*root, task.worker_call)
            .ok_or_else(|| validator.error(origin, "task worker call has no call placement"))?;
        if !matches!(
            &worker_call.callee,
            CompileCalleeTarget::ScriptFunction { function, debug_name }
                if *function == task.worker && *debug_name == task.worker_debug_name
        ) {
            return Err(validator.error(
                origin,
                "task worker identity disagrees with its call placement",
            ));
        }
        match (task.operation, &task.continuation) {
            (CompileTaskOperation::SpawnScoped, None) => {}
            (CompileTaskOperation::SpawnScopedThen, Some(continuation)) => {
                let descriptor = validator.require_script_function(
                    continuation.function,
                    origin,
                    "task continuation",
                )?;
                if descriptor.signature.asyncness != CallableAsyncness::Sync {
                    return Err(
                        validator.error(origin, "task continuation descriptor must be synchronous")
                    );
                }
                if descriptor.debug_name != continuation.debug_name
                    && descriptor.canonical_symbol != continuation.debug_name
                {
                    return Err(validator.error(
                        origin,
                        "task continuation identity disagrees with its descriptor",
                    ));
                }
                let expected_outcome = crate::MirTypeContract::Result {
                    ok: worker.signature.return_contract.clone().map(Box::new),
                    err: Some(Box::new(crate::MirTypeContract::TaskError)),
                };
                if continuation.outcome_contract != expected_outcome
                    || descriptor
                        .signature
                        .parameters
                        .first()
                        .and_then(|parameter| parameter.contract.as_ref())
                        != Some(&expected_outcome)
                    || descriptor.signature.parameters.get(1..)
                        != Some(continuation.resume_parameters.as_slice())
                {
                    return Err(validator.error(
                        origin,
                        "task continuation ABI disagrees with worker outcome or resume parameters",
                    ));
                }
            }
            (CompileTaskOperation::SpawnScoped, Some(_)) => {
                return Err(validator.error(origin, "spawn_scoped cannot carry a continuation"));
            }
            (CompileTaskOperation::SpawnScopedThen, None) => {
                return Err(validator.error(origin, "spawn_scoped_then requires a continuation"));
            }
        }
    }
    Ok(())
}
