use crate::heap::HeapValue;
use crate::{
    ExecutionBudget, HeapExecution, Value, VmError, VmErrorKind, VmResult, array_methods,
    bytes_methods, map_methods, option_result_methods, set_methods,
};
use vela_def::MethodId;

pub(crate) use crate::standard_method_cache::{
    call_standard_cached, call_standard_readonly_cached, standard_cache_entry,
    standard_cache_entry_matches_method_id,
};

pub(crate) fn call(
    receiver: &mut Value,
    method: &str,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> Option<VmResult<Value>> {
    if bytes_methods::is_bytes(receiver, heap.as_deref())
        && let Some(result) = call_bytes_by_name(receiver, method, args, heap, budget)
    {
        return Some(result);
    }
    let result = match method {
        "len" => expect_no_args(method, args)
            .and_then(|()| len(receiver, heap.as_deref()).map(Value::i64)),
        "is_empty" => expect_no_args(method, args)
            .and_then(|()| is_empty(receiver, heap.as_deref()).map(Value::Bool)),
        "contains" => array_methods::contains(receiver, args, heap.as_deref()).map(Value::Bool),
        "slice" => array_methods::slice(receiver, args, heap, budget),
        "push" => array_methods::push(receiver, args, heap.as_deref_mut(), budget.as_deref_mut()),
        "pop" => array_methods::pop(receiver, args, heap.as_deref_mut(), budget.as_deref_mut()),
        "insert" => {
            array_methods::insert(receiver, args, heap.as_deref_mut(), budget.as_deref_mut())
        }
        "extend" => extend(receiver, args, heap, budget),
        "first" => array_methods::first(receiver, args, heap, budget),
        "last" => array_methods::last(receiver, args, heap, budget),
        "remove_at" => {
            array_methods::remove_at(receiver, args, heap.as_deref_mut(), budget.as_deref_mut())
        }
        "join" => array_methods::join(receiver, args, heap, budget),
        "index_of" => array_methods::index_of(receiver, args, heap, budget),
        "distinct" => array_methods::distinct(receiver, args, heap, budget),
        "reverse" => array_methods::reverse(receiver, args, heap, budget),
        "sort" => array_methods::sort(receiver, args, heap, budget),
        "min" => array_methods::min(receiver, args, heap, budget),
        "max" => array_methods::max(receiver, args, heap, budget),
        "is_some" => option_result_methods::is_some(receiver, args, heap.as_deref()),
        "is_none" => option_result_methods::is_none(receiver, args, heap.as_deref()),
        "is_ok" => option_result_methods::is_ok(receiver, args, heap.as_deref()),
        "is_err" => option_result_methods::is_err(receiver, args, heap.as_deref()),
        "unwrap_or" => option_result_methods::unwrap_or(receiver, args, heap.as_deref()),
        "ok_or" => option_result_methods::ok_or(receiver, args, heap, budget),
        "to_option" => option_result_methods::to_option(receiver, args, heap, budget),
        "to_error_option" => option_result_methods::to_error_option(receiver, args, heap, budget),
        "flatten" => flatten(receiver, args, heap, budget),
        "merge" => map_methods::merge(receiver, args, heap, budget),
        "has" => has(receiver, args, heap.as_deref()).map(Value::Bool),
        "get" => map_methods::get(receiver, args, heap, budget),
        "get_or" => map_methods::get_or(receiver, args, heap.as_deref()),
        "add" => set_methods::add(receiver, args, heap.as_deref_mut(), budget.as_deref_mut()),
        "set" => map_methods::set(receiver, args, heap.as_deref_mut(), budget.as_deref_mut()),
        "remove" => remove(receiver, args, heap, budget),
        "clear" => clear(receiver, args, heap),
        "keys" => map_methods::keys(receiver, args, heap, budget),
        "values" => values(receiver, args, heap, budget),
        "union" => set_methods::union(receiver, args, heap, budget),
        "intersection" => set_methods::intersection(receiver, args, heap, budget),
        "difference" => set_methods::difference(receiver, args, heap, budget),
        "symmetric_difference" => set_methods::symmetric_difference(receiver, args, heap, budget),
        "is_subset" => set_methods::is_subset(receiver, args, heap.as_deref()).map(Value::Bool),
        "is_superset" => set_methods::is_superset(receiver, args, heap.as_deref()).map(Value::Bool),
        "is_disjoint" => set_methods::is_disjoint(receiver, args, heap.as_deref()).map(Value::Bool),
        "entries" => map_methods::entries(receiver, args, heap, budget),
        _ => return None,
    };
    Some(result)
}

pub(crate) fn call_by_id(
    receiver: &mut Value,
    method_id: MethodId,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> Option<VmResult<Value>> {
    let cache = standard_cache_entry(method_id, receiver, heap.as_deref())?;
    call_standard_cached(receiver, cache, args, heap, budget)
}

pub(crate) fn call_readonly(
    receiver: &Value,
    method: &str,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> Option<VmResult<Value>> {
    if bytes_methods::is_bytes(receiver, heap)
        && let Some(result) = call_readonly_bytes_by_name(receiver, method, args, heap)
    {
        return Some(result);
    }
    let result = match method {
        "len" => expect_no_args(method, args).and_then(|()| len(receiver, heap).map(Value::i64)),
        "is_empty" => {
            expect_no_args(method, args).and_then(|()| is_empty(receiver, heap).map(Value::Bool))
        }
        "contains" => array_methods::contains(receiver, args, heap).map(Value::Bool),
        "is_some" => option_result_methods::is_some(receiver, args, heap),
        "is_none" => option_result_methods::is_none(receiver, args, heap),
        "is_ok" => option_result_methods::is_ok(receiver, args, heap),
        "is_err" => option_result_methods::is_err(receiver, args, heap),
        "unwrap_or" => option_result_methods::unwrap_or(receiver, args, heap),
        "has" => has(receiver, args, heap).map(Value::Bool),
        "get_or" => map_methods::get_or(receiver, args, heap),
        "is_subset" => set_methods::is_subset(receiver, args, heap).map(Value::Bool),
        "is_superset" => set_methods::is_superset(receiver, args, heap).map(Value::Bool),
        "is_disjoint" => set_methods::is_disjoint(receiver, args, heap).map(Value::Bool),
        _ => return None,
    };
    Some(result)
}

pub(crate) fn call_readonly_by_id(
    receiver: &Value,
    method_id: MethodId,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> Option<VmResult<Value>> {
    let cache = standard_cache_entry(method_id, receiver, heap)?;
    call_standard_readonly_cached(receiver, cache, args, heap)
}

fn call_bytes_by_name(
    receiver: &Value,
    method: &str,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> Option<VmResult<Value>> {
    match method {
        "len" | "is_empty" | "get" | "read_u32_le" | "read_u32_be" => {
            call_readonly_bytes_by_name(receiver, method, args, heap.as_deref())
        }
        "slice" => Some(bytes_methods::slice(receiver, args, heap, budget)),
        "to_hex" => Some(bytes_methods::to_hex(receiver, args, heap, budget)),
        _ => None,
    }
}

fn call_readonly_bytes_by_name(
    receiver: &Value,
    method: &str,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> Option<VmResult<Value>> {
    match method {
        "len" => Some(bytes_methods::len(receiver, args, heap)),
        "is_empty" => Some(bytes_methods::is_empty(receiver, args, heap)),
        "get" => Some(bytes_methods::get(receiver, args, heap)),
        "read_u32_le" => Some(bytes_methods::read_u32_le(receiver, args, heap)),
        "read_u32_be" => Some(bytes_methods::read_u32_be(receiver, args, heap)),
        _ => None,
    }
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

fn flatten(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    if option_result_methods::is_option_or_result(receiver, heap.as_deref()) {
        option_result_methods::flatten(receiver, args, heap, budget)
    } else {
        Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "method flatten",
        }))
    }
}

pub(crate) fn has(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<bool> {
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
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    if set_methods::is_set(receiver, heap.as_deref()) {
        set_methods::remove(receiver, args, heap.as_deref_mut())
    } else {
        map_methods::remove(receiver, args, heap.as_deref_mut(), budget.as_deref_mut())
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

fn values(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    if set_methods::is_set(receiver, heap.as_deref()) {
        set_methods::values(receiver, args, heap, budget)
    } else {
        map_methods::values(receiver, args, heap, budget)
    }
}

pub(crate) fn len(receiver: &Value, heap: Option<&HeapExecution<'_>>) -> VmResult<i64> {
    match receiver {
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
                HeapValue::String(value) => usize_to_i64(value.len(), "method len"),
                HeapValue::Bytes(value) => usize_to_i64(value.len(), "method len"),
                HeapValue::Array(values) | HeapValue::Set(values) => {
                    usize_to_i64(values.len(), "method len")
                }
                HeapValue::Map(values) => usize_to_i64(values.len(), "method len"),
                HeapValue::Record { fields: values, .. }
                | HeapValue::Enum { fields: values, .. } => {
                    usize_to_i64(values.len(), "method len")
                }
                HeapValue::Closure(_) | HeapValue::Iterator(_) | HeapValue::PathProxy(_) => {
                    type_error("method len")
                }
            }
        }
        _ => type_error("method len"),
    }
}

pub(crate) fn is_empty(receiver: &Value, heap: Option<&HeapExecution<'_>>) -> VmResult<bool> {
    match receiver {
        Value::Range(range) => Ok(range.is_empty()),
        Value::HeapRef(reference) => {
            let Some(value) = heap.and_then(|heap| heap.heap.get(*reference)) else {
                return type_error("method is_empty");
            };
            match value {
                HeapValue::String(value) => Ok(value.is_empty()),
                HeapValue::Bytes(value) => Ok(value.is_empty()),
                HeapValue::Array(values) | HeapValue::Set(values) => Ok(values.is_empty()),
                HeapValue::Map(values) => Ok(values.is_empty()),
                HeapValue::Record { fields: values, .. }
                | HeapValue::Enum { fields: values, .. } => Ok(values.is_empty()),
                HeapValue::Closure(_) | HeapValue::Iterator(_) | HeapValue::PathProxy(_) => {
                    type_error("method is_empty")
                }
            }
        }
        _ => type_error("method is_empty"),
    }
}

pub(crate) fn expect_no_args(method: &str, args: &[Value]) -> VmResult<()> {
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
    use vela_bytecode::compiler::compile_function_source_with_registry;
    use vela_bytecode::{Linker, UnlinkedCodeObject, UnlinkedProgram};
    use vela_common::SourceId;

    use crate::{ExecutionBudget, OwnedValue, Vm, VmResult};

    fn compile_standard_function_source(
        source: SourceId,
        text: &str,
        function_name: &str,
    ) -> vela_bytecode::compiler::error::CompileResult<UnlinkedCodeObject> {
        let registry = vela_stdlib::standard_registry().expect("standard registry should build");
        compile_function_source_with_registry(source, text, function_name, registry.compile_view())
    }

    fn run_linked_builtin_test_code(
        code: UnlinkedCodeObject,
        budget: &mut ExecutionBudget,
    ) -> VmResult<OwnedValue> {
        let entry = code.name.clone();
        let mut program = UnlinkedProgram::new();
        program.insert_function(code);
        let linked = Linker::new()
            .link_program(&program)
            .expect("builtin method test program should link");
        Vm::new().run_linked_program_with_budget(&linked, &entry, &[], budget)
    }

    #[test]
    fn string_len_counts_bytes() {
        let source = r#"
fn main() {
    return "quest".len() * 100 + "é日".len();
}
"#;
        let code = compile_standard_function_source(SourceId::new(1), source, "main")
            .expect("string len source should compile");
        let mut budget = ExecutionBudget::unbounded();

        let result =
            run_linked_builtin_test_code(code, &mut budget).expect("string len should run");
        assert_eq!(
            result,
            OwnedValue::Scalar(vela_common::ScalarValue::I64(505))
        );
    }

    #[test]
    fn managed_heap_string_len_counts_bytes() {
        let source = r#"
fn main() {
    let ascii = "quest";
    let unicode = "é日";
    return ascii.len() * 100 + unicode.len();
}
"#;
        let code = compile_standard_function_source(SourceId::new(1), source, "main")
            .expect("managed heap string len source should compile");
        let mut budget = ExecutionBudget::unbounded();

        let result = run_linked_builtin_test_code(code, &mut budget)
            .expect("managed heap string len should run");
        assert_eq!(
            result,
            OwnedValue::Scalar(vela_common::ScalarValue::I64(505))
        );
    }
}
