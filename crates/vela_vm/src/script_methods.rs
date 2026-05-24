use std::collections::BTreeMap;

use vela_bytecode::Program;

use crate::array_methods::{self, MethodRuntime};
use crate::heap::{GcRef, HeapValue};
use crate::map_methods;
use crate::{
    ExecutionBudget, HeapExecution, HostExecution, Value, Vm, VmError, VmErrorKind, VmResult,
    value_from_heap_slot, value_to_heap_slot,
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
    match method {
        "len" => {
            expect_no_args(method, args)?;
            len(receiver, heap.as_deref()).map(Value::Int)
        }
        "is_empty" => {
            expect_no_args(method, args)?;
            is_empty(receiver, heap.as_deref()).map(Value::Bool)
        }
        "contains" => string_predicate(
            receiver,
            method,
            "method contains",
            args,
            heap.as_deref(),
            |value, needle| value.contains(needle),
        )
        .map(Value::Bool),
        "starts_with" => string_predicate(
            receiver,
            method,
            "method starts_with",
            args,
            heap.as_deref(),
            |value, prefix| value.starts_with(prefix),
        )
        .map(Value::Bool),
        "ends_with" => string_predicate(
            receiver,
            method,
            "method ends_with",
            args,
            heap.as_deref(),
            |value, suffix| value.ends_with(suffix),
        )
        .map(Value::Bool),
        "push" => array_push(receiver, args, heap.as_deref_mut(), budget.as_deref_mut()),
        "pop" => array_pop(receiver, args, heap.as_deref_mut()),
        "map" => array_methods::map(
            receiver,
            args,
            MethodRuntime {
                vm,
                program,
                host,
                heap: heap.as_deref_mut(),
                budget: budget.as_deref_mut(),
                caller_roots: &caller_roots,
            },
        ),
        "filter" => {
            if map_methods::is_map(receiver, heap.as_deref()) {
                map_methods::filter(
                    receiver,
                    args,
                    MethodRuntime {
                        vm,
                        program,
                        host,
                        heap: heap.as_deref_mut(),
                        budget: budget.as_deref_mut(),
                        caller_roots: &caller_roots,
                    },
                )
            } else {
                array_methods::filter(
                    receiver,
                    args,
                    MethodRuntime {
                        vm,
                        program,
                        host,
                        heap: heap.as_deref_mut(),
                        budget: budget.as_deref_mut(),
                        caller_roots: &caller_roots,
                    },
                )
            }
        }
        "find" => array_methods::find(
            receiver,
            args,
            MethodRuntime {
                vm,
                program,
                host: host.as_deref_mut(),
                heap: heap.as_deref_mut(),
                budget: budget.as_deref_mut(),
                caller_roots: &caller_roots,
            },
        ),
        "any" => {
            if map_methods::is_map(receiver, heap.as_deref()) {
                map_methods::any(
                    receiver,
                    args,
                    MethodRuntime {
                        vm,
                        program,
                        host: host.as_deref_mut(),
                        heap: heap.as_deref_mut(),
                        budget: budget.as_deref_mut(),
                        caller_roots: &caller_roots,
                    },
                )
            } else {
                array_methods::any(
                    receiver,
                    args,
                    MethodRuntime {
                        vm,
                        program,
                        host: host.as_deref_mut(),
                        heap: heap.as_deref_mut(),
                        budget: budget.as_deref_mut(),
                        caller_roots: &caller_roots,
                    },
                )
            }
        }
        .map(Value::Bool),
        "all" => {
            if map_methods::is_map(receiver, heap.as_deref()) {
                map_methods::all(
                    receiver,
                    args,
                    MethodRuntime {
                        vm,
                        program,
                        host: host.as_deref_mut(),
                        heap: heap.as_deref_mut(),
                        budget: budget.as_deref_mut(),
                        caller_roots: &caller_roots,
                    },
                )
            } else {
                array_methods::all(
                    receiver,
                    args,
                    MethodRuntime {
                        vm,
                        program,
                        host: host.as_deref_mut(),
                        heap: heap.as_deref_mut(),
                        budget: budget.as_deref_mut(),
                        caller_roots: &caller_roots,
                    },
                )
            }
        }
        .map(Value::Bool),
        "count" => {
            if map_methods::is_map(receiver, heap.as_deref()) {
                map_methods::count(
                    receiver,
                    args,
                    MethodRuntime {
                        vm,
                        program,
                        host,
                        heap: heap.as_deref_mut(),
                        budget: budget.as_deref_mut(),
                        caller_roots: &caller_roots,
                    },
                )
            } else {
                array_methods::count(
                    receiver,
                    args,
                    MethodRuntime {
                        vm,
                        program,
                        host,
                        heap: heap.as_deref_mut(),
                        budget: budget.as_deref_mut(),
                        caller_roots: &caller_roots,
                    },
                )
            }
        }
        .map(Value::Int),
        "sum" => array_methods::sum(
            receiver,
            args,
            MethodRuntime {
                vm,
                program,
                host,
                heap: heap.as_deref_mut(),
                budget: budget.as_deref_mut(),
                caller_roots: &caller_roots,
            },
        ),
        "group_by" => array_methods::group_by(
            receiver,
            args,
            MethodRuntime {
                vm,
                program,
                host,
                heap: heap.as_deref_mut(),
                budget: budget.as_deref_mut(),
                caller_roots: &caller_roots,
            },
        ),
        "map_values" => map_methods::map_values(
            receiver,
            args,
            MethodRuntime {
                vm,
                program,
                host,
                heap: heap.as_deref_mut(),
                budget: budget.as_deref_mut(),
                caller_roots: &caller_roots,
            },
        ),
        "has" => map_has(receiver, args, heap.as_deref()).map(Value::Bool),
        "get" => map_get(receiver, args, heap.as_deref()),
        "get_or" => map_get_or(receiver, args, heap.as_deref()),
        "set" => map_set(receiver, args, heap.as_deref_mut(), budget),
        "remove" => map_remove(receiver, args, heap.as_deref_mut()),
        "keys" => map_keys(receiver, args, heap.as_deref()),
        "values" => map_values(receiver, args, heap.as_deref()),
        "entries" => map_entries(receiver, args, heap.as_deref()),
        _ => Err(VmError::new(VmErrorKind::UnknownMethod {
            method: method.to_owned(),
        })),
    }
}

fn string_predicate(
    receiver: &Value,
    method: &str,
    operation: &'static str,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
    predicate: impl FnOnce(&str, &str) -> bool,
) -> VmResult<bool> {
    expect_arity(method, args, 1)?;
    let receiver = string_value(receiver, heap, operation)?;
    let needle = string_value(&args[0], heap, operation)?;
    Ok(predicate(receiver, needle))
}

fn len(receiver: &Value, heap: Option<&HeapExecution<'_>>) -> VmResult<i64> {
    match receiver {
        Value::String(value) => usize_to_i64(value.chars().count(), "method len"),
        Value::Array(values) => usize_to_i64(values.len(), "method len"),
        Value::Map(values) => usize_to_i64(values.len(), "method len"),
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
                HeapValue::Map(values)
                | HeapValue::Record { fields: values, .. }
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
        Value::Range(range) => Ok(range.is_empty()),
        Value::HeapRef(reference) => {
            let Some(value) = heap.and_then(|heap| heap.heap.get(*reference)) else {
                return type_error("method is_empty");
            };
            match value {
                HeapValue::String(value) => Ok(value.is_empty()),
                HeapValue::Array(values) | HeapValue::Set(values) => Ok(values.is_empty()),
                HeapValue::Map(values)
                | HeapValue::Record { fields: values, .. }
                | HeapValue::Enum { fields: values, .. } => Ok(values.is_empty()),
            }
        }
        Value::Record { fields, .. } | Value::Enum { fields, .. } => Ok(fields.is_empty()),
        _ => type_error("method is_empty"),
    }
}

fn array_push(
    receiver: &mut Value,
    args: &[Value],
    heap: Option<&mut HeapExecution<'_>>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("push", args, 1)?;
    match receiver {
        Value::Array(values) => {
            values.push(args[0].clone());
            Ok(Value::Null)
        }
        Value::HeapRef(reference) => {
            let Some(heap) = heap else {
                return type_error("method push");
            };
            let slot = value_to_heap_slot(&args[0], heap, budget)?;
            let Some(HeapValue::Array(values)) = heap.heap.get_mut(*reference).ok() else {
                return type_error("method push");
            };
            values.push(slot);
            Ok(Value::Null)
        }
        _ => type_error("method push"),
    }
}

fn array_pop(
    receiver: &mut Value,
    args: &[Value],
    heap: Option<&mut HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_no_args("pop", args)?;
    match receiver {
        Value::Array(values) => Ok(values.pop().unwrap_or(Value::Null)),
        Value::HeapRef(reference) => {
            let Some(heap) = heap else {
                return type_error("method pop");
            };
            let Some(HeapValue::Array(values)) = heap.heap.get_mut(*reference).ok() else {
                return type_error("method pop");
            };
            Ok(values
                .pop()
                .map_or(Value::Null, |slot| value_from_heap_slot(&slot)))
        }
        _ => type_error("method pop"),
    }
}

fn map_has(receiver: &Value, args: &[Value], heap: Option<&HeapExecution<'_>>) -> VmResult<bool> {
    expect_arity("has", args, 1)?;
    let key = map_key(&args[0], heap)?;
    match receiver {
        Value::Map(values) => Ok(values.contains_key(&key)),
        Value::HeapRef(reference) => {
            let Some(HeapValue::Map(values)) = heap.and_then(|heap| heap.heap.get(*reference))
            else {
                return type_error("method has");
            };
            Ok(values.contains_key(&key))
        }
        _ => type_error("method has"),
    }
}

fn map_get(receiver: &Value, args: &[Value], heap: Option<&HeapExecution<'_>>) -> VmResult<Value> {
    expect_arity("get", args, 1)?;
    let key = map_key(&args[0], heap)?;
    match receiver {
        Value::Map(values) => Ok(values.get(&key).cloned().unwrap_or(Value::Null)),
        Value::HeapRef(reference) => {
            let Some(HeapValue::Map(values)) = heap.and_then(|heap| heap.heap.get(*reference))
            else {
                return type_error("method get");
            };
            Ok(values.get(&key).map_or(Value::Null, value_from_heap_slot))
        }
        _ => type_error("method get"),
    }
}

fn map_get_or(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("get_or", args, 2)?;
    let key = map_key(&args[0], heap)?;
    match receiver {
        Value::Map(values) => Ok(values.get(&key).cloned().unwrap_or_else(|| args[1].clone())),
        Value::HeapRef(reference) => {
            let Some(HeapValue::Map(values)) = heap.and_then(|heap| heap.heap.get(*reference))
            else {
                return type_error("method get_or");
            };
            Ok(values
                .get(&key)
                .map_or_else(|| args[1].clone(), value_from_heap_slot))
        }
        _ => type_error("method get_or"),
    }
}

fn map_set(
    receiver: &mut Value,
    args: &[Value],
    heap: Option<&mut HeapExecution<'_>>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("set", args, 2)?;
    let key = map_key(&args[0], heap.as_deref())?;
    match receiver {
        Value::Map(values) => {
            values.insert(key, args[1].clone());
            Ok(args[1].clone())
        }
        Value::HeapRef(reference) => {
            let Some(heap) = heap else {
                return type_error("method set");
            };
            let slot = value_to_heap_slot(&args[1], heap, budget)?;
            let Some(HeapValue::Map(values)) = heap.heap.get_mut(*reference).ok() else {
                return type_error("method set");
            };
            values.insert(key, slot);
            Ok(args[1].clone())
        }
        _ => type_error("method set"),
    }
}

fn map_remove(
    receiver: &mut Value,
    args: &[Value],
    heap: Option<&mut HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("remove", args, 1)?;
    let key = map_key(&args[0], heap.as_deref())?;
    match receiver {
        Value::Map(values) => Ok(values.remove(&key).unwrap_or(Value::Null)),
        Value::HeapRef(reference) => {
            let Some(heap) = heap else {
                return type_error("method remove");
            };
            let Some(HeapValue::Map(values)) = heap.heap.get_mut(*reference).ok() else {
                return type_error("method remove");
            };
            Ok(values
                .remove(&key)
                .map_or(Value::Null, |slot| value_from_heap_slot(&slot)))
        }
        _ => type_error("method remove"),
    }
}

fn map_keys(receiver: &Value, args: &[Value], heap: Option<&HeapExecution<'_>>) -> VmResult<Value> {
    expect_no_args("keys", args)?;
    match receiver {
        Value::Map(values) => Ok(Value::Array(
            values
                .keys()
                .map(|key| Value::String(key.clone()))
                .collect(),
        )),
        Value::HeapRef(reference) => {
            let Some(HeapValue::Map(values)) = heap.and_then(|heap| heap.heap.get(*reference))
            else {
                return type_error("method keys");
            };
            Ok(Value::Array(
                values
                    .keys()
                    .map(|key| Value::String(key.clone()))
                    .collect(),
            ))
        }
        _ => type_error("method keys"),
    }
}

fn map_values(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_no_args("values", args)?;
    match receiver {
        Value::Map(values) => Ok(Value::Array(values.values().cloned().collect())),
        Value::HeapRef(reference) => {
            let Some(HeapValue::Map(values)) = heap.and_then(|heap| heap.heap.get(*reference))
            else {
                return type_error("method values");
            };
            Ok(Value::Array(
                values.values().map(value_from_heap_slot).collect(),
            ))
        }
        _ => type_error("method values"),
    }
}

fn map_entries(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_no_args("entries", args)?;
    match receiver {
        Value::Map(values) => Ok(Value::Array(
            values
                .iter()
                .map(|(key, value)| map_entry(key, value.clone()))
                .collect(),
        )),
        Value::HeapRef(reference) => {
            let Some(HeapValue::Map(values)) = heap.and_then(|heap| heap.heap.get(*reference))
            else {
                return type_error("method entries");
            };
            Ok(Value::Array(
                values
                    .iter()
                    .map(|(key, value)| map_entry(key, value_from_heap_slot(value)))
                    .collect(),
            ))
        }
        _ => type_error("method entries"),
    }
}

fn map_entry(key: &str, value: Value) -> Value {
    let mut fields = BTreeMap::new();
    fields.insert("key".to_owned(), Value::String(key.to_owned()));
    fields.insert("value".to_owned(), value);
    Value::Record {
        type_name: "MapEntry".to_owned(),
        fields,
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

fn map_key(value: &Value, heap: Option<&HeapExecution<'_>>) -> VmResult<String> {
    string_value(value, heap, "map key").map(str::to_owned)
}

fn string_value<'a>(
    value: &'a Value,
    heap: Option<&'a HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<&'a str> {
    match value {
        Value::String(value) => Ok(value),
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::String(value)) => Ok(value),
            _ => type_error(operation),
        },
        _ => type_error(operation),
    }
}

fn usize_to_i64(value: usize, operation: &'static str) -> VmResult<i64> {
    i64::try_from(value).map_err(|_| VmError::new(VmErrorKind::TypeMismatch { operation }))
}

fn type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}
