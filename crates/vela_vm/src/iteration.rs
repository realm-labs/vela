use crate::heap::HeapValue;
use crate::ranges::RangeCursor;
use crate::{HeapExecution, Value, VmError, VmErrorKind, VmResult, value_from_heap_slot};

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
}

pub(crate) fn make_iterator(
    iterable: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<IteratorState> {
    match iterable {
        Value::Array(values) => Ok(IteratorState::new(values.clone())),
        Value::Map(values) => Ok(IteratorState::new(values.values().cloned().collect())),
        Value::Set(values) => Ok(IteratorState::new(values.clone())),
        Value::Range(range) => Ok(IteratorState::range(range.cursor())),
        Value::Iterator(iterator) => Ok(iterator.clone()),
        Value::HeapRef(reference) => {
            let Some(heap_value) = heap.and_then(|heap| heap.heap.get(*reference)) else {
                return Err(VmError::new(VmErrorKind::TypeMismatch {
                    operation: "for in",
                }));
            };
            match heap_value {
                HeapValue::Array(values) | HeapValue::Set(values) => Ok(IteratorState::new(
                    values.iter().map(value_from_heap_slot).collect(),
                )),
                HeapValue::Map(values) => Ok(IteratorState::new(
                    values.values().map(value_from_heap_slot).collect(),
                )),
                HeapValue::String(_) | HeapValue::Record { .. } | HeapValue::Enum { .. } => {
                    Err(VmError::new(VmErrorKind::TypeMismatch {
                        operation: "for in",
                    }))
                }
            }
        }
        Value::Null
        | Value::Missing
        | Value::Bool(_)
        | Value::Int(_)
        | Value::Float(_)
        | Value::String(_)
        | Value::Record { .. }
        | Value::Enum { .. }
        | Value::Closure(_)
        | Value::HostRef(_)
        | Value::PathProxy(_) => Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "for in",
        })),
    }
}
