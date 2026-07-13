use vela_bytecode::LinkedProgram;
use vela_def::MethodId;

use crate::callback_method_dispatch::{self, CallbackMethodDispatch};
use crate::method_runtime::CallerRoots;
use crate::script_builtin_methods;
use crate::std_method_ids::std_method_ids;
use crate::{
    ExecutionBudget, HeapExecution, HostExecution, Value, Vm, VmBytecodeProfiler, VmError,
    VmErrorKind, VmInlineCaches, VmResult, array_methods,
};

pub(crate) struct ScriptMethodDispatch<'a, 'host, 'heap> {
    pub(crate) vm: &'a Vm,
    pub(crate) program: &'a LinkedProgram,
    pub(crate) host: Option<&'a mut HostExecution<'host>>,
    pub(crate) heap: Option<&'a mut HeapExecution<'heap>>,
    pub(crate) budget: Option<&'a mut ExecutionBudget>,
    pub(crate) caller_roots: CallerRoots<'a>,
    pub(crate) inline_caches: Option<&'a dyn VmInlineCaches>,
    pub(crate) bytecode_profiler: Option<&'a dyn VmBytecodeProfiler>,
}

pub(crate) fn call_method_id(
    receiver: &mut Value,
    method: &str,
    method_id: MethodId,
    args: &[Value],
    mut dispatch: ScriptMethodDispatch<'_, '_, '_>,
) -> VmResult<Value> {
    if let Some(result) = contextual_array_method_by_id(receiver, method_id, args, &mut dispatch) {
        return result;
    }
    if let Some(result) = script_builtin_methods::call_by_id(
        receiver,
        method_id,
        args,
        &mut dispatch.heap,
        &mut dispatch.budget,
    ) {
        return result;
    }
    {
        let mut callback_dispatch = CallbackMethodDispatch {
            vm: dispatch.vm,
            program: dispatch.program,
            host: dispatch.host.as_deref_mut(),
            heap: dispatch.heap.as_deref_mut(),
            budget: dispatch.budget.as_deref_mut(),
            caller_roots: dispatch.caller_roots,
            inline_caches: dispatch.inline_caches,
            bytecode_profiler: dispatch.bytecode_profiler,
        };
        if let Some(result) =
            callback_method_dispatch::call_by_id(method_id, receiver, args, &mut callback_dispatch)
        {
            return result;
        }
    }
    Err(VmError::new(VmErrorKind::UnknownMethod {
        method: method.to_owned(),
    }))
}

fn contextual_array_method_by_id(
    receiver: &Value,
    method_id: MethodId,
    args: &[Value],
    dispatch: &mut ScriptMethodDispatch<'_, '_, '_>,
) -> Option<VmResult<Value>> {
    if !array_methods::is_array(receiver, dispatch.heap.as_deref()) {
        return None;
    }
    let ids = std_method_ids();
    if method_id == ids.array_contains {
        return Some(
            array_methods::contains_by_key(receiver, args, dispatch.heap.as_deref())
                .map(Value::Bool),
        );
    }
    if method_id == ids.array_index_of {
        return Some(array_methods::index_of_by_key(
            receiver,
            args,
            &mut dispatch.heap,
            &mut dispatch.budget,
        ));
    }
    if method_id == ids.array_distinct {
        return Some(array_methods::distinct_by_key(
            receiver,
            args,
            &mut dispatch.heap,
            &mut dispatch.budget,
        ));
    }
    None
}
