use crate::heap::HeapValue;
use crate::{HeapExecution, Value, VmError, VmErrorKind, VmResult, value_from_heap_slot};

#[derive(Clone, Debug, PartialEq)]
pub struct IteratorState {
    values: Vec<Value>,
    next: usize,
}

impl IteratorState {
    fn new(values: Vec<Value>) -> Self {
        Self { values, next: 0 }
    }

    pub(crate) fn next(&mut self) -> Option<Value> {
        let value = self.values.get(self.next).cloned()?;
        self.next = self.next.saturating_add(1);
        Some(value)
    }

    pub(crate) fn trace_heap_refs(&self, refs: &mut Vec<crate::heap::GcRef>) {
        self.values
            .iter()
            .for_each(|value| value.trace_heap_refs(refs));
    }
}

pub(crate) fn make_iterator(
    iterable: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<IteratorState> {
    match iterable {
        Value::Array(values) => Ok(IteratorState::new(values.clone())),
        Value::Map(values) => Ok(IteratorState::new(values.values().cloned().collect())),
        Value::HeapRef(reference) => {
            let Some(heap_value) = heap.and_then(|heap| heap.heap.get(*reference)) else {
                return Err(VmError::new(VmErrorKind::TypeMismatch {
                    operation: "for in",
                }));
            };
            match heap_value {
                HeapValue::Array(values) => Ok(IteratorState::new(
                    values.iter().map(value_from_heap_slot).collect(),
                )),
                HeapValue::Map(values) => Ok(IteratorState::new(
                    values.values().map(value_from_heap_slot).collect(),
                )),
                HeapValue::String(_)
                | HeapValue::Set(_)
                | HeapValue::Record { .. }
                | HeapValue::Enum { .. } => Err(VmError::new(VmErrorKind::TypeMismatch {
                    operation: "for in",
                })),
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
        | Value::Iterator(_)
        | Value::HostRef(_) => Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "for in",
        })),
    }
}
