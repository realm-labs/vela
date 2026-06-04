use vela_bytecode::Program;
use vela_common::MethodId;
use vela_reflect::registry::TypeRegistry;

use crate::callback_method_dispatch::{self, CallbackMethodDispatch};
use crate::heap::{GcRef, HeapValue};
use crate::script_builtin_methods;
use crate::string_method_dispatch;
use crate::{
    ExecutionBudget, HeapExecution, HostExecution, Value, Vm, VmError, VmErrorKind, VmResult,
};

pub(crate) struct ScriptMethodDispatch<'a, 'host, 'heap> {
    pub(crate) vm: &'a Vm,
    pub(crate) program: Option<&'a Program>,
    pub(crate) host: Option<&'a mut HostExecution<'host>>,
    pub(crate) heap: Option<&'a mut HeapExecution<'heap>>,
    pub(crate) budget: Option<&'a mut ExecutionBudget>,
    pub(crate) caller_roots: Vec<GcRef>,
}

pub(crate) fn call_method(
    receiver: &mut Value,
    method: &str,
    args: &[Value],
    mut dispatch: ScriptMethodDispatch<'_, '_, '_>,
) -> VmResult<Value> {
    if let Some(result) =
        string_method_dispatch::call(method, receiver, args, dispatch.heap.as_deref())
    {
        return result;
    }
    {
        let mut callback_dispatch = CallbackMethodDispatch {
            vm: dispatch.vm,
            program: dispatch.program,
            host: dispatch.host.as_deref_mut(),
            heap: dispatch.heap.as_deref_mut(),
            budget: dispatch.budget.as_deref_mut(),
            caller_roots: &dispatch.caller_roots,
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
    receiver: &Value,
    method: &str,
    method_id: MethodId,
    args: &[Value],
    mut dispatch: ScriptMethodDispatch<'_, '_, '_>,
) -> VmResult<Value> {
    call_script_impl_method(
        receiver,
        ScriptMethodLookup::Id(method_id),
        method,
        args,
        &mut dispatch,
    )
}

pub(crate) fn call_non_mutating_method(
    receiver: &Value,
    method: &str,
    args: &[Value],
    mut dispatch: ScriptMethodDispatch<'_, '_, '_>,
) -> Option<VmResult<Value>> {
    if let Some(result) =
        string_method_dispatch::call(method, receiver, args, dispatch.heap.as_deref())
    {
        return Some(result);
    }
    {
        let mut callback_dispatch = CallbackMethodDispatch {
            vm: dispatch.vm,
            program: dispatch.program,
            host: dispatch.host.as_deref_mut(),
            heap: dispatch.heap.as_deref_mut(),
            budget: dispatch.budget.as_deref_mut(),
            caller_roots: &dispatch.caller_roots,
        };
        if let Some(result) =
            callback_method_dispatch::call(method, receiver, args, &mut callback_dispatch)
        {
            return Some(result);
        }
    }

    script_builtin_methods::call_readonly(receiver, method, args, dispatch.heap.as_deref())
}

fn call_script_impl_method(
    receiver: &Value,
    lookup: ScriptMethodLookup<'_>,
    method: &str,
    args: &[Value],
    dispatch: &mut ScriptMethodDispatch<'_, '_, '_>,
) -> VmResult<Value> {
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
        ScriptMethodLookup::Name(name) => program.script_method(&type_name, name),
        ScriptMethodLookup::Id(method_id) => program.script_method_by_id(&type_name, method_id),
    }) else {
        return Err(VmError::new(VmErrorKind::UnknownMethod {
            method: method.to_owned(),
        }));
    };

    let mut values = Vec::with_capacity(args.len() + 1);
    values.push(receiver.clone());
    values.extend(args.iter().cloned());
    let protected_root_len = dispatch
        .heap
        .as_deref_mut()
        .map(|heap| heap.push_protected_roots(&dispatch.caller_roots));
    let result = dispatch.vm.execute_code_object(
        function,
        dispatch.program,
        &values,
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

fn receiver_type_name(
    receiver: &Value,
    heap: Option<&HeapExecution<'_>>,
    registry: Option<&TypeRegistry>,
) -> Option<String> {
    match receiver {
        Value::Record { type_name, .. } => Some(type_name.clone()),
        Value::Enum { enum_name, .. } => Some(enum_name.clone()),
        Value::HostRef(reference) => registry
            .and_then(|registry| registry.type_of_host(*reference))
            .map(|desc| desc.key.name.clone()),
        Value::HeapRef(reference) => match heap?.heap.get(*reference)? {
            HeapValue::Record { type_name, .. } => Some(type_name.clone()),
            HeapValue::Enum { enum_name, .. } => Some(enum_name.clone()),
            _ => None,
        },
        _ => None,
    }
}
