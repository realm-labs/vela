use crate::heap::HeapValue;
use crate::{HeapExecution, Value, VmError, VmErrorKind, VmResult};

mod basic;
mod combination;
mod higher_order;
mod key;
mod mutation;

pub(crate) use basic::{from_array, has, values};
pub(crate) use combination::{
    difference, intersection, is_disjoint, is_subset, is_superset, symmetric_difference, union,
};
pub(crate) use higher_order::{all, any, count, filter, find, map};
use key::{SetKey, set_keys, slot_key};
pub(crate) use mutation::{add, clear, extend, remove};

pub(crate) fn is_set(receiver: &Value, heap: Option<&HeapExecution<'_>>) -> bool {
    match receiver {
        Value::HeapRef(reference) => {
            matches!(
                heap.and_then(|heap| heap.heap.get(*reference)),
                Some(HeapValue::Set(_))
            )
        }
        _ => false,
    }
}

pub(super) fn push_unique(
    values: &mut Vec<Value>,
    value: Value,
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<bool> {
    let key = SetKey::from_value(&value, heap, operation)?;
    if values
        .iter()
        .any(|value| SetKey::from_value(value, heap, operation).as_ref() == Ok(&key))
    {
        return Ok(false);
    }
    values.push(value);
    Ok(true)
}

pub(super) fn set_values(
    receiver: &Value,
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<Vec<Value>> {
    set_slots(receiver, heap, operation).map(<[Value]>::to_vec)
}

pub(super) fn set_slots<'a>(
    receiver: &Value,
    heap: Option<&'a HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<&'a [Value]> {
    match receiver {
        Value::HeapRef(reference) => {
            let Some(HeapValue::Set(values)) = heap.and_then(|heap| heap.heap.get(*reference))
            else {
                return type_error(operation);
            };
            Ok(values)
        }
        _ => type_error(operation),
    }
}

pub(super) fn expect_arity<T>(name: &str, args: &[T], expected: usize) -> VmResult<()> {
    if args.len() == expected {
        return Ok(());
    }
    Err(VmError::new(VmErrorKind::ArityMismatch {
        name: name.to_owned(),
        expected,
        actual: args.len(),
    }))
}

pub(super) fn type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}

#[cfg(test)]
mod tests {
    use vela_bytecode::compiler::compile_function_source;
    use vela_common::SourceId;

    use crate::owned_value::OwnedValue;
    use crate::{ExecutionBudget, Vm};

    #[test]
    fn runs_compiled_set_combination_methods() {
        let source = r#"
fn main() {
    let player = set::from_array(["daily", "quest", "raid"]);
    let event = set::from_array(["quest", "bonus", "daily"]);
    let unioned = player.union(event).values().sort_by(|tag| tag).join(",");
    let shared = player.intersection(event).values().sort_by(|tag| tag).join(",");
    let missing = player.difference(event).values().join(",");
    let changed = player.symmetric_difference(event).values().sort_by(|tag| tag).join(",");
    let required = set::from_array(["daily", "quest"]);
    if unioned == "bonus,daily,quest,raid"
        && shared == "daily,quest"
        && missing == "raid"
        && changed == "bonus,raid"
        && required.is_subset(player)
        && player.is_superset(required)
        && player.is_disjoint(set::from_array(["bonus"]))
        && player.len() == 3
    {
        return shared;
    }
    return "";
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("set combination source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm.run(&code).expect("set combination methods should run");
        assert_eq!(result, OwnedValue::String("daily,quest".to_owned()));
    }

    #[test]
    fn managed_heap_execution_runs_set_combination_methods() {
        let source = r#"
fn main() {
    let base = set::from_array([1, 2, 3, 5]);
    let bonus = set::from_array([2, 4, 5]);
    let unioned = base.union(bonus);
    let shared = base.intersection(bonus);
    let only_base = base.difference(bonus);
    let changed = base.symmetric_difference(bonus);
    let required = set::from_array([1, 3]);
    let excluded = set::from_array([9]);
    if !required.is_subset(base) || !base.is_superset(required) || !base.is_disjoint(excluded) {
        return -1;
    }
    return unioned.values().sum()
        + shared.values().sum()
        + only_base.values().sum()
        + changed.values().sum();
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap set combination source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();
        let mut budget = ExecutionBudget::unbounded();

        let result = vm
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap set combination methods should run");
        assert_eq!(result, OwnedValue::Int(34));
    }

    #[test]
    fn managed_heap_execution_runs_string_set_predicates() {
        let source = r#"
fn main() {
    let player = set::from_array(["daily", "quest", "raid"]);
    let required = set::from_array(["daily", "quest"]);
    let event = set::from_array(["quest", "bonus"]);
    let unioned = player.union(event).values().sort_by(|tag| tag).join(",");
    let shared = player.intersection(event).values().sort_by(|tag| tag).join(",");
    let missing = player.difference(required).values().sort_by(|tag| tag).join(",");
    let changed = player.symmetric_difference(event).values().sort_by(|tag| tag).join(",");
    if required.is_subset(player)
        && player.is_superset(required)
        && player.is_disjoint(set::from_array(["bonus"]))
        && unioned == "bonus,daily,quest,raid"
        && shared == "quest"
        && missing == "raid"
        && changed == "bonus,daily,raid"
    {
        return unioned;
    }
    return "";
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap string set predicate source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();
        let mut budget = ExecutionBudget::unbounded();

        let result = vm
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap string set predicates should run");
        assert_eq!(
            result,
            OwnedValue::String("bonus,daily,quest,raid".to_owned())
        );
    }

    #[test]
    fn managed_heap_execution_runs_set_has_method() {
        let source = r#"
fn main() {
    let tags = set::from_array(["daily", "quest", "raid"]);
    let ids = set::from_array([2, 4, 8]);
    if tags.has("quest") && !tags.has("missing") && ids.has(8) && !ids.has(16) {
        return tags.len() + ids.len();
    }
    return 0;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap set has source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();
        let mut budget = ExecutionBudget::unbounded();

        let result = vm
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap set has method should run");
        assert_eq!(result, OwnedValue::Int(6));
    }

    #[test]
    fn runs_compiled_set_filter_method() {
        let source = r#"
fn main() {
    let tags = set::from_array(["daily", "quest", "raid", "daily"]);
    let filtered = tags.filter(|tag| tag.starts_with("q") || tag == "raid");
    let unchanged = tags.values().sort_by(|tag| tag).join(",");
    if unchanged == "daily,quest,raid" && filtered.len() == 2 {
        return filtered.values().sort_by(|tag| tag).join(",");
    }
    return "";
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("set filter source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm.run(&code).expect("set filter should run");
        assert_eq!(result, OwnedValue::String("quest,raid".to_owned()));
    }

    #[test]
    fn managed_heap_execution_runs_set_filter_method() {
        let source = r#"
fn main() {
    let ids = set::from_array([1, 2, 3, 4, 5]);
    let filtered = ids.filter(|id| id > 2 && id != 4);
    return filtered.values().sum() + ids.values().sum();
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap set filter source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();
        let mut budget = ExecutionBudget::unbounded();

        let result = vm
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap set filter should run");
        assert_eq!(result, OwnedValue::Int(23));
    }

    #[test]
    fn runs_compiled_set_map_method() {
        let source = r#"
fn main() {
    let tags = set::from_array(["daily", "quest", "raid"]);
    let mapped = tags.map(|tag| tag.to_upper()).values().sort_by(|tag| tag).join(",");
    let lengths = tags.map(|tag| tag.len());
    if tags.len() == 3 && lengths.len() == 2 {
        return mapped;
    }
    return "";
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("set map source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm.run(&code).expect("set map should run");
        assert_eq!(result, OwnedValue::String("DAILY,QUEST,RAID".to_owned()));
    }

    #[test]
    fn managed_heap_execution_runs_set_map_method() {
        let source = r#"
fn main() {
    let ids = set::from_array([1, 2, 3, 4]);
    let doubled = ids.map(|id| id * 2);
    let parity = ids.map(|id| id % 2);
    return doubled.values().sum() + parity.values().sum();
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap set map source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();
        let mut budget = ExecutionBudget::unbounded();

        let result = vm
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap set map should run");
        assert_eq!(result, OwnedValue::Int(21));
    }

    #[test]
    fn runs_compiled_set_higher_order_predicates() {
        let source = r#"
fn main() {
    let tags = set::from_array(["daily", "quest", "raid"]);
    let found = tags.find(|tag| tag.starts_with("q"));
    if tags.any(|tag| tag == "raid")
        && tags.all(|tag| tag.len() >= 4)
        && tags.count(|tag| tag.contains("a")) == 2
    {
        return found.unwrap_or("");
    }
    return "";
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("set higher-order source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm.run(&code).expect("set higher-order methods should run");
        assert_eq!(result, OwnedValue::String("quest".to_owned()));
    }

    #[test]
    fn managed_heap_execution_runs_set_higher_order_predicates() {
        let source = r#"
fn main() {
    let ids = set::from_array([2, 4, 6, 9]);
    let first_large = ids.find(|id| id > 5).unwrap_or(0);
    if ids.any(|id| id == 9)
        && !ids.all(|id| id % 2 == 0)
        && ids.count(|id| id % 2 == 0) == 3
    {
        return first_large + ids.values().sum();
    }
    return 0;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap set higher-order source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();
        let mut budget = ExecutionBudget::unbounded();

        let result = vm
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap set higher-order methods should run");
        assert_eq!(result, OwnedValue::Int(27));
    }

    #[test]
    fn set_filter_rejects_non_callback_args() {
        let source = r#"
fn main() {
    let tags = set::from_array(["quest"]);
    return tags.filter("quest");
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("set filter type error source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let error = vm
            .run(&code)
            .expect_err("set filter should reject non-callback args");
        assert_eq!(
            error.kind,
            crate::VmErrorKind::TypeMismatch {
                operation: "method filter"
            }
        );
    }

    #[test]
    fn set_combination_methods_reject_non_set_args() {
        let source = r#"
fn main() {
    let tags = set::from_array(["quest"]);
    return tags.union(["raid"]);
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("set combination type error source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let error = vm
            .run(&code)
            .expect_err("set union should reject non-set args");
        assert_eq!(
            error.kind,
            crate::VmErrorKind::TypeMismatch {
                operation: "method union"
            }
        );

        let source = r#"
fn main() {
    let tags = set::from_array(["quest"]);
    return tags.symmetric_difference(["quest"]);
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("set symmetric_difference type error source should compile");

        let error = vm
            .run(&code)
            .expect_err("set symmetric_difference should reject non-set args");
        assert_eq!(
            error.kind,
            crate::VmErrorKind::TypeMismatch {
                operation: "method symmetric_difference"
            }
        );

        let source = r#"
fn main() {
    let tags = set::from_array(["quest"]);
    return tags.is_subset(["quest"]);
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("set predicate type error source should compile");

        let error = vm
            .run(&code)
            .expect_err("set predicate should reject non-set args");
        assert_eq!(
            error.kind,
            crate::VmErrorKind::TypeMismatch {
                operation: "method is_subset"
            }
        );
    }

    #[test]
    fn runs_compiled_set_clear_method() {
        let source = r#"
fn main() {
    let tags = set::from_array(["daily", "quest"]);
    tags.clear();
    tags.add("raid");
    if tags.len() == 1 && tags.has("raid") {
        let values = tags.values();
        return values[0];
    }
    return "";
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("set clear method should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm.run(&code).expect("set clear method should run");
        assert_eq!(result, OwnedValue::String("raid".to_owned()));
    }

    #[test]
    fn managed_heap_execution_runs_set_clear_method() {
        let source = r#"
fn main() {
    let ids = set::from_array([2, 4, 6]);
    ids.clear();
    ids.add(9);
    if ids.len() == 1 && ids.has(9) {
        let values = ids.values();
        return values[0];
    }
    return 0;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap set clear method should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();
        let mut budget = ExecutionBudget::unbounded();

        let result = vm
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap set clear method should run");
        assert_eq!(result, OwnedValue::Int(9));
    }

    #[test]
    fn runs_compiled_set_extend_method() {
        let source = r#"
fn main() {
    let tags = set::from_array(["daily", "quest"]);
    tags.extend(set::from_array(["quest", "raid"]));
    if tags.len() == 3 && tags.has("daily") && tags.has("quest") && tags.has("raid") {
        return tags.values().join("|");
    }
    return "";
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("set extend method should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm.run(&code).expect("set extend method should run");
        assert_eq!(result, OwnedValue::String("daily|quest|raid".to_owned()));
    }

    #[test]
    fn managed_heap_execution_runs_set_extend_method() {
        let source = r#"
fn main() {
    let ids = set::from_array([2, 4]);
    let more = set::from_array([4, 6, 8]);
    ids.extend(more);
    if ids.len() == 4 && ids.has(8) {
        return ids.values().sum();
    }
    return 0;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap set extend method should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();
        let mut budget = ExecutionBudget::unbounded();

        let result = vm
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap set extend method should run");
        assert_eq!(result, OwnedValue::Int(20));
    }

    #[test]
    fn set_extend_rejects_non_set_arguments() {
        let source = r#"
fn main() {
    let tags = set::from_array(["quest"]);
    tags.extend(["raid"]);
    return tags.len();
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("set extend error source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let error = vm
            .run(&code)
            .expect_err("set extend should reject non-set args");
        assert_eq!(
            error.kind,
            crate::VmErrorKind::TypeMismatch {
                operation: "method extend"
            }
        );
    }
}
