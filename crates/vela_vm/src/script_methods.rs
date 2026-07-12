use vela_bytecode::LinkedProgram;
use vela_def::MethodId;
use vela_reflect::registry::TypeRegistry;

use crate::callback_method_dispatch::{self, CallbackMethodDispatch};
use crate::heap::HeapValue;
use crate::method_runtime::CallerRoots;
use crate::script_builtin_methods;
use crate::std_method_ids::std_method_ids;
use crate::{
    EqualityRuntime, ExecutionBudget, HeapExecution, HostExecution, SmallStorage, Value, Vm,
    VmBytecodeProfiler, VmError, VmErrorKind, VmInlineCaches, VmResult, array_methods,
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
    call_script_impl_method(receiver, method_id, method, args, &mut dispatch)
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
    let mut runtime = EqualityRuntime {
        vm: dispatch.vm,
        program: dispatch.program,
        host: dispatch.host.as_deref_mut(),
        heap: dispatch.heap.as_deref_mut(),
        budget: dispatch.budget.as_deref_mut(),
        caller_roots: dispatch.caller_roots,
        inline_caches: dispatch.inline_caches,
        bytecode_profiler: dispatch.bytecode_profiler,
    };
    if method_id == ids.array_sort {
        return Some(array_methods::sort_with_ordering(
            receiver,
            args,
            &mut runtime,
        ));
    }
    if method_id == ids.array_min {
        return Some(array_methods::min_with_ordering(
            receiver,
            args,
            &mut runtime,
        ));
    }
    if method_id == ids.array_max {
        return Some(array_methods::max_with_ordering(
            receiver,
            args,
            &mut runtime,
        ));
    }
    None
}

fn call_script_impl_method(
    receiver: &Value,
    expected_method_id: MethodId,
    method: &str,
    args: &[Value],
    dispatch: &mut ScriptMethodDispatch<'_, '_, '_>,
) -> VmResult<Value> {
    let function = {
        let owner = receiver_type_id(
            receiver,
            dispatch.heap.as_deref(),
            dispatch.vm.type_registry(),
        )
        .ok_or_else(|| {
            VmError::new(VmErrorKind::UnknownMethod {
                method: method.to_owned(),
            })
        })?;
        let Some(target) = dispatch.program.script_method_dispatch(owner, method) else {
            return Err(VmError::new(VmErrorKind::UnknownMethod {
                method: method.to_owned(),
            }));
        };
        let Some(target) = dispatch.program.method_dispatch(target) else {
            return Err(VmError::new(VmErrorKind::UnknownMethod {
                method: method.to_owned(),
            }));
        };
        let vela_bytecode::linked::LinkedMethodDispatchKind::Script {
            method_id,
            function,
        } = target.kind
        else {
            return Err(VmError::new(VmErrorKind::UnknownMethod {
                method: method.to_owned(),
            }));
        };
        if method_id != expected_method_id {
            return Err(VmError::new(VmErrorKind::UnknownMethod {
                method: method.to_owned(),
            }));
        }
        function
    };

    let values = SmallStorage::try_from_prefix_and_slice_map(*receiver, args, 4, |arg| {
        Ok::<_, VmError>(*arg)
    })?;
    let protected_root_len = dispatch
        .heap
        .as_deref_mut()
        .map(|heap| dispatch.caller_roots.push_to_heap(heap));
    let owner = std::sync::Arc::clone(dispatch.caller_roots.linked_owner().ok_or_else(|| {
        VmError::new(VmErrorKind::UnknownMethod {
            method: method.to_owned(),
        })
    })?);
    let result = dispatch.vm.execute_linked_call(
        crate::linked_execution::LinkedExecutionCall {
            owner,
            function,
            captures: &[],
            args: values.as_slice(),
            check_param_guards: true,
            call_site: None,
            call_site_offset: None,
            inline_caches: dispatch.inline_caches,
            bytecode_profiler: dispatch.bytecode_profiler,
        },
        dispatch.host.as_deref_mut(),
        dispatch.heap.as_deref_mut(),
        dispatch.budget.as_deref_mut(),
    );
    if let (Some(heap), Some(protected_root_len)) =
        (dispatch.heap.as_deref_mut(), protected_root_len)
    {
        heap.truncate_protected_roots(protected_root_len);
    }
    result
}

fn receiver_type_id(
    receiver: &Value,
    heap: Option<&HeapExecution<'_>>,
    registry: Option<&TypeRegistry>,
) -> Option<vela_def::TypeId> {
    match receiver {
        Value::HostRef(reference) => registry
            .and_then(|registry| registry.type_of_host(*reference))
            .map(|desc| desc.key.id),
        Value::HeapRef(reference) => match heap?.heap.get(*reference)? {
            HeapValue::Record {
                identity: Some(identity),
                ..
            } => Some(identity.type_id),
            HeapValue::Enum {
                identity: Some(identity),
                ..
            } => Some(identity.type_id),
            _ => None,
        },
        _ => None,
    }
}
