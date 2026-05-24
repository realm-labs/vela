use vela_bytecode::Program;

use crate::heap::{GcRef, HeapValue};
use crate::{
    ExecutionBudget, HeapExecution, HostExecution, Value, Vm, VmError, VmErrorKind, VmResult,
    value_from_heap_slot,
};

pub(crate) struct MethodRuntime<'a, 'host, 'heap> {
    pub(crate) vm: &'a Vm,
    pub(crate) program: Option<&'a Program>,
    pub(crate) host: Option<&'a mut HostExecution<'host>>,
    pub(crate) heap: Option<&'a mut HeapExecution<'heap>>,
    pub(crate) budget: Option<&'a mut ExecutionBudget>,
    pub(crate) caller_roots: &'a [GcRef],
}

pub(crate) fn map(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("map", args, 1)?;
    let values = array_values(receiver, runtime.heap.as_deref(), "method map")?;
    let mut mapped = Vec::with_capacity(values.len());
    for value in values {
        mapped.push(call_unary_callback(
            &mut runtime,
            "method map",
            &args[0],
            value,
            &mapped,
        )?);
    }
    Ok(Value::Array(mapped))
}

pub(crate) fn filter(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("filter", args, 1)?;
    let values = array_values(receiver, runtime.heap.as_deref(), "method filter")?;
    let mut filtered = Vec::new();
    for value in values {
        let predicate = call_unary_callback(
            &mut runtime,
            "method filter",
            &args[0],
            value.clone(),
            &filtered,
        )?;
        if is_truthy(&predicate) {
            filtered.push(value);
        }
    }
    Ok(Value::Array(filtered))
}

pub(crate) fn find(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("find", args, 1)?;
    let values = array_values(receiver, runtime.heap.as_deref(), "method find")?;
    for value in values {
        let predicate =
            call_unary_callback(&mut runtime, "method find", &args[0], value.clone(), &[])?;
        if is_truthy(&predicate) {
            return Ok(value);
        }
    }
    Ok(Value::Null)
}

pub(crate) fn any(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<bool> {
    expect_arity("any", args, 1)?;
    let values = array_values(receiver, runtime.heap.as_deref(), "method any")?;
    for value in values {
        let predicate = call_unary_callback(&mut runtime, "method any", &args[0], value, &[])?;
        if is_truthy(&predicate) {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) fn all(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<bool> {
    expect_arity("all", args, 1)?;
    let values = array_values(receiver, runtime.heap.as_deref(), "method all")?;
    for value in values {
        let predicate = call_unary_callback(&mut runtime, "method all", &args[0], value, &[])?;
        if !is_truthy(&predicate) {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(crate) fn count(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<i64> {
    expect_arity("count", args, 1)?;
    let values = array_values(receiver, runtime.heap.as_deref(), "method count")?;
    let mut count = 0_i64;
    for value in values {
        let predicate = call_unary_callback(&mut runtime, "method count", &args[0], value, &[])?;
        if is_truthy(&predicate) {
            count = count.checked_add(1).ok_or_else(|| {
                VmError::new(VmErrorKind::TypeMismatch {
                    operation: "method count",
                })
            })?;
        }
    }
    Ok(count)
}

fn array_values(
    receiver: &Value,
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<Vec<Value>> {
    match receiver {
        Value::Array(values) => Ok(values.clone()),
        Value::HeapRef(reference) => {
            let Some(HeapValue::Array(values)) = heap.and_then(|heap| heap.heap.get(*reference))
            else {
                return type_error(operation);
            };
            Ok(values.iter().map(value_from_heap_slot).collect())
        }
        _ => type_error(operation),
    }
}

fn call_unary_callback(
    runtime: &mut MethodRuntime<'_, '_, '_>,
    operation: &'static str,
    callback: &Value,
    value: Value,
    protected_values: &[Value],
) -> VmResult<Value> {
    let Value::Closure(closure) = callback else {
        return type_error(operation);
    };
    let mut roots = runtime.caller_roots.to_vec();
    value.trace_heap_refs(&mut roots);
    protected_values
        .iter()
        .for_each(|value| value.trace_heap_refs(&mut roots));
    let protected_root_len = runtime
        .heap
        .as_deref_mut()
        .map(|heap| heap.push_protected_roots(roots));
    let result = runtime.vm.execute_closure_value(
        closure,
        runtime.program,
        &[value],
        runtime.host.as_deref_mut(),
        runtime.heap.as_deref_mut(),
        runtime.budget.as_deref_mut(),
    );
    if let (Some(heap), Some(protected_root_len)) =
        (runtime.heap.as_deref_mut(), protected_root_len)
    {
        heap.truncate_protected_roots(protected_root_len);
    }
    result
}

fn expect_arity(name: &str, args: &[Value], expected: usize) -> VmResult<()> {
    if args.len() == expected {
        return Ok(());
    }
    Err(VmError::new(VmErrorKind::ArityMismatch {
        name: name.to_owned(),
        expected,
        actual: args.len(),
    }))
}

fn is_truthy(value: &Value) -> bool {
    !matches!(value, Value::Missing | Value::Null | Value::Bool(false))
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
    fn runs_compiled_array_higher_order_methods() {
        let source = r#"
fn main() {
    let values = [1, 2, 3, 4];
    let doubled = values.map(|value| value * 2);
    let evens = values.filter(|value| value % 2 == 0);
    let first_large = values.find(|value| value > 2);
    let missing = values.find(|value| value > 10);
    let count = values.count(|value| value > 1);
    if doubled[2] == 6 && evens[0] == 2 && evens[1] == 4
        && first_large == 3 && missing == null
        && values.any(|value| value == 4)
        && values.all(|value| value > 0)
    {
        return count;
    }
    return 0;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("array higher-order methods should compile");

        let result = Vm::new()
            .run(&code)
            .expect("array higher-order methods should run");
        assert_eq!(result, Value::Int(3));
    }

    #[test]
    fn managed_heap_execution_runs_array_higher_order_methods() {
        let source = r#"
fn main() {
    let names = ["boar", "wolf", "wyrm"];
    let lengths = names.map(|name| name.len());
    let matches = names.filter(|name| name.starts_with("w"));
    let found = names.find(|name| name.contains("yr"));
    if lengths[0] == 4 && lengths[2] == 4
        && matches.len() == 2 && matches[1] == "wyrm"
        && found == "wyrm"
        && names.any(|name| name.ends_with("f"))
        && names.all(|name| name.len() == 4)
    {
        return names.count(|name| name.contains("o"));
    }
    return 0;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap array higher-order methods should compile");
        let mut budget = ExecutionBudget::unbounded();

        let result = Vm::new()
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap array higher-order methods should run");
        assert_eq!(result, Value::Int(2));
    }
}
