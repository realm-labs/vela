use vela_bytecode::Program;
use vela_common::MethodId;
use vela_reflect::TypeRegistry;

use crate::array_methods;
use crate::callback_method_dispatch::{self, CallbackMethodDispatch};
use crate::heap::{GcRef, HeapValue};
use crate::map_methods;
use crate::option_result_methods;
use crate::set_methods;
use crate::string_method_dispatch;
use crate::string_methods;
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
    if method == "find" && string_methods::is_string(receiver, heap.as_deref()) {
        return string_methods::find(receiver, args, heap.as_deref());
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

    match method {
        "len" => {
            expect_no_args(method, args)?;
            len(receiver, heap.as_deref()).map(Value::Int)
        }
        "is_empty" => {
            expect_no_args(method, args)?;
            is_empty(receiver, heap.as_deref()).map(Value::Bool)
        }
        "contains" => {
            if string_methods::is_string(receiver, heap.as_deref()) {
                string_methods::contains(receiver, args, heap.as_deref())
            } else {
                array_methods::contains(receiver, args, heap.as_deref())
            }
        }
        .map(Value::Bool),
        "slice" => {
            if string_methods::is_string(receiver, heap.as_deref()) {
                string_methods::slice(receiver, args, heap.as_deref())
            } else {
                array_methods::slice(receiver, args, heap.as_deref())
            }
        }
        "push" => array_methods::push(receiver, args, heap.as_deref_mut(), budget.as_deref_mut()),
        "pop" => array_methods::pop(receiver, args, heap.as_deref_mut()),
        "insert" => {
            array_methods::insert(receiver, args, heap.as_deref_mut(), budget.as_deref_mut())
        }
        "extend" => {
            if set_methods::is_set(receiver, heap.as_deref()) {
                set_methods::extend(receiver, args, heap.as_deref_mut(), budget.as_deref_mut())
            } else if map_methods::is_map(receiver, heap.as_deref()) {
                map_methods::extend(receiver, args, heap.as_deref_mut(), budget.as_deref_mut())
            } else {
                array_methods::extend(receiver, args, heap.as_deref_mut(), budget.as_deref_mut())
            }
        }
        "first" => array_methods::first(receiver, args, heap.as_deref()),
        "last" => array_methods::last(receiver, args, heap.as_deref()),
        "remove_at" => array_methods::remove_at(receiver, args, heap.as_deref_mut()),
        "join" => array_methods::join(receiver, args, heap.as_deref()),
        "index_of" => array_methods::index_of(receiver, args, heap.as_deref()),
        "distinct" => array_methods::distinct(receiver, args, heap.as_deref()),
        "reverse" => array_methods::reverse(receiver, args, heap.as_deref()),
        "sort" => array_methods::sort(receiver, args, heap.as_deref()),
        "min" => array_methods::min(receiver, args, heap.as_deref()),
        "max" => array_methods::max(receiver, args, heap.as_deref()),
        "is_some" => option_result_methods::is_some(receiver, args, heap.as_deref()),
        "is_none" => option_result_methods::is_none(receiver, args, heap.as_deref()),
        "is_ok" => option_result_methods::is_ok(receiver, args, heap.as_deref()),
        "is_err" => option_result_methods::is_err(receiver, args, heap.as_deref()),
        "unwrap_or" => option_result_methods::unwrap_or(receiver, args, heap.as_deref()),
        "ok_or" => option_result_methods::ok_or(receiver, args, heap.as_deref()),
        "to_option" => option_result_methods::to_option(receiver, args, heap.as_deref()),
        "to_error_option" => {
            option_result_methods::to_error_option(receiver, args, heap.as_deref())
        }
        "flatten" => {
            if option_result_methods::is_option_or_result(receiver, heap.as_deref()) {
                option_result_methods::flatten(receiver, args, heap.as_deref())
            } else {
                Err(VmError::new(VmErrorKind::TypeMismatch {
                    operation: "method flatten",
                }))
            }
        }
        "merge" => map_methods::merge(receiver, args, heap.as_deref()),
        "has" => {
            if set_methods::is_set(receiver, heap.as_deref()) {
                set_methods::has(receiver, args, heap.as_deref())
            } else {
                map_methods::has(receiver, args, heap.as_deref())
            }
        }
        .map(Value::Bool),
        "get" => map_methods::get(receiver, args, heap.as_deref()),
        "get_or" => map_methods::get_or(receiver, args, heap.as_deref()),
        "add" => set_methods::add(receiver, args, heap.as_deref_mut(), budget.as_deref_mut()),
        "set" => map_methods::set(receiver, args, heap.as_deref_mut(), budget),
        "remove" => {
            if set_methods::is_set(receiver, heap.as_deref()) {
                set_methods::remove(receiver, args, heap.as_deref_mut())
            } else {
                map_methods::remove(receiver, args, heap.as_deref_mut())
            }
        }
        "clear" => {
            if set_methods::is_set(receiver, heap.as_deref()) {
                set_methods::clear(receiver, args, heap.as_deref_mut())
            } else if map_methods::is_map(receiver, heap.as_deref()) {
                map_methods::clear(receiver, args, heap.as_deref_mut())
            } else {
                array_methods::clear(receiver, args, heap.as_deref_mut())
            }
        }
        "keys" => map_methods::keys(receiver, args, heap.as_deref()),
        "values" => {
            if set_methods::is_set(receiver, heap.as_deref()) {
                set_methods::values(receiver, args, heap.as_deref())
            } else {
                map_methods::values(receiver, args, heap.as_deref())
            }
        }
        "union" => set_methods::union(receiver, args, heap.as_deref()),
        "intersection" => set_methods::intersection(receiver, args, heap.as_deref()),
        "difference" => set_methods::difference(receiver, args, heap.as_deref()),
        "symmetric_difference" => {
            set_methods::symmetric_difference(receiver, args, heap.as_deref())
        }
        "is_subset" => set_methods::is_subset(receiver, args, heap.as_deref()).map(Value::Bool),
        "is_superset" => set_methods::is_superset(receiver, args, heap.as_deref()).map(Value::Bool),
        "is_disjoint" => set_methods::is_disjoint(receiver, args, heap.as_deref()).map(Value::Bool),
        "entries" => map_methods::entries(receiver, args, heap.as_deref()),
        _ => call_script_impl_method(
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
        ),
    }
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

fn len(receiver: &Value, heap: Option<&HeapExecution<'_>>) -> VmResult<i64> {
    match receiver {
        Value::String(value) => usize_to_i64(value.chars().count(), "method len"),
        Value::Array(values) => usize_to_i64(values.len(), "method len"),
        Value::Map(values) => usize_to_i64(values.len(), "method len"),
        Value::Set(values) => usize_to_i64(values.len(), "method len"),
        Value::Range(range) => range.len().ok_or_else(|| {
            VmError::new(VmErrorKind::TypeMismatch {
                operation: "method len",
            })
        }),
        Value::HeapRef(reference) => {
            let Some(value) = heap.and_then(|heap| heap.heap.get(*reference)) else {
                return type_error("method len");
            };
            match value {
                HeapValue::String(value) => usize_to_i64(value.chars().count(), "method len"),
                HeapValue::Array(values) | HeapValue::Set(values) => {
                    usize_to_i64(values.len(), "method len")
                }
                HeapValue::Map(values) => usize_to_i64(values.len(), "method len"),
                HeapValue::Record { fields: values, .. }
                | HeapValue::Enum { fields: values, .. } => {
                    usize_to_i64(values.len(), "method len")
                }
            }
        }
        Value::Record { fields, .. } | Value::Enum { fields, .. } => {
            usize_to_i64(fields.len(), "method len")
        }
        _ => type_error("method len"),
    }
}

fn is_empty(receiver: &Value, heap: Option<&HeapExecution<'_>>) -> VmResult<bool> {
    match receiver {
        Value::String(value) => Ok(value.is_empty()),
        Value::Array(values) => Ok(values.is_empty()),
        Value::Map(values) => Ok(values.is_empty()),
        Value::Set(values) => Ok(values.is_empty()),
        Value::Range(range) => Ok(range.is_empty()),
        Value::HeapRef(reference) => {
            let Some(value) = heap.and_then(|heap| heap.heap.get(*reference)) else {
                return type_error("method is_empty");
            };
            match value {
                HeapValue::String(value) => Ok(value.is_empty()),
                HeapValue::Array(values) | HeapValue::Set(values) => Ok(values.is_empty()),
                HeapValue::Map(values) => Ok(values.is_empty()),
                HeapValue::Record { fields: values, .. }
                | HeapValue::Enum { fields: values, .. } => Ok(values.is_empty()),
            }
        }
        Value::Record { fields, .. } | Value::Enum { fields, .. } => Ok(fields.is_empty()),
        _ => type_error("method is_empty"),
    }
}

fn expect_no_args(method: &str, args: &[Value]) -> VmResult<()> {
    expect_arity(method, args, 0)
}

fn expect_arity(method: &str, args: &[Value], expected: usize) -> VmResult<()> {
    if args.len() == expected {
        return Ok(());
    }
    Err(VmError::new(VmErrorKind::ArityMismatch {
        name: method.to_owned(),
        expected,
        actual: args.len(),
    }))
}

fn usize_to_i64(value: usize, operation: &'static str) -> VmResult<i64> {
    i64::try_from(value).map_err(|_| VmError::new(VmErrorKind::TypeMismatch { operation }))
}

fn type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}
