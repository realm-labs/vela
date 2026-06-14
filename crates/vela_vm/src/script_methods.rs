use vela_bytecode::{LinkedProgram, UnlinkedProgramCode};
use vela_def::MethodId;
use vela_reflect::registry::TypeRegistry;

use crate::callback_method_dispatch::{self, CallbackMethodDispatch};
use crate::heap::HeapValue;
use crate::method_runtime::CallerRoots;
use crate::script_builtin_methods;
use crate::std_method_ids::std_method_ids;
use crate::string_method_dispatch;
use crate::{
    EqualityRuntime, ExecutionBudget, HeapExecution, HostExecution, SmallStorage, Value, Vm,
    VmBytecodeProfiler, VmError, VmErrorKind, VmInlineCaches, VmResult, array_methods,
};

pub(crate) struct ScriptMethodDispatch<'a, 'host, 'heap> {
    pub(crate) vm: &'a Vm,
    pub(crate) program: Option<&'a dyn UnlinkedProgramCode>,
    pub(crate) linked_program: Option<&'a LinkedProgram>,
    pub(crate) host: Option<&'a mut HostExecution<'host>>,
    pub(crate) heap: Option<&'a mut HeapExecution<'heap>>,
    pub(crate) budget: Option<&'a mut ExecutionBudget>,
    pub(crate) caller_roots: CallerRoots<'a>,
    pub(crate) inline_caches: Option<&'a dyn VmInlineCaches>,
    pub(crate) bytecode_profiler: Option<&'a dyn VmBytecodeProfiler>,
}

pub(crate) fn call_method(
    receiver: &mut Value,
    method: &str,
    value_method_id: Option<MethodId>,
    args: &[Value],
    mut dispatch: ScriptMethodDispatch<'_, '_, '_>,
) -> VmResult<Value> {
    if let Some(result) = contextual_array_method_by_name(receiver, method, args, &mut dispatch) {
        return result;
    }
    if let Some(result) = value_method_id.and_then(|method_id| {
        script_builtin_methods::call_by_id(
            receiver,
            method_id,
            args,
            &mut dispatch.heap,
            &mut dispatch.budget,
        )
    }) {
        return result;
    }
    if let Some(result) = string_method_dispatch::call(
        method,
        receiver,
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
            linked_program: dispatch.linked_program,
            host: dispatch.host.as_deref_mut(),
            heap: dispatch.heap.as_deref_mut(),
            budget: dispatch.budget.as_deref_mut(),
            caller_roots: dispatch.caller_roots,
            inline_caches: dispatch.inline_caches,
            bytecode_profiler: dispatch.bytecode_profiler,
        };
        if let Some(result) =
            callback_method_dispatch::call(method, receiver, args, &mut callback_dispatch)
        {
            return result;
        }
    }

    if let Some(result) = script_builtin_methods::call(
        receiver,
        method,
        args,
        &mut dispatch.heap,
        &mut dispatch.budget,
    ) {
        return result;
    }

    call_script_impl_method(
        receiver,
        ScriptMethodLookup::Name(method),
        method,
        args,
        &mut dispatch,
    )
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
            linked_program: dispatch.linked_program,
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
    call_script_impl_method(
        receiver,
        ScriptMethodLookup::Id(method_id),
        method,
        args,
        &mut dispatch,
    )
}

pub(crate) fn call_readonly_method_without_callbacks(
    receiver: &Value,
    method: &str,
    value_method_id: Option<MethodId>,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> Option<VmResult<Value>> {
    if method == "contains" && array_methods::is_array(receiver, heap) {
        return None;
    }
    if let Some(result) = value_method_id.and_then(|method_id| {
        script_builtin_methods::call_readonly_by_id(receiver, method_id, args, heap)
    }) {
        return Some(result);
    }
    if let Some(result) = string_method_dispatch::call_readonly(method, receiver, args, heap) {
        return Some(result);
    }

    script_builtin_methods::call_readonly(receiver, method, args, heap)
}

pub(crate) fn call_non_mutating_method(
    receiver: &Value,
    method: &str,
    value_method_id: Option<MethodId>,
    args: &[Value],
    mut dispatch: ScriptMethodDispatch<'_, '_, '_>,
) -> Option<VmResult<Value>> {
    if let Some(result) = contextual_array_method_by_name(receiver, method, args, &mut dispatch) {
        return Some(result);
    }
    if let Some(result) = value_method_id.and_then(|method_id| {
        script_builtin_methods::call_readonly_by_id(
            receiver,
            method_id,
            args,
            dispatch.heap.as_deref(),
        )
    }) {
        return Some(result);
    }
    if let Some(result) = string_method_dispatch::call(
        method,
        receiver,
        args,
        &mut dispatch.heap,
        &mut dispatch.budget,
    ) {
        return Some(result);
    }
    {
        let mut callback_dispatch = CallbackMethodDispatch {
            vm: dispatch.vm,
            program: dispatch.program,
            linked_program: dispatch.linked_program,
            host: dispatch.host.as_deref_mut(),
            heap: dispatch.heap.as_deref_mut(),
            budget: dispatch.budget.as_deref_mut(),
            caller_roots: dispatch.caller_roots,
            inline_caches: dispatch.inline_caches,
            bytecode_profiler: dispatch.bytecode_profiler,
        };
        if let Some(result) =
            callback_method_dispatch::call(method, receiver, args, &mut callback_dispatch)
        {
            return Some(result);
        }
    }

    script_builtin_methods::call_readonly(receiver, method, args, dispatch.heap.as_deref())
}

fn contextual_array_method_by_name(
    receiver: &Value,
    method: &str,
    args: &[Value],
    dispatch: &mut ScriptMethodDispatch<'_, '_, '_>,
) -> Option<VmResult<Value>> {
    let ids = std_method_ids();
    let method_id = match method {
        "contains" => ids.array_contains,
        "index_of" => ids.array_index_of,
        "distinct" => ids.array_distinct,
        _ => return None,
    };
    contextual_array_method_by_id(receiver, method_id, args, dispatch)
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
    let mut runtime = EqualityRuntime {
        vm: dispatch.vm,
        program: dispatch.program,
        linked_program: dispatch.linked_program,
        host: dispatch.host.as_deref_mut(),
        heap: dispatch.heap.as_deref_mut(),
        budget: dispatch.budget.as_deref_mut(),
        caller_roots: dispatch.caller_roots,
        inline_caches: dispatch.inline_caches,
        bytecode_profiler: dispatch.bytecode_profiler,
    };
    if method_id == ids.array_contains {
        return Some(
            array_methods::contains_with_equality(receiver, args, &mut runtime).map(Value::Bool),
        );
    }
    if method_id == ids.array_index_of {
        return Some(array_methods::index_of_with_equality(
            receiver,
            args,
            &mut runtime,
        ));
    }
    if method_id == ids.array_distinct {
        return Some(array_methods::distinct_with_equality(
            receiver,
            args,
            &mut runtime,
        ));
    }
    None
}

fn call_script_impl_method(
    receiver: &Value,
    lookup: ScriptMethodLookup<'_>,
    method: &str,
    args: &[Value],
    dispatch: &mut ScriptMethodDispatch<'_, '_, '_>,
) -> VmResult<Value> {
    let function = {
        let type_name = receiver_type_name(
            receiver,
            dispatch.heap.as_deref(),
            dispatch.vm.type_registry(),
        )
        .ok_or_else(|| {
            VmError::new(VmErrorKind::UnknownMethod {
                method: method.to_owned(),
            })
        })?;
        let Some(function) = dispatch.program.and_then(|program| match lookup {
            ScriptMethodLookup::Name(name) => program.script_method(type_name, name),
            ScriptMethodLookup::Id(method_id) => program.script_method_by_id(type_name, method_id),
        }) else {
            return Err(VmError::new(VmErrorKind::UnknownMethod {
                method: method.to_owned(),
            }));
        };
        function
    };

    let values = SmallStorage::try_from_prefix_and_slice_map(*receiver, args, 4, |arg| {
        Ok::<_, VmError>(*arg)
    })?;
    let protected_root_len = dispatch
        .heap
        .as_deref_mut()
        .map(|heap| dispatch.caller_roots.push_to_heap(heap));
    let result = dispatch.vm.execute_code_object(
        function,
        dispatch.program,
        values.as_slice(),
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScriptMethodLookup<'a> {
    Name(&'a str),
    Id(MethodId),
}

fn receiver_type_name<'a>(
    receiver: &Value,
    heap: Option<&'a HeapExecution<'_>>,
    registry: Option<&'a TypeRegistry>,
) -> Option<&'a str> {
    match receiver {
        Value::HostRef(reference) => registry
            .and_then(|registry| registry.type_of_host(*reference))
            .map(|desc| desc.key.name.as_str()),
        Value::HeapRef(reference) => match heap?.heap.get(*reference)? {
            HeapValue::Record { type_name, .. } => Some(type_name.as_str()),
            HeapValue::Enum { enum_name, .. } => Some(enum_name.as_str()),
            _ => None,
        },
        _ => None,
    }
}
