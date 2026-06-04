use vela_bytecode::Program;

use crate::heap::GcRef;
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
    let Value::Closure(closure) = callback else {
        return Err(VmError::new(VmErrorKind::TypeMismatch { operation }));
    };
    let protected_root_len = if runtime.heap.is_some() {
        let mut roots = runtime.caller_roots.to_vec();
        args.iter()
            .for_each(|value| value.trace_heap_refs(&mut roots));
        protected_values
            .iter()
            .for_each(|value| value.trace_heap_refs(&mut roots));
        runtime
            .heap
            .as_deref_mut()
            .map(|heap| heap.push_protected_roots(roots))
    } else {
        None
    };
    let result = runtime.vm.execute_closure_value(
        closure,
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
