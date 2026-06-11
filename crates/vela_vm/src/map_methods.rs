use std::collections::BTreeMap;

use crate::heap::HeapValue;
use crate::script_object::ScriptFields;
use crate::string_methods;
use crate::{
    ExecutionBudget, HeapExecution, Value, VmError, VmErrorKind, VmResult, allocate_heap_value,
    stored_runtime_value,
};

mod higher_order;
mod introspection;
mod lookup;
mod merge;
mod mutation;

pub(crate) use higher_order::{all, any, count, filter, find, map_values};
pub(crate) use introspection::{entries, keys, values};
pub(crate) use lookup::{get, get_or, has};
pub(crate) use merge::merge;
pub(crate) use mutation::{clear, extend, remove, set};

pub(crate) fn is_map(receiver: &Value, heap: Option<&crate::HeapExecution<'_>>) -> bool {
    match receiver {
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
    map_slots(receiver, heap, operation).map(|values| {
        values
            .iter()
            .map(|(key, value)| (key.clone(), stored_runtime_value(value)))
            .collect()
    })
}

pub(super) fn map_slots<'a>(
    receiver: &Value,
    heap: Option<&'a crate::HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<&'a BTreeMap<String, Value>> {
    match receiver {
        Value::HeapRef(reference) => {
            let Some(HeapValue::Map(values)) = heap.and_then(|heap| heap.heap.get(*reference))
            else {
                return type_error(operation);
            };
            Ok(values)
        }
        _ => type_error(operation),
    }
}

pub(super) fn map_entry(
    key: &str,
    value: Value,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    let Some(heap_ref) = heap.as_deref_mut() else {
        return type_error("map entry");
    };
    let key = allocate_heap_value(
        HeapValue::String(key.to_owned()),
        heap_ref,
        budget.as_deref_mut(),
    )?;
    allocate_heap_value(
        HeapValue::Record {
            type_name: "MapEntry".to_owned(),
            identity: None,
            fields: ScriptFields::two("MapEntry", "key", key, "value", value),
        },
        heap_ref,
        budget.as_deref_mut(),
    )
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
    use vela_bytecode::compiler::compile_function_source_with_registry;
    use vela_bytecode::compiler::error::CompileResult;
    use vela_bytecode::{Linker, UnlinkedCodeObject, UnlinkedProgram};
    use vela_common::SourceId;

    use crate::owned_value::OwnedValue;
    use crate::{ExecutionBudget, Vm, VmResult};

    fn compile_function_source(
        source: SourceId,
        text: &str,
        function_name: &str,
    ) -> CompileResult<UnlinkedCodeObject> {
        let registry = vela_stdlib::standard_registry().expect("standard registry should build");
        compile_function_source_with_registry(source, text, function_name, registry.compile_view())
    }

    fn run_linked_map_test_code(vm: &Vm, code: UnlinkedCodeObject) -> VmResult<OwnedValue> {
        let mut budget = ExecutionBudget::unbounded();
        run_linked_map_test_code_with_budget(vm, code, &mut budget)
    }

    fn run_linked_map_test_code_with_budget(
        vm: &Vm,
        code: UnlinkedCodeObject,
        budget: &mut ExecutionBudget,
    ) -> VmResult<OwnedValue> {
        let entry = code.name.clone();
        let mut program = UnlinkedProgram::new();
        program.insert_function(code);

        let mut linker = Linker::new();
        for id in vm
            .native_ids
            .keys()
            .chain(vm.host_native_ids.keys())
            .copied()
        {
            linker.add_native_implementation(id);
        }
        let linked = linker
            .link_program(&program)
            .expect("map method test code should link");

        vm.run_linked_program_with_budget(&linked, &entry, &[], budget)
    }

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

        let result = run_linked_map_test_code(&Vm::new(), code)
            .expect("map higher-order methods should run");
        assert_eq!(result, OwnedValue::Int(2));
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

        let result = run_linked_map_test_code_with_budget(&Vm::new(), code, &mut budget)
            .expect("heap map higher-order methods should run");
        assert_eq!(result, OwnedValue::Bool(true));
    }

    #[test]
    fn runs_compiled_map_find_method() {
        let source = r#"
fn main() {
    let rewards = {"gold": 4, "xp": 6, "quest": 8};
    let found = rewards.find(|key, value| key == "xp" && value == 6);
    let missing = rewards.find(|key, value| key == "missing" && value > 0);
    let entry = option::unwrap_or(found, MapEntry { key: "", value: 0 });
    if entry.key == "xp" && entry.value == 6 && option::is_none(missing) {
        return 1;
    }
    return 0;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("map find source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = run_linked_map_test_code(&vm, code).expect("map find should run");
        assert_eq!(result, OwnedValue::Int(1));
    }

    #[test]
    fn runs_compiled_map_zero_arg_callbacks() {
        let source = r#"
fn main() {
    let rewards = {"gold": 4, "xp": 6};
    let mapped = rewards.map_values(|| 1);
    let filtered = rewards.filter(|| true);
    if mapped["gold"] == 1
        && mapped["xp"] == 1
        && filtered.len() == 2
        && rewards.any(|| true)
        && rewards.all(|| true)
    {
        return rewards.count(|| true);
    }
    return 0;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("map zero-arg callback source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result =
            run_linked_map_test_code(&vm, code).expect("map zero-arg callbacks should run");
        assert_eq!(result, OwnedValue::Int(2));
    }

    #[test]
    fn managed_heap_execution_runs_map_find_method() {
        let source = r#"
fn main() {
    let quests = {"boar": "done", "wolf": "active", "wyrm": "done"};
    let found = quests.find(|key, value| key.starts_with("w") && value == "done");
    let missing = quests.find(|value| value == "blocked");
    let entry = option::unwrap_or(found, MapEntry { key: "", value: "" });
    if entry.key == "wyrm" && entry.value == "done" && option::is_none(missing) {
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

        let result = run_linked_map_test_code_with_budget(&vm, code, &mut budget)
            .expect("heap map find should run");
        assert_eq!(result, OwnedValue::String("wyrm".to_owned()));
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

        let result =
            run_linked_map_test_code(&vm, code).expect("map introspection methods should run");
        assert_eq!(result, OwnedValue::Int(14));
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

        let result = run_linked_map_test_code_with_budget(&vm, code, &mut budget)
            .expect("heap map introspection methods should run");
        assert_eq!(result, OwnedValue::String("done|active|open".to_owned()));
    }

    #[test]
    fn managed_heap_execution_runs_map_lookup_methods() {
        let source = r#"
fn main() {
    let states = {"daily": "done", "raid": "active", "boss": "ready"};
    let scores = {"daily": 3, "raid": 8, "boss": 13};
    if states.has("raid")
        && !states.has("missing")
        && option::unwrap_or(states.get("boss"), "") == "ready"
        && states.get_or("missing", "fallback") == "fallback"
        && scores.get_or("raid", 0) == 8
        && option::unwrap_or(scores.get("missing"), -1) == -1
    {
        return states.len() + scores.get_or("daily", 0);
    }
    return 0;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap map lookup methods should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();
        let mut budget = ExecutionBudget::unbounded();

        let result = run_linked_map_test_code_with_budget(&vm, code, &mut budget)
            .expect("heap map lookup methods should run");
        assert_eq!(result, OwnedValue::Int(6));
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

        let result = run_linked_map_test_code(&vm, code).expect("map merge should run");
        assert_eq!(result, OwnedValue::String("gold,quest,xp".to_owned()));
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

        let result = run_linked_map_test_code_with_budget(&vm, code, &mut budget)
            .expect("heap map merge should run");
        assert_eq!(result, OwnedValue::String("wolf|gold|done".to_owned()));
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

        let error = run_linked_map_test_code(&Vm::new(), code)
            .expect_err("map merge should reject non-map argument");
        assert_eq!(
            error.kind(),
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

        let result =
            run_linked_map_test_code(&Vm::new(), code).expect("map clear method should run");
        assert_eq!(result, OwnedValue::Int(99));
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

        let result = run_linked_map_test_code_with_budget(&Vm::new(), code, &mut budget)
            .expect("heap map clear method should run");
        assert_eq!(result, OwnedValue::String("boss".to_owned()));
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

        let result = run_linked_map_test_code(&vm, code).expect("map extend method should run");
        assert_eq!(result, OwnedValue::String("gold,quest,xp".to_owned()));
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

        let result = run_linked_map_test_code_with_budget(&vm, code, &mut budget)
            .expect("heap map extend method should run");
        assert_eq!(result, OwnedValue::String("claimed|active".to_owned()));
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

        let error = run_linked_map_test_code(&Vm::new(), code)
            .expect_err("map extend should reject non-map args");
        assert_eq!(
            error.kind(),
            crate::VmErrorKind::TypeMismatch {
                operation: "method extend"
            }
        );
    }
}
