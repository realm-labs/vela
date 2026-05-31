use std::collections::BTreeMap;

use crate::heap::HeapValue;
use crate::script_object::ScriptFields;
use crate::string_methods;
use crate::{HeapExecution, Value, VmError, VmErrorKind, VmResult, value_from_heap_slot};

mod higher_order;
mod introspection;
mod lookup;
mod mutation;

pub(crate) use higher_order::{all, any, count, filter, find, map_values};
pub(crate) use introspection::{entries, keys, values};
pub(crate) use lookup::{get, get_or, has};
pub(crate) use mutation::{clear, extend, remove, set};

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

pub(super) fn map_entries(
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

pub(super) fn map_entry(key: &str, value: Value) -> Value {
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

pub(super) fn expect_arity(name: &str, args: &[Value], expected: usize) -> VmResult<()> {
    if args.len() == expected {
        return Ok(());
    }
    Err(VmError::new(VmErrorKind::ArityMismatch {
        name: name.to_owned(),
        expected,
        actual: args.len(),
    }))
}

pub(super) fn expect_no_args(method: &str, args: &[Value]) -> VmResult<()> {
    expect_arity(method, args, 0)
}

pub(super) fn map_key(value: &Value, heap: Option<&HeapExecution<'_>>) -> VmResult<String> {
    string_methods::string_value(value, heap, "map key").map(str::to_owned)
}

pub(super) fn type_error<T>(operation: &'static str) -> VmResult<T> {
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
    let keyed = rewards.map_values(|key, value| key.len() + value);
    let filtered = rewards.filter(|key, value| key.contains("o") && value == 4);
    let valuable = rewards.filter(|value| value >= 6);
    if doubled["gold"] == 8 && doubled["quest"] == 16
        && keyed["gold"] == 8 && keyed["xp"] == 8
        && filtered.len() == 1 && filtered["gold"] == 4
        && valuable.len() == 2 && valuable["xp"] == 6 && valuable["quest"] == 8
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
    let scores = quests.map_values(|key, value| key.len() + value.len());
    let done = quests.filter(|key, value| key.starts_with("w") && value == "done");
    let active = quests.filter(|value| value == "active");
    if lengths["wolf"] == 6 && lengths["boar"] == 4
        && scores["boar"] == 8 && scores["wolf"] == 10
        && done.len() == 1 && done["wyrm"] == "done"
        && active.len() == 1 && active["wolf"] == "active"
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
    fn runs_compiled_map_introspection_methods() {
        let source = r#"
fn main() {
    let rewards = {"xp": 6, "gold": 4, "quest": 8};
    let keys = rewards.keys();
    let values = rewards.values();
    let entries = rewards.entries();
    if keys.join(",") == "gold,quest,xp"
        && values[0] == 4
        && values[1] == 8
        && values[2] == 6
        && entries[0].key == "gold"
        && entries[0].value == 4
        && entries[2].key == "xp"
        && entries[2].value == 6
    {
        return values[1] + entries[0].value + keys[2].len();
    }
    return 0;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("map introspection methods should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm.run(&code).expect("map introspection methods should run");
        assert_eq!(result, Value::Int(14));
    }

    #[test]
    fn managed_heap_execution_runs_map_introspection_methods() {
        let source = r#"
fn main() {
    let quests = {"raid": "active", "daily": "done", "world": "open"};
    let keys = quests.keys();
    let values = quests.values();
    let entries = quests.entries();
    if keys.join(",") == "daily,raid,world"
        && entries[0].key == "daily"
        && entries[0].value == "done"
        && entries[1].key == "raid"
        && entries[1].value == "active"
        && entries[2].key == "world"
        && entries[2].value == "open"
    {
        return values.join("|");
    }
    return "";
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap map introspection methods should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();
        let mut budget = ExecutionBudget::unbounded();

        let result = vm
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap map introspection methods should run");
        assert_eq!(result, Value::String("done|active|open".to_owned()));
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

    #[test]
    fn runs_compiled_map_clear_method() {
        let source = r#"
fn main() {
    let rewards = {"gold": 4, "xp": 6};
    rewards.clear();
    rewards.set("quest", 8);
    if rewards.len() == 1 && rewards["quest"] == 8 {
        return rewards.get_or("gold", 99);
    }
    return 0;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("map clear method should compile");

        let result = Vm::new().run(&code).expect("map clear method should run");
        assert_eq!(result, Value::Int(99));
    }

    #[test]
    fn managed_heap_execution_runs_map_clear_method() {
        let source = r#"
fn main() {
    let quests = {"daily": "done", "raid": "active"};
    quests.clear();
    quests.set("boss", "ready");
    if quests.len() == 1 && quests["boss"] == "ready" {
        return quests.keys().join("|");
    }
    return "";
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap map clear method should compile");
        let mut budget = ExecutionBudget::unbounded();

        let result = Vm::new()
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap map clear method should run");
        assert_eq!(result, Value::String("boss".to_owned()));
    }

    #[test]
    fn runs_compiled_map_extend_method() {
        let source = r#"
fn main() {
    let rewards = {"gold": 4, "xp": 6};
    rewards.extend({"xp": 10, "quest": 8});
    if rewards.len() == 3
        && rewards["gold"] == 4
        && rewards["xp"] == 10
        && rewards["quest"] == 8
    {
        return rewards.keys().join(",");
    }
    return "";
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("map extend method should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm.run(&code).expect("map extend method should run");
        assert_eq!(result, Value::String("gold,quest,xp".to_owned()));
    }

    #[test]
    fn managed_heap_execution_runs_map_extend_method() {
        let source = r#"
fn main() {
    let quests = {"daily": "done"};
    let patch = {"raid": "active", "daily": "claimed"};
    quests.extend(patch);
    if quests.len() == 2 && quests["daily"] == "claimed" {
        return quests.values().join("|");
    }
    return "";
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap map extend method should compile");
        let mut budget = ExecutionBudget::unbounded();
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap map extend method should run");
        assert_eq!(result, Value::String("claimed|active".to_owned()));
    }

    #[test]
    fn map_extend_rejects_non_map_arguments() {
        let source = r#"
fn main() {
    let rewards = {"gold": 4};
    rewards.extend(["xp"]);
    return rewards.len();
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("map extend error source should compile");

        let error = Vm::new()
            .run(&code)
            .expect_err("map extend should reject non-map args");
        assert_eq!(
            error.kind,
            crate::VmErrorKind::TypeMismatch {
                operation: "method extend"
            }
        );
    }
}
