use crate::heap::GcRef;
use crate::heap::HeapValue;
use crate::heap_execution::HeapExecution;
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
    Array { source: GcRef, len: usize },
    Set { source: GcRef, len: usize },
    MapValues { source: GcRef, keys: Vec<String> },
    StringChars { source: GcRef },
    Range(RangeCursor),
}

impl SequenceSource {
    fn into_iterator(self) -> IteratorState {
        match self {
            Self::Array { source, len } => IteratorState::from_array_source(source, len),
            Self::Set { source, len } => IteratorState::from_set_source(source, len),
            Self::MapValues { source, keys } => IteratorState::from_map_values_source(source, keys),
            Self::StringChars { source } => IteratorState::from_string_chars_source(source),
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
            heap_iterable_source(*reference, heap_value)
        }
        _ => Err(not_iterable()),
    }
}

fn heap_iterable_source(reference: GcRef, value: &HeapValue) -> VmResult<IterableSource> {
    match value {
        HeapValue::Array(values) => Ok(IterableSource::Sequence(SequenceSource::Array {
            source: reference,
            len: values.len(),
        })),
        HeapValue::Set(values) => Ok(IterableSource::Sequence(SequenceSource::Set {
            source: reference,
            len: values.len(),
        })),
        HeapValue::Map(values) => Ok(IterableSource::Sequence(SequenceSource::MapValues {
            source: reference,
            keys: values.keys().cloned().collect(),
        })),
        HeapValue::Iterator(iterator) => Ok(IterableSource::Iterator(iterator.clone())),
        HeapValue::String(_) => Ok(IterableSource::Sequence(SequenceSource::StringChars {
            source: reference,
        })),
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
