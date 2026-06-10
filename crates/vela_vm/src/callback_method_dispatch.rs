use vela_bytecode::UnlinkedProgramCode;

use crate::heap::GcRef;
use crate::method_runtime::MethodRuntime;
use crate::{
    ExecutionBudget, HeapExecution, HostExecution, Value, Vm, VmError, VmErrorKind, VmResult,
    array_methods, map_methods, option_result_methods, set_methods,
};

pub(crate) struct CallbackMethodDispatch<'a, 'host, 'heap> {
    pub(crate) vm: &'a Vm,
    pub(crate) program: Option<&'a dyn UnlinkedProgramCode>,
    pub(crate) host: Option<&'a mut HostExecution<'host>>,
    pub(crate) heap: Option<&'a mut HeapExecution<'heap>>,
    pub(crate) budget: Option<&'a mut ExecutionBudget>,
    pub(crate) caller_roots: &'a [GcRef],
}

impl<'a, 'host, 'heap> CallbackMethodDispatch<'a, 'host, 'heap> {
    fn runtime<'dispatch>(&'dispatch mut self) -> MethodRuntime<'dispatch, 'host, 'heap> {
        MethodRuntime {
            vm: self.vm,
            program: self.program,
            host: self.host.as_deref_mut(),
            heap: self.heap.as_deref_mut(),
            budget: self.budget.as_deref_mut(),
            caller_roots: self.caller_roots,
        }
    }

    fn heap_ref(&self) -> Option<&HeapExecution<'heap>> {
        self.heap.as_deref()
    }
}

pub(crate) fn call(
    method: &str,
    receiver: &Value,
    args: &[Value],
    dispatch: &mut CallbackMethodDispatch<'_, '_, '_>,
) -> Option<VmResult<Value>> {
    match method {
        "map" => Some(call_map(receiver, args, dispatch)),
        "map_err" => Some(call_map_err(receiver, args, dispatch)),
        "and_then" => Some(call_and_then(receiver, args, dispatch)),
        "or_else" => Some(call_or_else(receiver, args, dispatch)),
        "filter" => Some(call_filter(receiver, args, dispatch)),
        "find" => Some(call_find(receiver, args, dispatch)),
        "any" => Some(call_any(receiver, args, dispatch).map(Value::Bool)),
        "all" => Some(call_all(receiver, args, dispatch).map(Value::Bool)),
        "count" => Some(call_count(receiver, args, dispatch).map(Value::Int)),
        "sum" => Some(array_methods::sum(receiver, args, dispatch.runtime())),
        "group_by" => Some(array_methods::group_by(receiver, args, dispatch.runtime())),
        "sort_by" => Some(array_methods::sort_by(receiver, args, dispatch.runtime())),
        "map_values" => Some(map_methods::map_values(receiver, args, dispatch.runtime())),
        _ => None,
    }
}

fn call_map(
    receiver: &Value,
    args: &[Value],
    dispatch: &mut CallbackMethodDispatch<'_, '_, '_>,
) -> VmResult<Value> {
    if option_result_methods::is_option_or_result(receiver, dispatch.heap_ref()) {
        option_result_methods::map(receiver, args, dispatch.runtime())
    } else if set_methods::is_set(receiver, dispatch.heap_ref()) {
        set_methods::map(receiver, args, dispatch.runtime())
    } else {
        array_methods::map(receiver, args, dispatch.runtime())
    }
}

fn call_map_err(
    receiver: &Value,
    args: &[Value],
    dispatch: &mut CallbackMethodDispatch<'_, '_, '_>,
) -> VmResult<Value> {
    if option_result_methods::is_result(receiver, dispatch.heap_ref()) {
        option_result_methods::map_err(receiver, args, dispatch.runtime())
    } else {
        type_error("method map_err")
    }
}

fn call_and_then(
    receiver: &Value,
    args: &[Value],
    dispatch: &mut CallbackMethodDispatch<'_, '_, '_>,
) -> VmResult<Value> {
    if option_result_methods::is_option_or_result(receiver, dispatch.heap_ref()) {
        option_result_methods::and_then(receiver, args, dispatch.runtime())
    } else {
        type_error("method and_then")
    }
}

fn call_or_else(
    receiver: &Value,
    args: &[Value],
    dispatch: &mut CallbackMethodDispatch<'_, '_, '_>,
) -> VmResult<Value> {
    if option_result_methods::is_option_or_result(receiver, dispatch.heap_ref()) {
        option_result_methods::or_else(receiver, args, dispatch.runtime())
    } else {
        type_error("method or_else")
    }
}

fn call_filter(
    receiver: &Value,
    args: &[Value],
    dispatch: &mut CallbackMethodDispatch<'_, '_, '_>,
) -> VmResult<Value> {
    if option_result_methods::is_option(receiver, dispatch.heap_ref()) {
        option_result_methods::filter(receiver, args, dispatch.runtime())
    } else if set_methods::is_set(receiver, dispatch.heap_ref()) {
        set_methods::filter(receiver, args, dispatch.runtime())
    } else if map_methods::is_map(receiver, dispatch.heap_ref()) {
        map_methods::filter(receiver, args, dispatch.runtime())
    } else {
        array_methods::filter(receiver, args, dispatch.runtime())
    }
}

fn call_find(
    receiver: &Value,
    args: &[Value],
    dispatch: &mut CallbackMethodDispatch<'_, '_, '_>,
) -> VmResult<Value> {
    if set_methods::is_set(receiver, dispatch.heap_ref()) {
        set_methods::find(receiver, args, dispatch.runtime())
    } else if map_methods::is_map(receiver, dispatch.heap_ref()) {
        map_methods::find(receiver, args, dispatch.runtime())
    } else {
        array_methods::find(receiver, args, dispatch.runtime())
    }
}

fn call_any(
    receiver: &Value,
    args: &[Value],
    dispatch: &mut CallbackMethodDispatch<'_, '_, '_>,
) -> VmResult<bool> {
    if set_methods::is_set(receiver, dispatch.heap_ref()) {
        set_methods::any(receiver, args, dispatch.runtime())
    } else if map_methods::is_map(receiver, dispatch.heap_ref()) {
        map_methods::any(receiver, args, dispatch.runtime())
    } else {
        array_methods::any(receiver, args, dispatch.runtime())
    }
}

fn call_all(
    receiver: &Value,
    args: &[Value],
    dispatch: &mut CallbackMethodDispatch<'_, '_, '_>,
) -> VmResult<bool> {
    if set_methods::is_set(receiver, dispatch.heap_ref()) {
        set_methods::all(receiver, args, dispatch.runtime())
    } else if map_methods::is_map(receiver, dispatch.heap_ref()) {
        map_methods::all(receiver, args, dispatch.runtime())
    } else {
        array_methods::all(receiver, args, dispatch.runtime())
    }
}

fn call_count(
    receiver: &Value,
    args: &[Value],
    dispatch: &mut CallbackMethodDispatch<'_, '_, '_>,
) -> VmResult<i64> {
    if set_methods::is_set(receiver, dispatch.heap_ref()) {
        set_methods::count(receiver, args, dispatch.runtime())
    } else if map_methods::is_map(receiver, dispatch.heap_ref()) {
        map_methods::count(receiver, args, dispatch.runtime())
    } else {
        array_methods::count(receiver, args, dispatch.runtime())
    }
}

fn type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}
