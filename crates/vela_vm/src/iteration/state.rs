use crate::heap::GcRef;
use crate::ranges::RangeCursor;
use crate::value::Value;

#[derive(Clone, Debug, PartialEq)]
pub struct IteratorState {
    cursor: IteratorCursor,
}

#[derive(Clone, Debug, PartialEq)]
enum IteratorCursor {
    Values { values: Vec<Value>, next: usize },
    Range(RangeCursor),
}

impl IteratorState {
    #[must_use]
    pub fn from_values(values: Vec<Value>) -> Self {
        Self::values_at(values, 0)
    }

    #[must_use]
    pub fn from_values_at(values: Vec<Value>, next: usize) -> Self {
        Self::values_at(values, next)
    }

    pub(crate) fn from_range_cursor(cursor: RangeCursor) -> Self {
        Self {
            cursor: IteratorCursor::Range(cursor),
        }
    }

    fn values_at(values: Vec<Value>, next: usize) -> Self {
        Self {
            cursor: IteratorCursor::Values { values, next },
        }
    }

    pub(crate) fn next(&mut self) -> Option<Value> {
        match &mut self.cursor {
            IteratorCursor::Values { values, next } => {
                let value = values.get(*next).copied()?;
                *next = next.saturating_add(1);
                Some(value)
            }
            IteratorCursor::Range(cursor) => cursor.next().map(Value::i64),
        }
    }

    pub(crate) fn trace_heap_refs(&self, refs: &mut Vec<GcRef>) {
        match &self.cursor {
            IteratorCursor::Values { values, .. } => {
                values.iter().for_each(|value| value.trace_heap_refs(refs))
            }
            IteratorCursor::Range(_) => {}
        }
    }

    pub(crate) fn values(&self) -> &[Value] {
        match &self.cursor {
            IteratorCursor::Values { values, .. } => values,
            IteratorCursor::Range(_) => &[],
        }
    }

    pub(crate) fn next_index(&self) -> usize {
        match &self.cursor {
            IteratorCursor::Values { next, .. } => *next,
            IteratorCursor::Range(_) => 0,
        }
    }
}
