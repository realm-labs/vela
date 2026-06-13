use crate::heap::HeapValue;
use crate::heap_values::allocate_heap_value;
use crate::ranges::RangeCursor;
use crate::runtime_checks::{expect_int, validate_jump};
use crate::{
    CallFrame, ExecutionBudget, HeapExecution, Value, VmError, VmErrorKind, VmResult,
    stored_runtime_value,
};
use vela_bytecode::{InstructionOffset, LinkedCodeObject, Register, UnlinkedCodeObject};

pub(crate) struct IterRuntime<'a, 'heap> {
    pub(crate) frame: &'a mut CallFrame,
    pub(crate) heap: Option<&'a mut HeapExecution<'heap>>,
    pub(crate) budget: Option<&'a mut ExecutionBudget>,
}

pub(crate) struct RangeNextStep {
    pub(crate) cursor: Register,
    pub(crate) end: Register,
    pub(crate) done: Register,
    pub(crate) inclusive: bool,
    pub(crate) dst: Register,
    pub(crate) jump_if_done: InstructionOffset,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IteratorState {
    kind: IteratorKind,
}

#[derive(Clone, Debug, PartialEq)]
enum IteratorKind {
    Values { values: Vec<Value>, next: usize },
    Range(RangeCursor),
}

impl IteratorState {
    #[must_use]
    pub fn from_values(values: Vec<Value>) -> Self {
        Self::new(values)
    }

    #[must_use]
    pub fn from_values_at(values: Vec<Value>, next: usize) -> Self {
        Self {
            kind: IteratorKind::Values { values, next },
        }
    }

    fn new(values: Vec<Value>) -> Self {
        Self {
            kind: IteratorKind::Values { values, next: 0 },
        }
    }

    fn range(cursor: RangeCursor) -> Self {
        Self {
            kind: IteratorKind::Range(cursor),
        }
    }

    pub(crate) fn next(&mut self) -> Option<Value> {
        match &mut self.kind {
            IteratorKind::Values { values, next } => {
                let value = values.get(*next).cloned()?;
                *next = next.saturating_add(1);
                Some(value)
            }
            IteratorKind::Range(cursor) => cursor.next().map(Value::i64),
        }
    }

    pub(crate) fn trace_heap_refs(&self, refs: &mut Vec<crate::heap::GcRef>) {
        match &self.kind {
            IteratorKind::Values { values, .. } => {
                values.iter().for_each(|value| value.trace_heap_refs(refs))
            }
            IteratorKind::Range(_) => {}
        }
    }

    pub(crate) fn values(&self) -> &[Value] {
        match &self.kind {
            IteratorKind::Values { values, .. } => values,
            IteratorKind::Range(_) => &[],
        }
    }

    pub(crate) fn next_index(&self) -> usize {
        match &self.kind {
            IteratorKind::Values { next, .. } => *next,
            IteratorKind::Range(_) => 0,
        }
    }
}

pub(crate) fn make_iterator(
    iterable: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<IteratorState> {
    match iterable {
        Value::Range(range) => Ok(IteratorState::range(range.cursor())),
        Value::HeapRef(reference) => {
            let Some(heap_value) = heap.and_then(|heap| heap.heap.get(*reference)) else {
                return Err(VmError::new(VmErrorKind::TypeMismatch {
                    operation: "for in",
                }));
            };
            match heap_value {
                HeapValue::Array(values) | HeapValue::Set(values) => Ok(IteratorState::new(
                    values.iter().map(stored_runtime_value).collect(),
                )),
                HeapValue::Map(values) => Ok(IteratorState::new(
                    values.values().map(stored_runtime_value).collect(),
                )),
                HeapValue::Iterator(iterator) => Ok(iterator.clone()),
                HeapValue::String(_)
                | HeapValue::Bytes(_)
                | HeapValue::Record { .. }
                | HeapValue::Enum { .. }
                | HeapValue::Closure(_)
                | HeapValue::PathProxy(_) => Err(VmError::new(VmErrorKind::TypeMismatch {
                    operation: "for in",
                })),
            }
        }
        _ => Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "for in",
        })),
    }
}

pub(crate) fn dispatch_iter_init(
    mut runtime: IterRuntime<'_, '_>,
    dst: Register,
    iterable: Register,
) -> VmResult<()> {
    let iterator = make_iterator(runtime.frame.read(iterable)?, runtime.heap.as_deref())?;
    let Some(heap) = heap_ref(&mut runtime.heap) else {
        return Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "iterator heap",
        }));
    };
    let value = allocate_heap_value(
        HeapValue::Iterator(iterator),
        heap,
        budget_ref(&mut runtime.budget),
    )?;
    runtime.frame.write(dst, value)
}

pub(crate) fn dispatch_iter_next(
    mut runtime: IterRuntime<'_, '_>,
    code: &UnlinkedCodeObject,
    iterator: Register,
    dst: Register,
    jump_if_done: InstructionOffset,
) -> VmResult<Option<usize>> {
    let next = next_iterator_value(&mut runtime, iterator)?;
    match next {
        Some(value) => {
            runtime.frame.write(dst, value)?;
            Ok(None)
        }
        None => {
            validate_jump(code, jump_if_done.0)?;
            Ok(Some(jump_if_done.0))
        }
    }
}

pub(crate) fn dispatch_linked_iter_next(
    mut runtime: IterRuntime<'_, '_>,
    code: &LinkedCodeObject,
    iterator: Register,
    dst: Register,
    jump_if_done: InstructionOffset,
) -> VmResult<Option<usize>> {
    let next = next_iterator_value(&mut runtime, iterator)?;
    match next {
        Some(value) => {
            runtime.frame.write(dst, value)?;
            Ok(None)
        }
        None => {
            debug_assert!(jump_if_done.0 <= code.instructions.len());
            Ok(Some(jump_if_done.0))
        }
    }
}

pub(crate) fn dispatch_range_next(
    runtime: IterRuntime<'_, '_>,
    code: &UnlinkedCodeObject,
    step: RangeNextStep,
) -> VmResult<Option<usize>> {
    dispatch_range_next_with(runtime.frame, step, |offset| validate_jump(code, offset))
}

pub(crate) fn dispatch_linked_range_next(
    runtime: IterRuntime<'_, '_>,
    code: &LinkedCodeObject,
    step: RangeNextStep,
) -> VmResult<Option<usize>> {
    dispatch_range_next_with(runtime.frame, step, |offset| {
        debug_assert!(offset <= code.instructions.len());
        Ok(())
    })
}

fn next_iterator_value(
    runtime: &mut IterRuntime<'_, '_>,
    iterator: Register,
) -> VmResult<Option<Value>> {
    let value = *runtime.frame.read(iterator)?;
    match value {
        Value::HeapRef(reference) => {
            let Some(HeapValue::Iterator(iterator_state)) =
                heap_ref(&mut runtime.heap).and_then(|heap| heap.heap.get_mut(reference).ok())
            else {
                return Err(VmError::new(VmErrorKind::TypeMismatch {
                    operation: "iterator",
                }));
            };
            Ok(iterator_state.next())
        }
        _ => Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "iterator",
        })),
    }
}

fn dispatch_range_next_with(
    frame: &mut CallFrame,
    step: RangeNextStep,
    mut validate: impl FnMut(usize) -> VmResult<()>,
) -> VmResult<Option<usize>> {
    let is_done = match frame.read(step.done)? {
        Value::Bool(value) => *value,
        _ => {
            return Err(VmError::new(VmErrorKind::TypeMismatch {
                operation: "range",
            }));
        }
    };
    if is_done {
        validate(step.jump_if_done.0)?;
        return Ok(Some(step.jump_if_done.0));
    }

    let current = expect_int(frame.read(step.cursor)?, "range")?;
    let end = expect_int(frame.read(step.end)?, "range")?;
    let has_next = if step.inclusive {
        current <= end
    } else {
        current < end
    };
    if has_next {
        frame.write(
            step.dst,
            Value::Scalar(vela_common::ScalarValue::I64(current)),
        )?;
        if current == i64::MAX {
            frame.write(step.done, Value::Bool(true))?;
        } else {
            frame.write(
                step.cursor,
                Value::Scalar(vela_common::ScalarValue::I64(current + 1)),
            )?;
        }
        Ok(None)
    } else {
        frame.write(step.done, Value::Bool(true))?;
        validate(step.jump_if_done.0)?;
        Ok(Some(step.jump_if_done.0))
    }
}

#[inline]
fn heap_ref<'a, 'heap>(
    heap: &'a mut Option<&mut HeapExecution<'heap>>,
) -> Option<&'a mut HeapExecution<'heap>> {
    match heap {
        Some(heap) => Some(&mut **heap),
        None => None,
    }
}

#[inline]
fn budget_ref<'a>(budget: &'a mut Option<&mut ExecutionBudget>) -> Option<&'a mut ExecutionBudget> {
    match budget {
        Some(budget) => Some(&mut **budget),
        None => None,
    }
}
