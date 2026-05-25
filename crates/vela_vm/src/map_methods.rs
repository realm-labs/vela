use std::collections::BTreeMap;

use crate::array_methods::MethodRuntime;
use crate::heap::HeapValue;
use crate::{Value, VmError, VmErrorKind, VmResult, value_from_heap_slot};

pub(crate) fn map_values(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("map_values", args, 1)?;
    let entries = map_entries(receiver, runtime.heap.as_deref(), "method map_values")?;
    let mut mapped = BTreeMap::new();
    for (key, value) in entries {
        let protected = mapped.values().cloned().collect::<Vec<_>>();
        let value = call_callback(
            &mut runtime,
            "method map_values",
            &args[0],
            vec![value],
            &protected,
        )?;
        mapped.insert(key, value);
    }
    Ok(Value::Map(mapped))
}

pub(crate) fn filter(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("filter", args, 1)?;
    let entries = map_entries(receiver, runtime.heap.as_deref(), "method filter")?;
    let mut filtered = BTreeMap::new();
    for (key, value) in entries {
        let protected = filtered.values().cloned().collect::<Vec<_>>();
        let predicate = call_callback(
            &mut runtime,
            "method filter",
            &args[0],
            vec![Value::String(key.clone()), value.clone()],
            &protected,
        )?;
        if is_truthy(&predicate) {
            filtered.insert(key, value);
        }
    }
    Ok(Value::Map(filtered))
}

pub(crate) fn any(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<bool> {
    expect_arity("any", args, 1)?;
    let entries = map_entries(receiver, runtime.heap.as_deref(), "method any")?;
    for (key, value) in entries {
        let predicate_args = map_predicate_args(&args[0], key, value, "method any")?;
        let predicate = call_callback(&mut runtime, "method any", &args[0], predicate_args, &[])?;
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
    let entries = map_entries(receiver, runtime.heap.as_deref(), "method all")?;
    for (key, value) in entries {
        let predicate_args = map_predicate_args(&args[0], key, value, "method all")?;
        let predicate = call_callback(&mut runtime, "method all", &args[0], predicate_args, &[])?;
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
    let entries = map_entries(receiver, runtime.heap.as_deref(), "method count")?;
    let mut count = 0_i64;
    for (key, value) in entries {
        let predicate_args = map_predicate_args(&args[0], key, value, "method count")?;
        let predicate = call_callback(&mut runtime, "method count", &args[0], predicate_args, &[])?;
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

pub(crate) fn is_map(receiver: &Value, heap: Option<&crate::HeapExecution<'_>>) -> bool {
    match receiver {
        Value::Map(_) => true,
        Value::HeapRef(reference) => {
            matches!(
                heap.and_then(|heap| heap.heap.get(*reference)),
                Some(HeapValue::Map(_))
            )
        }
        _ => false,
    }
}

fn map_entries(
    receiver: &Value,
    heap: Option<&crate::HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<Vec<(String, Value)>> {
    match receiver {
        Value::Map(values) => Ok(values
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect()),
        Value::HeapRef(reference) => {
            let Some(HeapValue::Map(values)) = heap.and_then(|heap| heap.heap.get(*reference))
            else {
                return type_error(operation);
            };
            Ok(values
                .iter()
                .map(|(key, value)| (key.clone(), value_from_heap_slot(value)))
                .collect())
        }
        _ => type_error(operation),
    }
}

fn call_callback(
    runtime: &mut MethodRuntime<'_, '_, '_>,
    operation: &'static str,
    callback: &Value,
    args: Vec<Value>,
    protected_values: &[Value],
) -> VmResult<Value> {
    let Value::Closure(closure) = callback else {
        return type_error(operation);
    };
    let mut roots = runtime.caller_roots.to_vec();
    args.iter()
        .for_each(|value| value.trace_heap_refs(&mut roots));
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
        &args,
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

fn map_predicate_args(
    callback: &Value,
    key: String,
    value: Value,
    operation: &'static str,
) -> VmResult<Vec<Value>> {
    let Value::Closure(closure) = callback else {
        return type_error(operation);
    };
    match closure.code.params.len() {
        0 => Ok(Vec::new()),
        1 => Ok(vec![value]),
        _ => Ok(vec![Value::String(key), value]),
    }
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
    fn runs_compiled_map_higher_order_methods() {
        let source = r#"
fn main() {
    let rewards = {"gold": 4, "xp": 6, "quest": 8};
    let doubled = rewards.map_values(|value| value * 2);
    let filtered = rewards.filter(|key, value| key.contains("o") && value == 4);
    if doubled["gold"] == 8 && doubled["quest"] == 16
        && filtered.len() == 1 && filtered["gold"] == 4
        && rewards.any(|value| value == 6)
    {
        return rewards.count(|key, value| key.len() >= 2 && value > 4);
    }
    return 0;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("map higher-order methods should compile");

        let result = Vm::new()
            .run(&code)
            .expect("map higher-order methods should run");
        assert_eq!(result, Value::Int(2));
    }

    #[test]
    fn managed_heap_execution_runs_map_higher_order_methods() {
        let source = r#"
fn main() {
    let quests = {"boar": "done", "wolf": "active", "wyrm": "done"};
    let lengths = quests.map_values(|value| value.len());
    let done = quests.filter(|key, value| key.starts_with("w") && value == "done");
    if lengths["wolf"] == 6 && lengths["boar"] == 4
        && done.len() == 1 && done["wyrm"] == "done"
        && quests.count(|key, value| key.starts_with("w") && value.len() >= 4) == 2
    {
        return quests.any(|key, value| key == "wolf" && value == "active")
            && quests.all(|key, value| key.len() >= 4 && value.len() >= 4);
    }
    return false;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap map higher-order methods should compile");
        let mut budget = ExecutionBudget::unbounded();

        let result = Vm::new()
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap map higher-order methods should run");
        assert_eq!(result, Value::Bool(true));
    }
}
