use std::collections::BTreeMap;

use crate::heap::HeapValue;
use crate::method_runtime::{MethodRuntime, call_callback};
use crate::option_result::option_value;
use crate::script_object::ScriptFields;
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
            &[value],
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
            &[Value::String(key.clone()), value.clone()],
            &protected,
        )?;
        if is_truthy(&predicate) {
            filtered.insert(key, value);
        }
    }
    Ok(Value::Map(filtered))
}

pub(crate) fn find(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("find", args, 1)?;
    let entries = map_entries(receiver, runtime.heap.as_deref(), "method find")?;
    for (key, value) in entries {
        let predicate_args =
            map_predicate_args(&args[0], key.clone(), value.clone(), "method find")?;
        let predicate = call_callback(&mut runtime, "method find", &args[0], &predicate_args, &[])?;
        if is_truthy(&predicate) {
            return Ok(option_value(Some(map_entry(&key, value))));
        }
    }
    Ok(option_value(None))
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
        let predicate = call_callback(&mut runtime, "method any", &args[0], &predicate_args, &[])?;
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
        let predicate = call_callback(&mut runtime, "method all", &args[0], &predicate_args, &[])?;
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
        let predicate =
            call_callback(&mut runtime, "method count", &args[0], &predicate_args, &[])?;
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

pub(crate) fn merge(
    receiver: &Value,
    args: &[Value],
    heap: Option<&crate::HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("merge", args, 1)?;
    let mut merged = BTreeMap::new();
    for (key, value) in map_entries(receiver, heap, "method merge")? {
        merged.insert(key, value);
    }
    for (key, value) in map_entries(&args[0], heap, "method merge")? {
        merged.insert(key, value);
    }
    Ok(Value::Map(merged))
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

    #[test]
    fn runs_compiled_map_find_method() {
        let source = r#"
fn main() {
    let rewards = {"gold": 4, "xp": 6, "quest": 8};
    let found = rewards.find(|key, value| key == "xp" && value == 6);
    let missing = rewards.find(|key, value| key == "missing" && value > 0);
    let entry = option.unwrap_or(found, MapEntry { key: "", value: 0 });
    if entry.key == "xp" && entry.value == 6 && option.is_none(missing) {
        return 1;
    }
    return 0;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("map find source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm.run(&code).expect("map find should run");
        assert_eq!(result, Value::Int(1));
    }

    #[test]
    fn managed_heap_execution_runs_map_find_method() {
        let source = r#"
fn main() {
    let quests = {"boar": "done", "wolf": "active", "wyrm": "done"};
    let found = quests.find(|key, value| key.starts_with("w") && value == "done");
    let missing = quests.find(|value| value == "blocked");
    let entry = option.unwrap_or(found, MapEntry { key: "", value: "" });
    if entry.key == "wyrm" && entry.value == "done" && option.is_none(missing) {
        return entry.key;
    }
    return "";
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap map find source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();
        let mut budget = ExecutionBudget::unbounded();

        let result = vm
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap map find should run");
        assert_eq!(result, Value::String("wyrm".to_owned()));
    }

    #[test]
    fn runs_compiled_map_merge_method() {
        let source = r#"
fn main() {
    let base = {"gold": 4, "xp": 6};
    let bonus = {"quest": 8, "xp": 10};
    let merged = base.merge(bonus);
    if base["xp"] == 6
        && merged.len() == 3
        && merged["gold"] == 4
        && merged["quest"] == 8
        && merged["xp"] == 10
    {
        return merged.keys().join(",");
    }
    return "";
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main").expect("merge source");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm.run(&code).expect("map merge should run");
        assert_eq!(result, Value::String("gold,quest,xp".to_owned()));
    }

    #[test]
    fn managed_heap_execution_runs_map_merge_method() {
        let source = r#"
fn main() {
    let base = {"state": "active", "owner": "wolf"};
    let patch = {"state": "done", "reward": "gold"};
    let merged = base.merge(patch);
    if base["state"] == "active"
        && merged["state"] == "done"
        && merged["owner"] == "wolf"
        && merged["reward"] == "gold"
    {
        return merged.values().join("|");
    }
    return "";
}
"#;
        let code =
            compile_function_source(SourceId::new(1), source, "main").expect("heap merge source");
        let mut vm = Vm::new();
        vm.register_standard_natives();
        let mut budget = ExecutionBudget::unbounded();

        let result = vm
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap map merge should run");
        assert_eq!(result, Value::String("wolf|gold|done".to_owned()));
    }

    #[test]
    fn map_merge_rejects_non_map_arguments() {
        let source = r#"
fn main() {
    return {"gold": 4}.merge(["xp"]);
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("merge type error source");

        let error = Vm::new()
            .run(&code)
            .expect_err("map merge should reject non-map argument");
        assert_eq!(
            error.kind,
            crate::VmErrorKind::TypeMismatch {
                operation: "method merge"
            }
        );
    }
}
