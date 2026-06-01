use vela_bytecode::Program;
use vela_common::MethodId;
use vela_reflect::TypeRegistry;

use crate::callback_method_dispatch::{self, CallbackMethodDispatch};
use crate::heap::{GcRef, HeapValue};
use crate::script_builtin_methods;
use crate::string_method_dispatch;
use crate::{
    ExecutionBudget, HeapExecution, HostExecution, Value, Vm, VmError, VmErrorKind, VmResult,
};

#[allow(clippy::too_many_arguments)]
pub(crate) fn call_method(
    receiver: &mut Value,
    method: &str,
    args: &[Value],
    vm: &Vm,
    program: Option<&Program>,
    mut host: Option<&mut HostExecution<'_>>,
    mut heap: Option<&mut HeapExecution<'_>>,
    mut budget: Option<&mut ExecutionBudget>,
    caller_roots: Vec<GcRef>,
) -> VmResult<Value> {
    if let Some(result) = string_method_dispatch::call(method, receiver, args, heap.as_deref()) {
        return result;
    }
    if let Some(result) = callback_method_dispatch::call(
        method,
        receiver,
        args,
        &mut CallbackMethodDispatch {
            vm,
            program,
            host: host.as_deref_mut(),
            heap: heap.as_deref_mut(),
            budget: budget.as_deref_mut(),
            caller_roots: &caller_roots,
        },
    ) {
        return result;
    }

    if let Some(result) =
        script_builtin_methods::call(receiver, method, args, &mut heap, &mut budget)
    {
        return result;
    }

    call_script_impl_method(
        receiver,
        ScriptMethodLookup::Name(method),
        method,
        args,
        vm,
        program,
        host,
        heap,
        budget,
        &caller_roots,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn call_method_id(
    receiver: &Value,
    method: &str,
    method_id: MethodId,
    args: &[Value],
    vm: &Vm,
    program: Option<&Program>,
    host: Option<&mut HostExecution<'_>>,
    heap: Option<&mut HeapExecution<'_>>,
    budget: Option<&mut ExecutionBudget>,
    caller_roots: Vec<GcRef>,
) -> VmResult<Value> {
    call_script_impl_method(
        receiver,
        ScriptMethodLookup::Id(method_id),
        method,
        args,
        vm,
        program,
        host,
        heap,
        budget,
        &caller_roots,
    )
}

#[allow(clippy::too_many_arguments)]
fn call_script_impl_method(
    receiver: &Value,
    lookup: ScriptMethodLookup<'_>,
    method: &str,
    args: &[Value],
    vm: &Vm,
    program: Option<&Program>,
    host: Option<&mut HostExecution<'_>>,
    mut heap: Option<&mut HeapExecution<'_>>,
    budget: Option<&mut ExecutionBudget>,
    caller_roots: &[GcRef],
) -> VmResult<Value> {
    let type_name =
        receiver_type_name(receiver, heap.as_deref(), vm.type_registry()).ok_or_else(|| {
            VmError::new(VmErrorKind::UnknownMethod {
                method: method.to_owned(),
            })
        })?;
    let Some(function) = program.and_then(|program| match lookup {
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
    let protected_root_len = heap
        .as_deref_mut()
        .map(|heap| heap.push_protected_roots(caller_roots.to_vec()));
    let result = vm.execute_code_object(
        function,
        program,
        &values,
        host,
        heap.as_deref_mut(),
        budget,
    );
    if let (Some(heap), Some(protected_root_len)) = (heap, protected_root_len) {
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
