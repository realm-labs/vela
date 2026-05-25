use vela_bytecode::Program;
use vela_common::MethodId;
use vela_reflect::TypeRegistry;

use crate::array_methods::{self, MethodRuntime};
use crate::heap::{GcRef, HeapValue};
use crate::map_methods;
use crate::script_object::ScriptFields;
use crate::set_methods;
use crate::string_methods;
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
        "contains" => string_methods::contains(receiver, args, heap.as_deref()).map(Value::Bool),
        "starts_with" => {
            string_methods::starts_with(receiver, args, heap.as_deref()).map(Value::Bool)
        }
        "ends_with" => string_methods::ends_with(receiver, args, heap.as_deref()).map(Value::Bool),
        "to_upper" => string_methods::to_upper(receiver, args, heap.as_deref()),
        "to_lower" => string_methods::to_lower(receiver, args, heap.as_deref()),
        "trim" => string_methods::trim(receiver, args, heap.as_deref()),
        "split" => string_methods::split(receiver, args, heap.as_deref()),
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
        "sort_by" => array_methods::sort_by(
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
        "has" => {
            if set_methods::is_set(receiver, heap.as_deref()) {
                set_methods::has(receiver, args, heap.as_deref())
            } else {
                map_has(receiver, args, heap.as_deref())
            }
        }
        .map(Value::Bool),
        "get" => map_get(receiver, args, heap.as_deref()),
        "get_or" => map_get_or(receiver, args, heap.as_deref()),
        "add" => set_methods::add(receiver, args, heap.as_deref_mut(), budget.as_deref_mut()),
        "set" => map_set(receiver, args, heap.as_deref_mut(), budget),
        "remove" => {
            if set_methods::is_set(receiver, heap.as_deref()) {
                set_methods::remove(receiver, args, heap.as_deref_mut())
            } else {
                map_remove(receiver, args, heap.as_deref_mut())
            }
        }
        "keys" => map_keys(receiver, args, heap.as_deref()),
        "values" => {
            if set_methods::is_set(receiver, heap.as_deref()) {
                set_methods::values(receiver, args, heap.as_deref())
            } else {
                map_values(receiver, args, heap.as_deref())
            }
        }
        "entries" => map_entries(receiver, args, heap.as_deref()),
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
        Value::Array(values) => Ok(option_value(values.pop())),
        Value::HeapRef(reference) => {
            let Some(heap) = heap else {
                return type_error("method pop");
            };
            let Some(HeapValue::Array(values)) = heap.heap.get_mut(*reference).ok() else {
                return type_error("method pop");
            };
            Ok(option_value(
                values.pop().map(|slot| value_from_heap_slot(&slot)),
            ))
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
        Value::Map(values) => Ok(option_value(values.get(&key).cloned())),
        Value::HeapRef(reference) => {
            let Some(HeapValue::Map(values)) = heap.and_then(|heap| heap.heap.get(*reference))
            else {
                return type_error("method get");
            };
            Ok(option_value(values.get(&key).map(value_from_heap_slot)))
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
        Value::Map(values) => Ok(option_value(values.remove(&key))),
        Value::HeapRef(reference) => {
            let Some(heap) = heap else {
                return type_error("method remove");
            };
            let Some(HeapValue::Map(values)) = heap.heap.get_mut(*reference).ok() else {
                return type_error("method remove");
            };
            Ok(option_value(
                values.remove(&key).map(|slot| value_from_heap_slot(&slot)),
            ))
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
    Value::Record {
        type_name: "MapEntry".to_owned(),
        fields: ScriptFields::from_pairs(
            "MapEntry",
            [
                ("key".to_owned(), Value::String(key.to_owned())),
                ("value".to_owned(), value),
            ],
        ),
    }
}

fn option_value(payload: Option<Value>) -> Value {
    let (variant, fields) = match payload {
        Some(value) => ("Some", vec![("0".to_owned(), value)]),
        None => ("None", Vec::new()),
    };
    Value::Enum {
        enum_name: "Option".to_owned(),
        variant: variant.to_owned(),
        fields: ScriptFields::from_pairs(&format!("Option.{variant}"), fields),
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
    string_methods::string_value(value, heap, "map key").map(str::to_owned)
}

fn usize_to_i64(value: usize, operation: &'static str) -> VmResult<i64> {
    i64::try_from(value).map_err(|_| VmError::new(VmErrorKind::TypeMismatch { operation }))
}

fn type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}
