use vela_bytecode::Program;

use crate::heap::GcRef;
use crate::runtime_checks::expect_closure_ref;
use crate::{
    ExecutionBudget, ExecutionCall, HeapExecution, HostExecution, SmallStorage, Value, Vm, VmError,
    VmResult,
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
    let (code, captures) = {
        let closure = expect_closure_ref(callback, runtime.heap.as_deref(), operation)?;
        let captures = SmallStorage::try_from_slice_map(&closure.captures, 4, |value| {
            Ok::<_, VmError>(*value)
        })?;
        (closure.code.clone(), captures)
    };
    let protected_root_len = runtime.heap.as_deref_mut().map(|heap| {
        let protected_root_len = heap.push_protected_roots(runtime.caller_roots);
        heap.protect_values(args);
        heap.protect_value_refs(protected_values);
        protected_root_len
    });
    let result = runtime.vm.execute_call(
        ExecutionCall {
            code: &code,
            program: runtime.program,
            captures: captures.as_slice(),
            args,
            call_site: None,
            call_site_offset: None,
            inline_caches: None,
        },
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
