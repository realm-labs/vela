use crate::value_key::ValueKey;
use crate::{
    ExecutionBudget, HeapExecution, StandardMethodReceiver, Value, VmError, VmErrorKind, VmResult,
};

pub(super) struct SequenceCompletion {
    pub(super) receiver: StandardMethodReceiver,
    pub(super) source: Value,
    pub(super) values: Vec<Value>,
    pub(super) retain: Vec<bool>,
}

pub(super) struct MapCompletion {
    pub(super) source: Value,
    pub(super) entries: Vec<(Value, Value)>,
    pub(super) retain: Vec<bool>,
}

pub(super) fn complete_sequence(
    completion: SequenceCompletion,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<()> {
    let SequenceCompletion {
        receiver,
        source,
        values,
        retain,
    } = completion;
    if values.len() != retain.len() {
        return incomplete();
    }
    apply_owned_sequence(receiver, source, &values, &retain, heap, budget, operation)?;
    Ok(())
}

pub(super) fn complete_map(
    completion: MapCompletion,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<()> {
    let MapCompletion {
        source,
        entries,
        retain,
    } = completion;
    if entries.len() != retain.len() {
        return incomplete();
    }
    apply_owned_map(source, &entries, &retain, heap, budget, operation)?;
    Ok(())
}

fn apply_owned_sequence(
    receiver: StandardMethodReceiver,
    source: Value,
    values: &[Value],
    retain: &[bool],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<()> {
    let Value::HeapRef(reference) = source else {
        return incomplete();
    };
    match receiver {
        StandardMethodReceiver::Array => {
            let Some(heap) = heap.as_deref_mut() else {
                return incomplete();
            };
            crate::collection_mutation::retain_array_slots(
                heap,
                reference,
                values.len(),
                retain,
                budget.as_deref_mut(),
                operation,
            )
        }
        StandardMethodReceiver::Set => {
            let expected = value_keys(values, heap.as_deref(), operation)?;
            let keep = values
                .iter()
                .zip(retain)
                .filter(|(_, keep)| **keep)
                .map(|(value, _)| ValueKey::from_value(value, heap.as_deref(), operation))
                .collect::<VmResult<std::collections::BTreeSet<_>>>()?;
            let Some(heap) = heap.as_deref_mut() else {
                return incomplete();
            };
            crate::collection_mutation::retain_set_slots(
                heap,
                reference,
                &expected,
                &keep,
                budget.as_deref_mut(),
                operation,
            )
        }
        _ => incomplete(),
    }
}

fn apply_owned_map(
    source: Value,
    entries: &[(Value, Value)],
    retain: &[bool],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<()> {
    let Value::HeapRef(reference) = source else {
        return incomplete();
    };
    let expected = entries
        .iter()
        .map(|(key, _)| ValueKey::from_value(key, heap.as_deref(), operation))
        .collect::<VmResult<std::collections::BTreeSet<_>>>()?;
    let keep = entries
        .iter()
        .zip(retain)
        .filter(|(_, keep)| **keep)
        .map(|((key, _), _)| ValueKey::from_value(key, heap.as_deref(), operation))
        .collect::<VmResult<std::collections::BTreeSet<_>>>()?;
    let Some(heap) = heap.as_deref_mut() else {
        return incomplete();
    };
    crate::collection_mutation::retain_map_slots(
        heap,
        reference,
        &expected,
        &keep,
        budget.as_deref_mut(),
        operation,
    )
}

fn value_keys(
    values: &[Value],
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<std::collections::BTreeSet<ValueKey>> {
    values
        .iter()
        .map(|value| ValueKey::from_value(value, heap, operation))
        .collect()
}

fn incomplete<T>() -> VmResult<T> {
    Err(VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
        opcode: "incomplete callback method state",
    }))
}
