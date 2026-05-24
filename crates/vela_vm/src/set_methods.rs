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
        HeapSlot::HostRef(_) => type_error("method set"),
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
