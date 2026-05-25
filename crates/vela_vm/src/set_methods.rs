use crate::heap::{HeapSlot, HeapValue};
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

pub(crate) fn values(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("values", args, 0)?;
    set_values(receiver, heap, "method values").map(Value::Array)
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
    let required = set.from_array(["daily", "quest"]);
    if unioned == "bonus,daily,quest,raid"
        && shared == "daily,quest"
        && missing == "raid"
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
    let required = set.from_array([1, 3]);
    let excluded = set.from_array([9]);
    if !required.is_subset(base) || !base.is_superset(required) || !base.is_disjoint(excluded) {
        return -1;
    }
    return unioned.values().sum()
        + shared.values().sum()
        + only_base.values().sum();
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
        assert_eq!(result, Value::Int(26));
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
    if required.is_subset(player)
        && player.is_superset(required)
        && player.is_disjoint(set.from_array(["bonus"]))
        && unioned == "bonus,daily,quest,raid"
        && shared == "quest"
        && missing == "raid"
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
}
