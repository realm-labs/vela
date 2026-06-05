use crate::heap::HeapValue;
use crate::ranges::RangeCursor;
use crate::{HeapExecution, Value, VmError, VmErrorKind, VmResult, stored_runtime_value};

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
            IteratorKind::Range(cursor) => cursor.next().map(Value::Int),
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
