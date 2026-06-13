use crate::heap::GcRef;
use crate::method_runtime::{MethodRuntime, call_callback};
use crate::ranges::RangeCursor;
use crate::runtime_checks::is_truthy;
use crate::{Value, VmResult};

#[derive(Clone, Debug, PartialEq)]
pub struct IteratorState {
    cursor: IteratorCursor,
}

#[derive(Clone, Debug, PartialEq)]
enum IteratorCursor {
    Values {
        values: Vec<Value>,
        next: usize,
    },
    Range(RangeCursor),
    Map {
        source: Box<IteratorState>,
        callback: Value,
    },
    Filter {
        source: Box<IteratorState>,
        callback: Value,
    },
    Take {
        source: Box<IteratorState>,
        remaining: usize,
    },
    Skip {
        source: Box<IteratorState>,
        remaining: usize,
    },
}

impl IteratorState {
    #[must_use]
    pub fn from_values(values: Vec<Value>) -> Self {
        Self::values_at(values, 0)
    }

    #[must_use]
    pub fn empty() -> Self {
        Self::from_values(Vec::new())
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

    pub(crate) fn map(source: Self, callback: Value) -> Self {
        Self {
            cursor: IteratorCursor::Map {
                source: Box::new(source),
                callback,
            },
        }
    }

    pub(crate) fn filter(source: Self, callback: Value) -> Self {
        Self {
            cursor: IteratorCursor::Filter {
                source: Box::new(source),
                callback,
            },
        }
    }

    pub(crate) fn take(source: Self, remaining: usize) -> Self {
        Self {
            cursor: IteratorCursor::Take {
                source: Box::new(source),
                remaining,
            },
        }
    }

    pub(crate) fn skip(source: Self, remaining: usize) -> Self {
        Self {
            cursor: IteratorCursor::Skip {
                source: Box::new(source),
                remaining,
            },
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
            IteratorCursor::Take { source, remaining } => {
                if *remaining == 0 {
                    return None;
                }
                *remaining = remaining.saturating_sub(1);
                source.next()
            }
            IteratorCursor::Skip { source, remaining } => {
                while *remaining > 0 {
                    source.next()?;
                    *remaining = remaining.saturating_sub(1);
                }
                source.next()
            }
            IteratorCursor::Map { .. } | IteratorCursor::Filter { .. } => None,
        }
    }

    pub(crate) fn next_with_runtime(
        &mut self,
        runtime: &mut MethodRuntime<'_, '_, '_>,
        operation: &'static str,
        protected_values: &[Value],
    ) -> VmResult<Option<Value>> {
        match &mut self.cursor {
            IteratorCursor::Values { .. } | IteratorCursor::Range(_) => Ok(self.next()),
            IteratorCursor::Take { source, remaining } => {
                if *remaining == 0 {
                    return Ok(None);
                }
                *remaining = remaining.saturating_sub(1);
                source.next_with_runtime(runtime, operation, protected_values)
            }
            IteratorCursor::Skip { source, remaining } => {
                while *remaining > 0 {
                    if source
                        .next_with_runtime(runtime, operation, protected_values)?
                        .is_none()
                    {
                        return Ok(None);
                    }
                    *remaining = remaining.saturating_sub(1);
                }
                source.next_with_runtime(runtime, operation, protected_values)
            }
            IteratorCursor::Map { source, callback } => {
                let Some(value) = source.next_with_runtime(runtime, operation, protected_values)?
                else {
                    return Ok(None);
                };
                let protected = callback_protected_values(source, *callback, protected_values);
                call_callback(runtime, operation, callback, &[value], &protected).map(Some)
            }
            IteratorCursor::Filter { source, callback } => loop {
                let Some(value) = source.next_with_runtime(runtime, operation, protected_values)?
                else {
                    return Ok(None);
                };
                let protected = callback_protected_values(source, *callback, protected_values);
                let predicate = call_callback(runtime, operation, callback, &[value], &protected)?;
                if is_truthy(&predicate) {
                    return Ok(Some(value));
                }
            },
        }
    }

    pub(crate) fn trace_heap_refs(&self, refs: &mut Vec<GcRef>) {
        match &self.cursor {
            IteratorCursor::Values { values, .. } => {
                values.iter().for_each(|value| value.trace_heap_refs(refs))
            }
            IteratorCursor::Range(_) => {}
            IteratorCursor::Map { source, callback }
            | IteratorCursor::Filter { source, callback } => {
                source.trace_heap_refs(refs);
                callback.trace_heap_refs(refs);
            }
            IteratorCursor::Take { source, .. } | IteratorCursor::Skip { source, .. } => {
                source.trace_heap_refs(refs);
            }
        }
    }

    pub(crate) fn protected_values(&self) -> Vec<Value> {
        let mut values = Vec::new();
        self.push_protected_values(&mut values);
        values
    }

    fn push_protected_values(&self, protected: &mut Vec<Value>) {
        match &self.cursor {
            IteratorCursor::Values { values, .. } => protected.extend(values.iter().copied()),
            IteratorCursor::Range(_) => {}
            IteratorCursor::Map { source, callback }
            | IteratorCursor::Filter { source, callback } => {
                source.push_protected_values(protected);
                protected.push(*callback);
            }
            IteratorCursor::Take { source, .. } | IteratorCursor::Skip { source, .. } => {
                source.push_protected_values(protected);
            }
        }
    }

    pub(crate) fn values(&self) -> &[Value] {
        match &self.cursor {
            IteratorCursor::Values { values, .. } => values,
            IteratorCursor::Range(_)
            | IteratorCursor::Map { .. }
            | IteratorCursor::Filter { .. }
            | IteratorCursor::Take { .. }
            | IteratorCursor::Skip { .. } => &[],
        }
    }

    pub(crate) fn next_index(&self) -> usize {
        match &self.cursor {
            IteratorCursor::Values { next, .. } => *next,
            IteratorCursor::Range(_)
            | IteratorCursor::Map { .. }
            | IteratorCursor::Filter { .. }
            | IteratorCursor::Take { .. }
            | IteratorCursor::Skip { .. } => 0,
        }
    }
}

fn callback_protected_values(
    source: &IteratorState,
    callback: Value,
    outer_protected: &[Value],
) -> Vec<Value> {
    let mut protected = Vec::with_capacity(outer_protected.len().saturating_add(1));
    protected.extend_from_slice(outer_protected);
    protected.extend(source.protected_values());
    protected.push(callback);
    protected
}
