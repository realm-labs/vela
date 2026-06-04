use crate::heap::HeapValue;
use crate::{
    ExecutionBudget, HeapExecution, Value, VmError, VmErrorKind, VmResult, array_methods,
    map_methods, option_result_methods, set_methods,
};

pub(crate) fn call(
    receiver: &mut Value,
    method: &str,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> Option<VmResult<Value>> {
    let result = match method {
        "len" => expect_no_args(method, args)
            .and_then(|()| len(receiver, heap.as_deref()).map(Value::Int)),
        "is_empty" => expect_no_args(method, args)
            .and_then(|()| is_empty(receiver, heap.as_deref()).map(Value::Bool)),
        "contains" => array_methods::contains(receiver, args, heap.as_deref()).map(Value::Bool),
        "slice" => array_methods::slice(receiver, args, heap.as_deref()),
        "push" => array_methods::push(receiver, args, heap.as_deref_mut(), budget.as_deref_mut()),
        "pop" => array_methods::pop(receiver, args, heap.as_deref_mut()),
        "insert" => {
            array_methods::insert(receiver, args, heap.as_deref_mut(), budget.as_deref_mut())
        }
        "extend" => extend(receiver, args, heap, budget),
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
        "flatten" => flatten(receiver, args, heap.as_deref()),
        "merge" => map_methods::merge(receiver, args, heap.as_deref()),
        "has" => has(receiver, args, heap.as_deref()).map(Value::Bool),
        "get" => map_methods::get(receiver, args, heap.as_deref()),
        "get_or" => map_methods::get_or(receiver, args, heap.as_deref()),
        "add" => set_methods::add(receiver, args, heap.as_deref_mut(), budget.as_deref_mut()),
        "set" => map_methods::set(receiver, args, heap.as_deref_mut(), budget.as_deref_mut()),
        "remove" => remove(receiver, args, heap),
        "clear" => clear(receiver, args, heap),
        "keys" => map_methods::keys(receiver, args, heap.as_deref()),
        "values" => values(receiver, args, heap.as_deref()),
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
        _ => return None,
    };
    Some(result)
}

pub(crate) fn call_readonly(
    receiver: &Value,
    method: &str,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> Option<VmResult<Value>> {
    let result = match method {
        "len" => expect_no_args(method, args).and_then(|()| len(receiver, heap).map(Value::Int)),
        "is_empty" => {
            expect_no_args(method, args).and_then(|()| is_empty(receiver, heap).map(Value::Bool))
        }
        "contains" => array_methods::contains(receiver, args, heap).map(Value::Bool),
        "slice" => array_methods::slice(receiver, args, heap),
        "first" => array_methods::first(receiver, args, heap),
        "last" => array_methods::last(receiver, args, heap),
        "join" => array_methods::join(receiver, args, heap),
        "index_of" => array_methods::index_of(receiver, args, heap),
        "distinct" => array_methods::distinct(receiver, args, heap),
        "reverse" => array_methods::reverse(receiver, args, heap),
        "sort" => array_methods::sort(receiver, args, heap),
        "min" => array_methods::min(receiver, args, heap),
        "max" => array_methods::max(receiver, args, heap),
        "is_some" => option_result_methods::is_some(receiver, args, heap),
        "is_none" => option_result_methods::is_none(receiver, args, heap),
        "is_ok" => option_result_methods::is_ok(receiver, args, heap),
        "is_err" => option_result_methods::is_err(receiver, args, heap),
        "unwrap_or" => option_result_methods::unwrap_or(receiver, args, heap),
        "ok_or" => option_result_methods::ok_or(receiver, args, heap),
        "to_option" => option_result_methods::to_option(receiver, args, heap),
        "to_error_option" => option_result_methods::to_error_option(receiver, args, heap),
        "flatten" => flatten(receiver, args, heap),
        "merge" => map_methods::merge(receiver, args, heap),
        "has" => has(receiver, args, heap).map(Value::Bool),
        "get" => map_methods::get(receiver, args, heap),
        "get_or" => map_methods::get_or(receiver, args, heap),
        "keys" => map_methods::keys(receiver, args, heap),
        "values" => values(receiver, args, heap),
        "union" => set_methods::union(receiver, args, heap),
        "intersection" => set_methods::intersection(receiver, args, heap),
        "difference" => set_methods::difference(receiver, args, heap),
        "symmetric_difference" => set_methods::symmetric_difference(receiver, args, heap),
        "is_subset" => set_methods::is_subset(receiver, args, heap).map(Value::Bool),
        "is_superset" => set_methods::is_superset(receiver, args, heap).map(Value::Bool),
        "is_disjoint" => set_methods::is_disjoint(receiver, args, heap).map(Value::Bool),
        "entries" => map_methods::entries(receiver, args, heap),
        _ => return None,
    };
    Some(result)
}

fn extend(
    receiver: &mut Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    if set_methods::is_set(receiver, heap.as_deref()) {
        set_methods::extend(receiver, args, heap.as_deref_mut(), budget.as_deref_mut())
    } else if map_methods::is_map(receiver, heap.as_deref()) {
        map_methods::extend(receiver, args, heap.as_deref_mut(), budget.as_deref_mut())
    } else {
        array_methods::extend(receiver, args, heap.as_deref_mut(), budget.as_deref_mut())
    }
}

fn flatten(receiver: &Value, args: &[Value], heap: Option<&HeapExecution<'_>>) -> VmResult<Value> {
    if option_result_methods::is_option_or_result(receiver, heap) {
        option_result_methods::flatten(receiver, args, heap)
    } else {
        Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "method flatten",
        }))
    }
}

fn has(receiver: &Value, args: &[Value], heap: Option<&HeapExecution<'_>>) -> VmResult<bool> {
    if set_methods::is_set(receiver, heap) {
        set_methods::has(receiver, args, heap)
    } else {
        map_methods::has(receiver, args, heap)
    }
}

fn remove(
    receiver: &mut Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
) -> VmResult<Value> {
    if set_methods::is_set(receiver, heap.as_deref()) {
        set_methods::remove(receiver, args, heap.as_deref_mut())
    } else {
        map_methods::remove(receiver, args, heap.as_deref_mut())
    }
}

fn clear(
    receiver: &mut Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
) -> VmResult<Value> {
    if set_methods::is_set(receiver, heap.as_deref()) {
        set_methods::clear(receiver, args, heap.as_deref_mut())
    } else if map_methods::is_map(receiver, heap.as_deref()) {
        map_methods::clear(receiver, args, heap.as_deref_mut())
    } else {
        array_methods::clear(receiver, args, heap.as_deref_mut())
    }
}

fn values(receiver: &Value, args: &[Value], heap: Option<&HeapExecution<'_>>) -> VmResult<Value> {
    if set_methods::is_set(receiver, heap) {
        set_methods::values(receiver, args, heap)
    } else {
        map_methods::values(receiver, args, heap)
    }
}

fn len(receiver: &Value, heap: Option<&HeapExecution<'_>>) -> VmResult<i64> {
    match receiver {
        Value::String(value) => usize_to_i64(string_char_len(value), "method len"),
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
                HeapValue::String(value) => usize_to_i64(string_char_len(value), "method len"),
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

fn string_char_len(value: &str) -> usize {
    if value.is_ascii() {
        value.len()
    } else {
        value.chars().count()
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

#[cfg(test)]
mod tests {
    use vela_bytecode::compiler::compile_function_source;
    use vela_common::SourceId;

    use crate::{ExecutionBudget, Value, Vm};

    #[test]
    fn string_len_counts_unicode_characters() {
        let source = r#"
fn main() {
    return "quest".len() * 100 + "é日".len();
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("string len source should compile");

        let result = Vm::new().run(&code).expect("string len should run");
        assert_eq!(result, Value::Int(502));
    }

    #[test]
    fn managed_heap_string_len_counts_unicode_characters() {
        let source = r#"
fn main() {
    let ascii = "quest";
    let unicode = "é日";
    return ascii.len() * 100 + unicode.len();
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("managed heap string len source should compile");
        let mut budget = ExecutionBudget::unbounded();

        let result = Vm::new()
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("managed heap string len should run");
        assert_eq!(result, Value::Int(502));
    }
}
