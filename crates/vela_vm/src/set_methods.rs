use crate::heap::{HeapSlot, HeapValue};
use crate::method_runtime::{MethodRuntime, call_callback};
use crate::option_result::option_value;
use crate::{
    ExecutionBudget, HeapExecution, Value, VmError, VmErrorKind, VmResult, value_from_heap_slot,
    value_to_heap_slot,
};

pub(crate) fn from_array(args: &[Value]) -> VmResult<Value> {
    expect_arity("set.from_array", args, 1)?;
    let Value::Array(values) = &args[0] else {
        return type_error("set.from_array");
    };
    let mut set = Vec::new();
    for value in values {
        push_unique(&mut set, value.clone(), None, "set.from_array")?;
    }
    Ok(Value::Set(set))
}

pub(crate) fn has(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<bool> {
    expect_arity("has", args, 1)?;
    let key = SetKey::from_value(&args[0], heap, "method has")?;
    let values = set_values(receiver, heap, "method has")?;
    Ok(values
        .iter()
        .any(|value| SetKey::from_value(value, heap, "method has").as_ref() == Ok(&key)))
}

pub(crate) fn add(
    receiver: &mut Value,
    args: &[Value],
    heap: Option<&mut HeapExecution<'_>>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("add", args, 1)?;
    match receiver {
        Value::Set(values) => Ok(Value::Bool(push_unique(
            values,
            args[0].clone(),
            None,
            "method add",
        )?)),
        Value::HeapRef(reference) => {
            let Some(heap) = heap else {
                return type_error("method add");
            };
            let key = SetKey::from_value(&args[0], Some(&*heap), "method add")?;
            let Some(HeapValue::Set(values)) = heap.heap.get(*reference) else {
                return type_error("method add");
            };
            if values
                .iter()
                .any(|value| slot_key(value, &*heap).as_ref() == Ok(&key))
            {
                return Ok(Value::Bool(false));
            }
            let slot = value_to_heap_slot(&args[0], heap, budget)?;
            let Some(HeapValue::Set(values)) = heap.heap.get_mut(*reference).ok() else {
                return type_error("method add");
            };
            values.push(slot);
            Ok(Value::Bool(true))
        }
        _ => type_error("method add"),
    }
}

pub(crate) fn remove(
    receiver: &mut Value,
    args: &[Value],
    heap: Option<&mut HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("remove", args, 1)?;
    match receiver {
        Value::Set(values) => {
            let key = SetKey::from_value(&args[0], None, "method remove")?;
            let before = values.len();
            values.retain(|value| {
                SetKey::from_value(value, None, "method remove").as_ref() != Ok(&key)
            });
            Ok(Value::Bool(values.len() != before))
        }
        Value::HeapRef(reference) => {
            let Some(heap) = heap else {
                return type_error("method remove");
            };
            let key = SetKey::from_value(&args[0], Some(&*heap), "method remove")?;
            let Some(HeapValue::Set(values)) = heap.heap.get(*reference) else {
                return type_error("method remove");
            };
            let indexes = values
                .iter()
                .enumerate()
                .filter_map(|(index, value)| {
                    (slot_key(value, &*heap).as_ref() == Ok(&key)).then_some(index)
                })
                .collect::<Vec<_>>();
            let Some(HeapValue::Set(values)) = heap.heap.get_mut(*reference).ok() else {
                return type_error("method remove");
            };
            let before = values.len();
            for index in indexes.into_iter().rev() {
                values.remove(index);
            }
            Ok(Value::Bool(values.len() != before))
        }
        _ => type_error("method remove"),
    }
}

pub(crate) fn clear(
    receiver: &mut Value,
    args: &[Value],
    heap: Option<&mut HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("clear", args, 0)?;
    match receiver {
        Value::Set(values) => {
            values.clear();
            Ok(Value::Null)
        }
        Value::HeapRef(reference) => {
            let Some(heap) = heap else {
                return type_error("method clear");
            };
            let Some(HeapValue::Set(values)) = heap.heap.get_mut(*reference).ok() else {
                return type_error("method clear");
            };
            values.clear();
            Ok(Value::Null)
        }
        _ => type_error("method clear"),
    }
}

pub(crate) fn extend(
    receiver: &mut Value,
    args: &[Value],
    heap: Option<&mut HeapExecution<'_>>,
    mut budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("extend", args, 1)?;
    let extension = set_values(&args[0], heap.as_deref(), "method extend")?;
    match receiver {
        Value::Set(values) => {
            for value in extension {
                push_unique(values, value, None, "method extend")?;
            }
            Ok(Value::Null)
        }
        Value::HeapRef(reference) => {
            let Some(heap) = heap else {
                return type_error("method extend");
            };
            let Some(HeapValue::Set(values)) = heap.heap.get(*reference) else {
                return type_error("method extend");
            };
            let mut keys = values
                .iter()
                .map(|slot| slot_key(slot, &*heap))
                .collect::<VmResult<Vec<_>>>()?;
            let mut slots = Vec::new();
            for value in extension {
                let key = SetKey::from_value(&value, Some(&*heap), "method extend")?;
                if keys.contains(&key) {
                    continue;
                }
                keys.push(key);
                slots.push(value_to_heap_slot(&value, heap, budget.as_deref_mut())?);
            }
            let Some(HeapValue::Set(values)) = heap.heap.get_mut(*reference).ok() else {
                return type_error("method extend");
            };
            values.extend(slots);
            Ok(Value::Null)
        }
        _ => type_error("method extend"),
    }
}

pub(crate) fn values(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("values", args, 0)?;
    set_values(receiver, heap, "method values").map(Value::Array)
}

pub(crate) fn map(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("map", args, 1)?;
    let values = set_values(receiver, runtime.heap.as_deref(), "method map")?;
    let mut mapped = Vec::new();
    for value in values {
        let mapped_value = call_callback(
            &mut runtime,
            "method map",
            &args[0],
            std::slice::from_ref(&value),
            &mapped,
        )?;
        push_unique(
            &mut mapped,
            mapped_value,
            runtime.heap.as_deref(),
            "method map",
        )?;
    }
    Ok(Value::Set(mapped))
}

pub(crate) fn filter(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("filter", args, 1)?;
    let values = set_values(receiver, runtime.heap.as_deref(), "method filter")?;
    let mut filtered = Vec::new();
    for value in values {
        let predicate = call_callback(
            &mut runtime,
            "method filter",
            &args[0],
            std::slice::from_ref(&value),
            &filtered,
        )?;
        if is_truthy(&predicate) {
            push_unique(
                &mut filtered,
                value,
                runtime.heap.as_deref(),
                "method filter",
            )?;
        }
    }
    Ok(Value::Set(filtered))
}

pub(crate) fn find(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("find", args, 1)?;
    for value in set_values(receiver, runtime.heap.as_deref(), "method find")? {
        let predicate = call_callback(
            &mut runtime,
            "method find",
            &args[0],
            std::slice::from_ref(&value),
            &[],
        )?;
        if is_truthy(&predicate) {
            return Ok(option_value(Some(value)));
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
    for value in set_values(receiver, runtime.heap.as_deref(), "method any")? {
        let predicate = call_callback(
            &mut runtime,
            "method any",
            &args[0],
            std::slice::from_ref(&value),
            &[],
        )?;
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
    for value in set_values(receiver, runtime.heap.as_deref(), "method all")? {
        let predicate = call_callback(
            &mut runtime,
            "method all",
            &args[0],
            std::slice::from_ref(&value),
            &[],
        )?;
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
    let mut count = 0_i64;
    for value in set_values(receiver, runtime.heap.as_deref(), "method count")? {
        let predicate = call_callback(
            &mut runtime,
            "method count",
            &args[0],
            std::slice::from_ref(&value),
            &[],
        )?;
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

pub(crate) fn union(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("union", args, 1)?;
    let mut combined = Vec::new();
    for value in set_values(receiver, heap, "method union")? {
        push_unique(&mut combined, value, heap, "method union")?;
    }
    for value in set_values(&args[0], heap, "method union")? {
        push_unique(&mut combined, value, heap, "method union")?;
    }
    Ok(Value::Set(combined))
}

pub(crate) fn intersection(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("intersection", args, 1)?;
    let right = set_keys(
        &set_values(&args[0], heap, "method intersection")?,
        heap,
        "method intersection",
    )?;
    let mut result = Vec::new();
    for value in set_values(receiver, heap, "method intersection")? {
        let key = SetKey::from_value(&value, heap, "method intersection")?;
        if right.contains(&key) {
            push_unique(&mut result, value, heap, "method intersection")?;
        }
    }
    Ok(Value::Set(result))
}

pub(crate) fn difference(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("difference", args, 1)?;
    let right = set_keys(
        &set_values(&args[0], heap, "method difference")?,
        heap,
        "method difference",
    )?;
    let mut result = Vec::new();
    for value in set_values(receiver, heap, "method difference")? {
        let key = SetKey::from_value(&value, heap, "method difference")?;
        if !right.contains(&key) {
            push_unique(&mut result, value, heap, "method difference")?;
        }
    }
    Ok(Value::Set(result))
}

pub(crate) fn symmetric_difference(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("symmetric_difference", args, 1)?;
    let left_values = set_values(receiver, heap, "method symmetric_difference")?;
    let right_values = set_values(&args[0], heap, "method symmetric_difference")?;
    let left_keys = set_keys(&left_values, heap, "method symmetric_difference")?;
    let right_keys = set_keys(&right_values, heap, "method symmetric_difference")?;

    let mut result = Vec::new();
    for (value, key) in left_values.into_iter().zip(left_keys.iter()) {
        if !right_keys.contains(key) {
            push_unique(&mut result, value, heap, "method symmetric_difference")?;
        }
    }
    for (value, key) in right_values.into_iter().zip(right_keys.iter()) {
        if !left_keys.contains(key) {
            push_unique(&mut result, value, heap, "method symmetric_difference")?;
        }
    }
    Ok(Value::Set(result))
}

pub(crate) fn is_subset(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<bool> {
    expect_arity("is_subset", args, 1)?;
    set_contains_all(receiver, &args[0], heap, "method is_subset")
}

pub(crate) fn is_superset(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<bool> {
    expect_arity("is_superset", args, 1)?;
    set_contains_all(&args[0], receiver, heap, "method is_superset")
}

pub(crate) fn is_disjoint(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<bool> {
    expect_arity("is_disjoint", args, 1)?;
    let right = set_keys(
        &set_values(&args[0], heap, "method is_disjoint")?,
        heap,
        "method is_disjoint",
    )?;
    for value in set_values(receiver, heap, "method is_disjoint")? {
        let key = SetKey::from_value(&value, heap, "method is_disjoint")?;
        if right.contains(&key) {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(crate) fn is_set(receiver: &Value, heap: Option<&HeapExecution<'_>>) -> bool {
    match receiver {
        Value::Set(_) => true,
        Value::HeapRef(reference) => {
            matches!(
                heap.and_then(|heap| heap.heap.get(*reference)),
                Some(HeapValue::Set(_))
            )
        }
        _ => false,
    }
}

fn set_contains_all(
    subset: &Value,
    superset: &Value,
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<bool> {
    let superset = set_keys(&set_values(superset, heap, operation)?, heap, operation)?;
    for value in set_values(subset, heap, operation)? {
        let key = SetKey::from_value(&value, heap, operation)?;
        if !superset.contains(&key) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn push_unique(
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

fn set_values(
    receiver: &Value,
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<Vec<Value>> {
    match receiver {
        Value::Set(values) => Ok(values.clone()),
        Value::HeapRef(reference) => {
            let Some(HeapValue::Set(values)) = heap.and_then(|heap| heap.heap.get(*reference))
            else {
                return type_error(operation);
            };
            Ok(values.iter().map(value_from_heap_slot).collect())
        }
        _ => type_error(operation),
    }
}

fn set_keys(
    values: &[Value],
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<Vec<SetKey>> {
    values
        .iter()
        .map(|value| SetKey::from_value(value, heap, operation))
        .collect()
}

#[derive(Clone, Debug, PartialEq)]
enum SetKey {
    Null,
    Bool(bool),
    Int(i64),
    Float(u64),
    String(String),
}

impl SetKey {
    fn from_value(
        value: &Value,
        heap: Option<&HeapExecution<'_>>,
        operation: &'static str,
    ) -> VmResult<Self> {
        match value {
            Value::Null => Ok(Self::Null),
            Value::Bool(value) => Ok(Self::Bool(*value)),
            Value::Int(value) => Ok(Self::Int(*value)),
            Value::Float(value) if value.is_finite() => Ok(Self::Float(value.to_bits())),
            Value::String(value) => Ok(Self::String(value.clone())),
            Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
                Some(HeapValue::String(value)) => Ok(Self::String(value.clone())),
                _ => type_error(operation),
            },
            _ => type_error(operation),
        }
    }
}

fn slot_key(slot: &HeapSlot, heap: &HeapExecution<'_>) -> VmResult<SetKey> {
    match slot {
        HeapSlot::Null => Ok(SetKey::Null),
        HeapSlot::Bool(value) => Ok(SetKey::Bool(*value)),
        HeapSlot::Int(value) => Ok(SetKey::Int(*value)),
        HeapSlot::Float(value) if value.is_finite() => Ok(SetKey::Float(value.to_bits())),
        HeapSlot::Ref(reference) => match heap.heap.get(*reference) {
            Some(HeapValue::String(value)) => Ok(SetKey::String(value.clone())),
            _ => type_error("method set"),
        },
        HeapSlot::HostRef(_) | HeapSlot::PathProxy(_) => type_error("method set"),
        HeapSlot::Float(_) => type_error("method set"),
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

fn type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}

fn is_truthy(value: &Value) -> bool {
    !matches!(value, Value::Bool(false) | Value::Null)
}

#[cfg(test)]
mod tests {
    use vela_bytecode::compiler::compile_function_source;
    use vela_common::SourceId;

    use crate::{ExecutionBudget, Value, Vm};

    #[test]
    fn runs_compiled_set_combination_methods() {
        let source = r#"
fn main() {
    let player = set.from_array(["daily", "quest", "raid"]);
    let event = set.from_array(["quest", "bonus", "daily"]);
    let unioned = player.union(event).values().sort_by(|tag| tag).join(",");
    let shared = player.intersection(event).values().sort_by(|tag| tag).join(",");
    let missing = player.difference(event).values().join(",");
    let changed = player.symmetric_difference(event).values().sort_by(|tag| tag).join(",");
    let required = set.from_array(["daily", "quest"]);
    if unioned == "bonus,daily,quest,raid"
        && shared == "daily,quest"
        && missing == "raid"
        && changed == "bonus,raid"
        && required.is_subset(player)
        && player.is_superset(required)
        && player.is_disjoint(set.from_array(["bonus"]))
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
        assert_eq!(result, Value::String("daily,quest".to_owned()));
    }

    #[test]
    fn managed_heap_execution_runs_set_combination_methods() {
        let source = r#"
fn main() {
    let base = set.from_array([1, 2, 3, 5]);
    let bonus = set.from_array([2, 4, 5]);
    let unioned = base.union(bonus);
    let shared = base.intersection(bonus);
    let only_base = base.difference(bonus);
    let changed = base.symmetric_difference(bonus);
    let required = set.from_array([1, 3]);
    let excluded = set.from_array([9]);
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
        assert_eq!(result, Value::Int(34));
    }

    #[test]
    fn managed_heap_execution_runs_string_set_predicates() {
        let source = r#"
fn main() {
    let player = set.from_array(["daily", "quest", "raid"]);
    let required = set.from_array(["daily", "quest"]);
    let event = set.from_array(["quest", "bonus"]);
    let unioned = player.union(event).values().sort_by(|tag| tag).join(",");
    let shared = player.intersection(event).values().sort_by(|tag| tag).join(",");
    let missing = player.difference(required).values().sort_by(|tag| tag).join(",");
    let changed = player.symmetric_difference(event).values().sort_by(|tag| tag).join(",");
    if required.is_subset(player)
        && player.is_superset(required)
        && player.is_disjoint(set.from_array(["bonus"]))
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
        assert_eq!(result, Value::String("bonus,daily,quest,raid".to_owned()));
    }

    #[test]
    fn runs_compiled_set_filter_method() {
        let source = r#"
fn main() {
    let tags = set.from_array(["daily", "quest", "raid", "daily"]);
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
        assert_eq!(result, Value::String("quest,raid".to_owned()));
    }

    #[test]
    fn managed_heap_execution_runs_set_filter_method() {
        let source = r#"
fn main() {
    let ids = set.from_array([1, 2, 3, 4, 5]);
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
        assert_eq!(result, Value::Int(23));
    }

    #[test]
    fn runs_compiled_set_map_method() {
        let source = r#"
fn main() {
    let tags = set.from_array(["daily", "quest", "raid"]);
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
        assert_eq!(result, Value::String("DAILY,QUEST,RAID".to_owned()));
    }

    #[test]
    fn managed_heap_execution_runs_set_map_method() {
        let source = r#"
fn main() {
    let ids = set.from_array([1, 2, 3, 4]);
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
        assert_eq!(result, Value::Int(21));
    }

    #[test]
    fn runs_compiled_set_higher_order_predicates() {
        let source = r#"
fn main() {
    let tags = set.from_array(["daily", "quest", "raid"]);
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
        assert_eq!(result, Value::String("quest".to_owned()));
    }

    #[test]
    fn managed_heap_execution_runs_set_higher_order_predicates() {
        let source = r#"
fn main() {
    let ids = set.from_array([2, 4, 6, 9]);
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
        assert_eq!(result, Value::Int(27));
    }

    #[test]
    fn set_filter_rejects_non_callback_args() {
        let source = r#"
fn main() {
    let tags = set.from_array(["quest"]);
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
    let tags = set.from_array(["quest"]);
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
    let tags = set.from_array(["quest"]);
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
    let tags = set.from_array(["quest"]);
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
    let tags = set.from_array(["daily", "quest"]);
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
        assert_eq!(result, Value::String("raid".to_owned()));
    }

    #[test]
    fn managed_heap_execution_runs_set_clear_method() {
        let source = r#"
fn main() {
    let ids = set.from_array([2, 4, 6]);
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
        assert_eq!(result, Value::Int(9));
    }

    #[test]
    fn runs_compiled_set_extend_method() {
        let source = r#"
fn main() {
    let tags = set.from_array(["daily", "quest"]);
    tags.extend(set.from_array(["quest", "raid"]));
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
        assert_eq!(result, Value::String("daily|quest|raid".to_owned()));
    }

    #[test]
    fn managed_heap_execution_runs_set_extend_method() {
        let source = r#"
fn main() {
    let ids = set.from_array([2, 4]);
    let more = set.from_array([4, 6, 8]);
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
        assert_eq!(result, Value::Int(20));
    }

    #[test]
    fn set_extend_rejects_non_set_arguments() {
        let source = r#"
fn main() {
    let tags = set.from_array(["quest"]);
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
