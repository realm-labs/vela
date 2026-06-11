use std::sync::OnceLock;

use vela_bytecode::{LinkedProgram, UnlinkedProgramCode};
use vela_def::MethodId;

use crate::heap::GcRef;
use crate::method_runtime::MethodRuntime;
use crate::{
    ExecutionBudget, HeapExecution, HostExecution, Value, Vm, VmError, VmErrorKind, VmResult,
    array_methods, map_methods, option_result_methods, set_methods,
};

pub(crate) struct CallbackMethodDispatch<'a, 'host, 'heap> {
    pub(crate) vm: &'a Vm,
    pub(crate) program: Option<&'a dyn UnlinkedProgramCode>,
    pub(crate) linked_program: Option<&'a LinkedProgram>,
    pub(crate) host: Option<&'a mut HostExecution<'host>>,
    pub(crate) heap: Option<&'a mut HeapExecution<'heap>>,
    pub(crate) budget: Option<&'a mut ExecutionBudget>,
    pub(crate) caller_roots: &'a [GcRef],
}

#[derive(Clone, Copy)]
struct CallbackMethodIds {
    array_map: MethodId,
    array_filter: MethodId,
    array_find: MethodId,
    array_any: MethodId,
    array_all: MethodId,
    array_count: MethodId,
    array_sum: MethodId,
    array_group_by: MethodId,
    array_sort_by: MethodId,
    map_filter: MethodId,
    map_find: MethodId,
    map_any: MethodId,
    map_all: MethodId,
    map_count: MethodId,
    map_map_values: MethodId,
    set_map: MethodId,
    set_filter: MethodId,
    set_find: MethodId,
    set_any: MethodId,
    set_all: MethodId,
    set_count: MethodId,
    option_map: MethodId,
    option_and_then: MethodId,
    option_or_else: MethodId,
    option_filter: MethodId,
    result_map: MethodId,
    result_map_err: MethodId,
    result_and_then: MethodId,
    result_or_else: MethodId,
}

impl CallbackMethodIds {
    fn new() -> Self {
        Self {
            array_map: standard_method_id("Array", "map"),
            array_filter: standard_method_id("Array", "filter"),
            array_find: standard_method_id("Array", "find"),
            array_any: standard_method_id("Array", "any"),
            array_all: standard_method_id("Array", "all"),
            array_count: standard_method_id("Array", "count"),
            array_sum: standard_method_id("Array", "sum"),
            array_group_by: standard_method_id("Array", "group_by"),
            array_sort_by: standard_method_id("Array", "sort_by"),
            map_filter: standard_method_id("Map", "filter"),
            map_find: standard_method_id("Map", "find"),
            map_any: standard_method_id("Map", "any"),
            map_all: standard_method_id("Map", "all"),
            map_count: standard_method_id("Map", "count"),
            map_map_values: standard_method_id("Map", "map_values"),
            set_map: standard_method_id("Set", "map"),
            set_filter: standard_method_id("Set", "filter"),
            set_find: standard_method_id("Set", "find"),
            set_any: standard_method_id("Set", "any"),
            set_all: standard_method_id("Set", "all"),
            set_count: standard_method_id("Set", "count"),
            option_map: standard_method_id("Option", "map"),
            option_and_then: standard_method_id("Option", "and_then"),
            option_or_else: standard_method_id("Option", "or_else"),
            option_filter: standard_method_id("Option", "filter"),
            result_map: standard_method_id("Result", "map"),
            result_map_err: standard_method_id("Result", "map_err"),
            result_and_then: standard_method_id("Result", "and_then"),
            result_or_else: standard_method_id("Result", "or_else"),
        }
    }
}

fn callback_method_ids() -> &'static CallbackMethodIds {
    static IDS: OnceLock<CallbackMethodIds> = OnceLock::new();
    IDS.get_or_init(CallbackMethodIds::new)
}

fn standard_method_id(owner: &str, name: &str) -> MethodId {
    let Some(id) = vela_stdlib::std_method_id(owner, name) else {
        panic!("missing standard method identity for {owner}::{name}");
    };
    id
}

impl<'a, 'host, 'heap> CallbackMethodDispatch<'a, 'host, 'heap> {
    fn runtime<'dispatch>(&'dispatch mut self) -> MethodRuntime<'dispatch, 'host, 'heap> {
        MethodRuntime {
            vm: self.vm,
            program: self.program,
            linked_program: self.linked_program,
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
        "count" => Some(call_count(receiver, args, dispatch).map(Value::i64)),
        "sum" => Some(array_methods::sum(receiver, args, dispatch.runtime())),
        "group_by" => Some(array_methods::group_by(receiver, args, dispatch.runtime())),
        "sort_by" => Some(array_methods::sort_by(receiver, args, dispatch.runtime())),
        "map_values" => Some(map_methods::map_values(receiver, args, dispatch.runtime())),
        _ => None,
    }
}

pub(crate) fn call_by_id(
    method_id: MethodId,
    receiver: &Value,
    args: &[Value],
    dispatch: &mut CallbackMethodDispatch<'_, '_, '_>,
) -> Option<VmResult<Value>> {
    let ids = callback_method_ids();
    if method_id == ids.array_map && array_methods::is_array(receiver, dispatch.heap_ref()) {
        return Some(array_methods::map(receiver, args, dispatch.runtime()));
    }
    if method_id == ids.set_map && set_methods::is_set(receiver, dispatch.heap_ref()) {
        return Some(set_methods::map(receiver, args, dispatch.runtime()));
    }
    if method_id == ids.option_map
        && option_result_methods::is_option(receiver, dispatch.heap_ref())
    {
        return Some(option_result_methods::map(
            receiver,
            args,
            dispatch.runtime(),
        ));
    }
    if method_id == ids.result_map
        && option_result_methods::is_result(receiver, dispatch.heap_ref())
    {
        return Some(option_result_methods::map(
            receiver,
            args,
            dispatch.runtime(),
        ));
    }
    if method_id == ids.result_map_err
        && option_result_methods::is_result(receiver, dispatch.heap_ref())
    {
        return Some(option_result_methods::map_err(
            receiver,
            args,
            dispatch.runtime(),
        ));
    }
    if method_id == ids.option_and_then
        && option_result_methods::is_option(receiver, dispatch.heap_ref())
    {
        return Some(option_result_methods::and_then(
            receiver,
            args,
            dispatch.runtime(),
        ));
    }
    if method_id == ids.result_and_then
        && option_result_methods::is_result(receiver, dispatch.heap_ref())
    {
        return Some(option_result_methods::and_then(
            receiver,
            args,
            dispatch.runtime(),
        ));
    }
    if method_id == ids.option_or_else
        && option_result_methods::is_option(receiver, dispatch.heap_ref())
    {
        return Some(option_result_methods::or_else(
            receiver,
            args,
            dispatch.runtime(),
        ));
    }
    if method_id == ids.result_or_else
        && option_result_methods::is_result(receiver, dispatch.heap_ref())
    {
        return Some(option_result_methods::or_else(
            receiver,
            args,
            dispatch.runtime(),
        ));
    }
    if method_id == ids.array_filter && array_methods::is_array(receiver, dispatch.heap_ref()) {
        return Some(array_methods::filter(receiver, args, dispatch.runtime()));
    }
    if method_id == ids.map_filter && map_methods::is_map(receiver, dispatch.heap_ref()) {
        return Some(map_methods::filter(receiver, args, dispatch.runtime()));
    }
    if method_id == ids.set_filter && set_methods::is_set(receiver, dispatch.heap_ref()) {
        return Some(set_methods::filter(receiver, args, dispatch.runtime()));
    }
    if method_id == ids.option_filter
        && option_result_methods::is_option(receiver, dispatch.heap_ref())
    {
        return Some(option_result_methods::filter(
            receiver,
            args,
            dispatch.runtime(),
        ));
    }
    if method_id == ids.array_find && array_methods::is_array(receiver, dispatch.heap_ref()) {
        return Some(array_methods::find(receiver, args, dispatch.runtime()));
    }
    if method_id == ids.map_find && map_methods::is_map(receiver, dispatch.heap_ref()) {
        return Some(map_methods::find(receiver, args, dispatch.runtime()));
    }
    if method_id == ids.set_find && set_methods::is_set(receiver, dispatch.heap_ref()) {
        return Some(set_methods::find(receiver, args, dispatch.runtime()));
    }
    if method_id == ids.array_any && array_methods::is_array(receiver, dispatch.heap_ref()) {
        return Some(array_methods::any(receiver, args, dispatch.runtime()).map(Value::Bool));
    }
    if method_id == ids.map_any && map_methods::is_map(receiver, dispatch.heap_ref()) {
        return Some(map_methods::any(receiver, args, dispatch.runtime()).map(Value::Bool));
    }
    if method_id == ids.set_any && set_methods::is_set(receiver, dispatch.heap_ref()) {
        return Some(set_methods::any(receiver, args, dispatch.runtime()).map(Value::Bool));
    }
    if method_id == ids.array_all && array_methods::is_array(receiver, dispatch.heap_ref()) {
        return Some(array_methods::all(receiver, args, dispatch.runtime()).map(Value::Bool));
    }
    if method_id == ids.map_all && map_methods::is_map(receiver, dispatch.heap_ref()) {
        return Some(map_methods::all(receiver, args, dispatch.runtime()).map(Value::Bool));
    }
    if method_id == ids.set_all && set_methods::is_set(receiver, dispatch.heap_ref()) {
        return Some(set_methods::all(receiver, args, dispatch.runtime()).map(Value::Bool));
    }
    if method_id == ids.array_count && array_methods::is_array(receiver, dispatch.heap_ref()) {
        return Some(array_methods::count(receiver, args, dispatch.runtime()).map(Value::i64));
    }
    if method_id == ids.map_count && map_methods::is_map(receiver, dispatch.heap_ref()) {
        return Some(map_methods::count(receiver, args, dispatch.runtime()).map(Value::i64));
    }
    if method_id == ids.set_count && set_methods::is_set(receiver, dispatch.heap_ref()) {
        return Some(set_methods::count(receiver, args, dispatch.runtime()).map(Value::i64));
    }
    if method_id == ids.array_sum && array_methods::is_array(receiver, dispatch.heap_ref()) {
        return Some(array_methods::sum(receiver, args, dispatch.runtime()));
    }
    if method_id == ids.array_group_by && array_methods::is_array(receiver, dispatch.heap_ref()) {
        return Some(array_methods::group_by(receiver, args, dispatch.runtime()));
    }
    if method_id == ids.array_sort_by && array_methods::is_array(receiver, dispatch.heap_ref()) {
        return Some(array_methods::sort_by(receiver, args, dispatch.runtime()));
    }
    if method_id == ids.map_map_values && map_methods::is_map(receiver, dispatch.heap_ref()) {
        return Some(map_methods::map_values(receiver, args, dispatch.runtime()));
    }
    None
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
