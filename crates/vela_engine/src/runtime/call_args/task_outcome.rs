use vela_host::adapter::ScriptStateAdapter;
use vela_vm::error::VmResult;
use vela_vm::owned_value::OwnedValue;
use vela_vm::value::Value;

use super::{CallArg, CallArgRuntime, call_args_type_error};

pub(super) fn task_outcome_value(
    outcome: &crate::task::ScopedTaskOutcome,
    runtime: &mut CallArgRuntime<'_, '_, '_>,
    host: &mut (dyn ScriptStateAdapter + Send),
) -> VmResult<Value> {
    match outcome {
        crate::task::ScopedTaskOutcome::Completed(image) => {
            let roots = image.import_into(runtime.heap, runtime.budget)?;
            let [payload] = roots.as_slice() else {
                return Err(call_args_type_error(
                    "detached task result must contain exactly one root",
                ));
            };
            let result = runtime.heap.allocate_enum_with_budget(
                "Result",
                "Ok",
                [("0".to_owned(), *payload)],
                runtime.budget,
            )?;
            Ok(Value::HeapRef(result))
        }
        crate::task::ScopedTaskOutcome::Failed(error) => {
            let task_error = OwnedValue::record(
                "task::Error",
                [
                    ("kind", OwnedValue::String(format!("{:?}", error.kind))),
                    ("detail", OwnedValue::String(error.detail.clone())),
                    (
                        "worker",
                        OwnedValue::String(error.metadata.worker_debug_name.clone()),
                    ),
                    (
                        "generation",
                        OwnedValue::String(format!("{:?}", error.metadata.generation)),
                    ),
                ],
            );
            CallArg::Positional(OwnedValue::enum_variant(
                "Result",
                "Err",
                [("0", task_error)],
            ))
            .runtime_value(runtime, host)
        }
    }
}
