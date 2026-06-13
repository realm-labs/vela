use crate::heap::HeapValue;
use crate::heap_execution::HeapExecution;
use crate::heap_values::stored_runtime_value;
use crate::ranges::RangeCursor;
use crate::{Value, VmError, VmErrorKind, VmResult};

use super::IteratorState;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum IterableSource {
    Sequence(SequenceSource),
    Iterator(IteratorState),
}

impl IterableSource {
    fn into_iterator(self) -> IteratorState {
        match self {
            Self::Sequence(source) => source.into_iterator(),
            Self::Iterator(iterator) => iterator,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum SequenceSource {
    Values(Vec<Value>),
    Range(RangeCursor),
}

impl SequenceSource {
    fn into_iterator(self) -> IteratorState {
        match self {
            Self::Values(values) => IteratorState::from_values(values),
            Self::Range(cursor) => IteratorState::from_range_cursor(cursor),
        }
    }
}

pub(crate) fn make_iterator(
    iterable: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<IteratorState> {
    classify_iterable(iterable, heap).map(IterableSource::into_iterator)
}

fn classify_iterable(
    iterable: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<IterableSource> {
    match iterable {
        Value::Range(range) => Ok(IterableSource::Sequence(SequenceSource::Range(
            range.cursor(),
        ))),
        Value::HeapRef(reference) => {
            let Some(heap_value) = heap.and_then(|heap| heap.heap.get(*reference)) else {
                return Err(not_iterable());
            };
            heap_iterable_source(heap_value)
        }
        _ => Err(not_iterable()),
    }
}

fn heap_iterable_source(value: &HeapValue) -> VmResult<IterableSource> {
    match value {
        HeapValue::Array(values) | HeapValue::Set(values) => Ok(IterableSource::Sequence(
            SequenceSource::Values(values.iter().map(stored_runtime_value).collect()),
        )),
        HeapValue::Map(values) => Ok(IterableSource::Sequence(SequenceSource::Values(
            values.values().map(stored_runtime_value).collect(),
        ))),
        HeapValue::Iterator(iterator) => Ok(IterableSource::Iterator(iterator.clone())),
        HeapValue::String(value) => Ok(IterableSource::Sequence(SequenceSource::Values(
            value.chars().map(Value::Char).collect(),
        ))),
        HeapValue::Bytes(_)
        | HeapValue::Record { .. }
        | HeapValue::Enum { .. }
        | HeapValue::Closure(_)
        | HeapValue::PathProxy(_) => Err(not_iterable()),
    }
}

fn not_iterable() -> VmError {
    VmError::new(VmErrorKind::TypeMismatch {
        operation: "for in",
    })
}
