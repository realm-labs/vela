use std::collections::BTreeMap;

use crate::heap::HeapValue;
use crate::heap_values::script_map_from_string_entries;
use crate::script_set::ScriptSet;
use crate::{
    CallFrame, ExecutionBudget, HeapExecution, SmallStorage, Value, VmError, VmErrorKind, VmResult,
    allocate_heap_value, collection_mutation::check_collection_len, expect_int,
    store_runtime_value,
};
use vela_bytecode::{Constant, ConstantId, LinkedCodeObject, Register};
use vela_common::Span;

pub(crate) fn make_array(
    frame: &mut CallFrame,
    heap: Option<&mut HeapExecution<'_>>,
    mut budget: Option<&mut ExecutionBudget>,
    dst: Register,
    elements: &[Register],
) -> VmResult<()> {
    let Some(heap) = heap else {
        return Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "array heap",
        }));
    };
    let slots = runtime_values_from_registers(frame, elements, heap, budget_ref(&mut budget))?;
    let value = allocate_heap_value(HeapValue::Array(slots), heap, budget_ref(&mut budget))?;
    frame.write(dst, value)
}

pub(crate) fn make_map(
    frame: &mut CallFrame,
    heap: Option<&mut HeapExecution<'_>>,
    mut budget: Option<&mut ExecutionBudget>,
    dst: Register,
    entries: &[(String, Register)],
) -> VmResult<()> {
    let Some(heap) = heap else {
        return Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "map heap",
        }));
    };
    let slots = runtime_map_from_registers(frame, entries, heap, budget_ref(&mut budget))?;
    let slots = script_map_from_string_entries(slots, heap, budget_ref(&mut budget), "map heap")?;
    let value = allocate_heap_value(HeapValue::Map(slots), heap, budget_ref(&mut budget))?;
    frame.write(dst, value)
}

pub(crate) fn make_set_from_array(
    frame: &mut CallFrame,
    heap: Option<&mut HeapExecution<'_>>,
    mut budget: Option<&mut ExecutionBudget>,
    dst: Register,
    src: Register,
) -> VmResult<()> {
    let Some(heap) = heap else {
        return Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "set::from_array",
        }));
    };
    let Value::HeapRef(reference) = frame.read(src)? else {
        return Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "set::from_array",
        }));
    };
    let values = match heap.heap.get(reference) {
        Some(HeapValue::Array(values)) => values.clone(),
        _ => {
            return Err(VmError::new(VmErrorKind::TypeMismatch {
                operation: "set::from_array",
            }));
        }
    };
    let values = ScriptSet::from_values(values, Some(&*heap), "set::from_array")?;
    check_collection_len("set", 0, values.len(), budget.as_deref(), |budget| {
        budget.collection_limits().max_set_len
    })?;
    let value = allocate_heap_value(HeapValue::Set(values), heap, budget_ref(&mut budget))?;
    frame.write(dst, value)
}

pub(crate) fn make_linked_map(
    frame: &mut CallFrame,
    heap: Option<&mut HeapExecution<'_>>,
    mut budget: Option<&mut ExecutionBudget>,
    dst: Register,
    code: &LinkedCodeObject,
    entries: &[(ConstantId, Register)],
    source_span: Option<Span>,
) -> VmResult<()> {
    let Some(heap) = heap else {
        return Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "map heap",
        }));
    };
    let slots = runtime_linked_map_from_registers(
        frame,
        code,
        entries,
        source_span,
        heap,
        budget_ref(&mut budget),
    )?;
    let slots = script_map_from_string_entries(slots, heap, budget_ref(&mut budget), "map heap")?;
    let value = allocate_heap_value(HeapValue::Map(slots), heap, budget_ref(&mut budget))?;
    frame.write(dst, value)
}

pub(crate) fn make_range(
    frame: &mut CallFrame,
    dst: Register,
    start: Register,
    end: Register,
    inclusive: bool,
) -> VmResult<()> {
    let start = expect_int(&frame.read(start)?, "range")?;
    let end = expect_int(&frame.read(end)?, "range")?;
    frame.write(
        dst,
        Value::Range(crate::ranges::RangeValue::new(start, end, inclusive)),
    )
}

#[inline]
fn runtime_values_from_registers(
    frame: &CallFrame,
    registers: &[Register],
    heap: &mut HeapExecution<'_>,
    mut budget: Option<&mut ExecutionBudget>,
) -> VmResult<Vec<Value>> {
    SmallStorage::try_from_slice_map(registers, 8, |register| {
        runtime_value_from_register(frame, *register, heap, budget.as_deref_mut())
    })
    .map(SmallStorage::into_vec)
}

#[inline]
fn runtime_value_from_register(
    frame: &CallFrame,
    register: Register,
    heap: &mut HeapExecution<'_>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    store_runtime_value(&frame.read(register)?, heap, budget)
}

fn runtime_map_from_registers(
    frame: &CallFrame,
    entries: &[(String, Register)],
    heap: &mut HeapExecution<'_>,
    mut budget: Option<&mut ExecutionBudget>,
) -> VmResult<BTreeMap<String, Value>> {
    entries
        .iter()
        .map(|(key, register)| {
            Ok((
                key.clone(),
                store_runtime_value(&frame.read(*register)?, heap, budget.as_deref_mut())?,
            ))
        })
        .collect()
}

fn runtime_linked_map_from_registers(
    frame: &CallFrame,
    code: &LinkedCodeObject,
    entries: &[(ConstantId, Register)],
    source_span: Option<Span>,
    heap: &mut HeapExecution<'_>,
    mut budget: Option<&mut ExecutionBudget>,
) -> VmResult<BTreeMap<String, Value>> {
    entries
        .iter()
        .map(|(key, register)| {
            let Some(Constant::String(key)) = code.constants.get(key.0) else {
                return Err(
                    VmError::new(VmErrorKind::ConstantOutOfBounds { constant: key.0 })
                        .with_source_span_if_absent(source_span),
                );
            };
            Ok((
                key.clone(),
                store_runtime_value(&frame.read(*register)?, heap, budget.as_deref_mut())?,
            ))
        })
        .collect()
}

#[inline]
fn budget_ref<'a>(budget: &'a mut Option<&mut ExecutionBudget>) -> Option<&'a mut ExecutionBudget> {
    match budget {
        Some(budget) => Some(&mut **budget),
        None => None,
    }
}
