use vela_bytecode::Program;

use crate::heap::{GcRef, HeapValue};
use crate::{
    ExecutionBudget, HeapExecution, HostExecution, Value, Vm, VmError, VmErrorKind, VmResult,
};

pub(crate) struct MethodRuntime<'a, 'host, 'heap> {
    pub(crate) vm: &'a Vm,
    pub(crate) program: Option<&'a Program>,
    pub(crate) host: Option<&'a mut HostExecution<'host>>,
    pub(crate) heap: Option<&'a mut HeapExecution<'heap>>,
    pub(crate) budget: Option<&'a mut ExecutionBudget>,
    pub(crate) caller_roots: &'a [GcRef],
}

pub(crate) fn call_callback(
    runtime: &mut MethodRuntime<'_, '_, '_>,
    operation: &'static str,
    callback: &Value,
    args: &[Value],
    protected_values: &[Value],
) -> VmResult<Value> {
    call_callback_with_protected_values(runtime, operation, callback, args, protected_values.iter())
}

pub(crate) fn call_callback_with_protected_values<'value>(
    runtime: &mut MethodRuntime<'_, '_, '_>,
    operation: &'static str,
    callback: &Value,
    args: &[Value],
    protected_values: impl IntoIterator<Item = &'value Value>,
) -> VmResult<Value> {
    let closure = match callback {
        Value::Closure(closure) => closure.clone(),
        Value::HeapRef(reference) => {
            let Some(HeapValue::Closure(closure)) = runtime
                .heap
                .as_deref()
                .and_then(|heap| heap.heap.get(*reference))
            else {
                return Err(VmError::new(VmErrorKind::TypeMismatch { operation }));
            };
            closure.clone()
        }
        _ => return Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
    };
    let protected_root_len = runtime.heap.as_deref_mut().map(|heap| {
        let protected_root_len = heap.push_protected_roots(runtime.caller_roots);
        heap.protect_values(args);
        heap.protect_value_refs(protected_values);
        protected_root_len
    });
    let result = runtime.vm.execute_closure_value(
        &closure,
        runtime.program,
        args,
        runtime.host.as_deref_mut(),
        runtime.heap.as_deref_mut(),
        runtime.budget.as_deref_mut(),
    );
    if let (Some(heap), Some(protected_root_len)) =
        (runtime.heap.as_deref_mut(), protected_root_len)
    {
        heap.truncate_protected_roots(protected_root_len);
    }
    result
}
