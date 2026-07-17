use crate::heap::GcRef;
use crate::heap::HeapValue;
use crate::heap_values::stored_runtime_value;
use crate::method_runtime::{MethodRuntime, call_callback};
use crate::ranges::RangeCursor;
use crate::runtime_checks::is_truthy;
use crate::value_key::ValueKey;
use crate::{ExecutionBudget, HeapExecution, Value, VmError, VmErrorKind, VmResult};
use vela_bytecode::{LinkedProgram, TypeGuardPlan};

#[derive(Clone, Debug, PartialEq)]
pub struct IteratorState {
    cursor: IteratorCursor,
    item_guards: Vec<IteratorItemGuard>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct IteratorItemGuard {
    pub(crate) plan: TypeGuardPlan,
    pub(crate) debug_name: String,
}

impl IteratorItemGuard {
    pub(crate) fn linked(plan: TypeGuardPlan, debug_name: String) -> Self {
        Self { plan, debug_name }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum IteratorCursor {
    Values {
        values: Vec<Value>,
        next: usize,
    },
    Array {
        source: GcRef,
        next: usize,
        len: usize,
    },
    Set {
        source: GcRef,
        next: usize,
        len: usize,
    },
    MapValues {
        source: GcRef,
        keys: Vec<ValueKey>,
        next: usize,
    },
    MapKeys {
        source: GcRef,
        keys: Vec<ValueKey>,
        next: usize,
    },
    MapEntries {
        source: GcRef,
        keys: Vec<ValueKey>,
        next: usize,
    },
    StringChars {
        source: GcRef,
        byte_next: usize,
    },
    StringBytes {
        source: GcRef,
        next: usize,
    },
    Bytes {
        source: GcRef,
        next: usize,
        len: usize,
    },
    Range(RangeCursor),
    Map {
        source: Box<IteratorState>,
        callback: Value,
        awaiting: bool,
    },
    Filter {
        source: Box<IteratorState>,
        callback: Value,
        awaiting: Option<Value>,
    },
    Take {
        source: Box<IteratorState>,
        remaining: usize,
        active: bool,
    },
    Skip {
        source: Box<IteratorState>,
        remaining: usize,
    },
}

pub(crate) enum IteratorPollStep {
    Complete(Option<Value>),
    Call { callback: Value, args: Vec<Value> },
}

impl IteratorState {
    pub(crate) fn remap_heap_refs(
        &mut self,
        references: &std::collections::BTreeMap<GcRef, GcRef>,
    ) -> VmResult<()> {
        fn remap_value(
            value: &mut Value,
            references: &std::collections::BTreeMap<GcRef, GcRef>,
        ) -> VmResult<()> {
            if let Value::HeapRef(reference) = value {
                *reference = references.get(reference).copied().ok_or_else(|| {
                    VmError::new(VmErrorKind::TypeMismatch {
                        operation: "iterator heap graph copy",
                    })
                })?;
            }
            Ok(())
        }

        fn remap_source(
            source: &mut GcRef,
            references: &std::collections::BTreeMap<GcRef, GcRef>,
        ) -> VmResult<()> {
            *source = references.get(source).copied().ok_or_else(|| {
                VmError::new(VmErrorKind::TypeMismatch {
                    operation: "iterator source graph copy",
                })
            })?;
            Ok(())
        }

        match &mut self.cursor {
            IteratorCursor::Values { values, .. } => {
                for value in values {
                    remap_value(value, references)?;
                }
            }
            IteratorCursor::Array { source, .. }
            | IteratorCursor::Set { source, .. }
            | IteratorCursor::StringChars { source, .. }
            | IteratorCursor::StringBytes { source, .. }
            | IteratorCursor::Bytes { source, .. } => remap_source(source, references)?,
            IteratorCursor::MapValues { source, keys, .. }
            | IteratorCursor::MapKeys { source, keys, .. }
            | IteratorCursor::MapEntries { source, keys, .. } => {
                remap_source(source, references)?;
                for key in keys {
                    key.remap_heap_refs(references)?;
                }
            }
            IteratorCursor::Range(_) => {}
            IteratorCursor::Map {
                source, callback, ..
            }
            | IteratorCursor::Filter {
                source, callback, ..
            } => {
                source.remap_heap_refs(references)?;
                remap_value(callback, references)?;
            }
            IteratorCursor::Take { source, .. } | IteratorCursor::Skip { source, .. } => {
                source.remap_heap_refs(references)?;
            }
        }
        Ok(())
    }

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
            item_guards: Vec::new(),
        }
    }

    pub(crate) fn from_array_source(source: GcRef, len: usize) -> Self {
        Self {
            cursor: IteratorCursor::Array {
                source,
                next: 0,
                len,
            },
            item_guards: Vec::new(),
        }
    }

    pub(crate) fn from_set_source(source: GcRef, len: usize) -> Self {
        Self {
            cursor: IteratorCursor::Set {
                source,
                next: 0,
                len,
            },
            item_guards: Vec::new(),
        }
    }

    pub(crate) fn from_map_values_source(source: GcRef, keys: Vec<ValueKey>) -> Self {
        Self {
            cursor: IteratorCursor::MapValues {
                source,
                keys,
                next: 0,
            },
            item_guards: Vec::new(),
        }
    }

    pub(crate) fn from_map_keys_source(source: GcRef, keys: Vec<ValueKey>) -> Self {
        Self {
            cursor: IteratorCursor::MapKeys {
                source,
                keys,
                next: 0,
            },
            item_guards: Vec::new(),
        }
    }

    pub(crate) fn from_map_entries_source(source: GcRef, keys: Vec<ValueKey>) -> Self {
        Self {
            cursor: IteratorCursor::MapEntries {
                source,
                keys,
                next: 0,
            },
            item_guards: Vec::new(),
        }
    }

    pub(crate) fn from_string_chars_source(source: GcRef) -> Self {
        Self {
            cursor: IteratorCursor::StringChars {
                source,
                byte_next: 0,
            },
            item_guards: Vec::new(),
        }
    }

    pub(crate) fn from_string_bytes_source(source: GcRef) -> Self {
        Self {
            cursor: IteratorCursor::StringBytes { source, next: 0 },
            item_guards: Vec::new(),
        }
    }

    pub(crate) fn from_bytes_source(source: GcRef, len: usize) -> Self {
        Self {
            cursor: IteratorCursor::Bytes {
                source,
                next: 0,
                len,
            },
            item_guards: Vec::new(),
        }
    }

    pub(crate) fn map(source: Self, callback: Value) -> Self {
        Self {
            cursor: IteratorCursor::Map {
                source: Box::new(source),
                callback,
                awaiting: false,
            },
            item_guards: Vec::new(),
        }
    }

    pub(crate) fn filter(source: Self, callback: Value) -> Self {
        Self {
            cursor: IteratorCursor::Filter {
                source: Box::new(source),
                callback,
                awaiting: None,
            },
            item_guards: Vec::new(),
        }
    }

    pub(crate) fn take(source: Self, remaining: usize) -> Self {
        Self {
            cursor: IteratorCursor::Take {
                source: Box::new(source),
                remaining,
                active: false,
            },
            item_guards: Vec::new(),
        }
    }

    pub(crate) fn skip(source: Self, remaining: usize) -> Self {
        Self {
            cursor: IteratorCursor::Skip {
                source: Box::new(source),
                remaining,
            },
            item_guards: Vec::new(),
        }
    }

    fn values_at(values: Vec<Value>, next: usize) -> Self {
        Self {
            cursor: IteratorCursor::Values { values, next },
            item_guards: Vec::new(),
        }
    }

    pub(crate) fn add_item_guard(&mut self, guard: IteratorItemGuard) {
        if !self.item_guards.contains(&guard) {
            self.item_guards.push(guard);
        }
    }

    pub(crate) fn next(&mut self) -> VmResult<Option<Value>> {
        if !self.item_guards.is_empty() {
            return Err(VmError::new(VmErrorKind::TypeMismatch {
                operation: "guarded iterator",
            }));
        }
        self.next_without_runtime()
    }

    fn next_without_runtime(&mut self) -> VmResult<Option<Value>> {
        match &mut self.cursor {
            IteratorCursor::Values { values, next } => {
                let Some(value) = values.get(*next).copied() else {
                    return Ok(None);
                };
                *next = next.saturating_add(1);
                Ok(Some(value))
            }
            IteratorCursor::Range(cursor) => Ok(cursor.next().map(Value::i64)),
            IteratorCursor::Take {
                source, remaining, ..
            } => {
                if *remaining == 0 {
                    return Ok(None);
                }
                *remaining = remaining.saturating_sub(1);
                source.next()
            }
            IteratorCursor::Skip { source, remaining } => {
                while *remaining > 0 {
                    if source.next()?.is_none() {
                        return Ok(None);
                    }
                    *remaining = remaining.saturating_sub(1);
                }
                source.next()
            }
            IteratorCursor::Array { .. }
            | IteratorCursor::Set { .. }
            | IteratorCursor::MapValues { .. }
            | IteratorCursor::MapKeys { .. }
            | IteratorCursor::MapEntries { .. }
            | IteratorCursor::StringChars { .. }
            | IteratorCursor::StringBytes { .. }
            | IteratorCursor::Bytes { .. }
            | IteratorCursor::Map { .. }
            | IteratorCursor::Filter { .. } => Ok(None),
        }
    }

    pub(crate) fn next_with_runtime(
        &mut self,
        runtime: &mut MethodRuntime<'_, '_>,
        operation: &'static str,
        protected_values: &[Value],
    ) -> VmResult<Option<Value>> {
        self.next_with_runtime_inner(runtime, operation, protected_values, true)
    }

    fn next_with_runtime_inner(
        &mut self,
        runtime: &mut MethodRuntime<'_, '_>,
        operation: &'static str,
        protected_values: &[Value],
        charge_step: bool,
    ) -> VmResult<Option<Value>> {
        if charge_step && let Some(budget) = runtime.budget.as_deref_mut() {
            budget.charge_execution_units(1)?;
        }
        let next = match &mut self.cursor {
            IteratorCursor::Values { .. } | IteratorCursor::Range(_) => self.next_without_runtime(),
            IteratorCursor::Array { source, next, len } => next_indexed_heap_value(
                *source,
                next,
                *len,
                runtime,
                operation,
                HeapSequenceKind::Array,
            ),
            IteratorCursor::Set { source, next, len } => next_indexed_heap_value(
                *source,
                next,
                *len,
                runtime,
                operation,
                HeapSequenceKind::Set,
            ),
            IteratorCursor::MapValues { source, keys, next } => {
                next_map_value(*source, keys, next, runtime, operation)
            }
            IteratorCursor::MapKeys { source, keys, next } => {
                next_map_key(*source, keys, next, runtime, operation)
            }
            IteratorCursor::MapEntries { source, keys, next } => {
                next_map_entry(*source, keys, next, runtime, operation)
            }
            IteratorCursor::StringChars { source, byte_next } => {
                next_string_char(*source, byte_next, runtime, operation)
            }
            IteratorCursor::StringBytes { source, next } => {
                next_string_byte(*source, next, runtime, operation)
            }
            IteratorCursor::Bytes { source, next, len } => next_indexed_heap_value(
                *source,
                next,
                *len,
                runtime,
                operation,
                HeapSequenceKind::Bytes,
            ),
            IteratorCursor::Take {
                source, remaining, ..
            } => {
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
            IteratorCursor::Map {
                source,
                callback,
                awaiting,
            } => {
                debug_assert!(!*awaiting);
                let Some(value) = source.next_with_runtime(runtime, operation, protected_values)?
                else {
                    return self.validate_item(None, runtime);
                };
                let protected = callback_protected_values(source, *callback, protected_values);
                call_callback(runtime, operation, callback, &[value], &protected).map(Some)
            }
            IteratorCursor::Filter {
                source,
                callback,
                awaiting,
            } => loop {
                debug_assert!(awaiting.is_none());
                let Some(value) = source.next_with_runtime(runtime, operation, protected_values)?
                else {
                    return self.validate_item(None, runtime);
                };
                let protected = callback_protected_values(source, *callback, protected_values);
                let predicate = call_callback(runtime, operation, callback, &[value], &protected)?;
                if is_truthy(&predicate) {
                    break Ok(Some(value));
                }
            },
        }?;
        self.validate_item(next, runtime)
    }

    pub(crate) fn poll_next(
        &mut self,
        program: &LinkedProgram,
        heap: &mut Option<&mut HeapExecution<'_>>,
        budget: &mut Option<&mut ExecutionBudget>,
        operation: &'static str,
        charge_step: bool,
        returned: Option<Value>,
    ) -> VmResult<IteratorPollStep> {
        let mut returned = returned;
        let step =
            self.poll_next_inner(program, heap, budget, operation, charge_step, &mut returned)?;
        if returned.is_some() {
            return Err(VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
                opcode: "iterator callback result escaped its continuation",
            }));
        }
        Ok(step)
    }

    #[allow(clippy::too_many_arguments)]
    fn poll_next_inner(
        &mut self,
        program: &LinkedProgram,
        heap: &mut Option<&mut HeapExecution<'_>>,
        budget: &mut Option<&mut ExecutionBudget>,
        operation: &'static str,
        charge_step: bool,
        returned: &mut Option<Value>,
    ) -> VmResult<IteratorPollStep> {
        if charge_step && let Some(budget) = budget.as_deref_mut() {
            budget.charge_execution_units(1)?;
        }
        let step = match &mut self.cursor {
            IteratorCursor::Values { .. } | IteratorCursor::Range(_) => {
                IteratorPollStep::Complete(self.next_without_runtime()?)
            }
            IteratorCursor::Array { source, next, len } => {
                IteratorPollStep::Complete(next_indexed_heap_value_direct(
                    *source,
                    next,
                    *len,
                    heap.as_deref(),
                    operation,
                    HeapSequenceKind::Array,
                )?)
            }
            IteratorCursor::Set { source, next, len } => {
                IteratorPollStep::Complete(next_indexed_heap_value_direct(
                    *source,
                    next,
                    *len,
                    heap.as_deref(),
                    operation,
                    HeapSequenceKind::Set,
                )?)
            }
            IteratorCursor::MapValues { source, keys, next } => IteratorPollStep::Complete(
                next_map_value_direct(*source, keys, next, heap.as_deref(), operation)?,
            ),
            IteratorCursor::MapKeys { source, keys, next } => IteratorPollStep::Complete(
                next_map_key_direct(*source, keys, next, heap.as_deref(), operation)?,
            ),
            IteratorCursor::MapEntries { source, keys, next } => IteratorPollStep::Complete(
                next_map_entry_direct(*source, keys, next, heap, budget, operation)?,
            ),
            IteratorCursor::StringChars { source, byte_next } => IteratorPollStep::Complete(
                next_string_char_direct(*source, byte_next, heap.as_deref(), operation)?,
            ),
            IteratorCursor::StringBytes { source, next } => IteratorPollStep::Complete(
                next_string_byte_direct(*source, next, heap.as_deref(), operation)?,
            ),
            IteratorCursor::Bytes { source, next, len } => {
                IteratorPollStep::Complete(next_indexed_heap_value_direct(
                    *source,
                    next,
                    *len,
                    heap.as_deref(),
                    operation,
                    HeapSequenceKind::Bytes,
                )?)
            }
            IteratorCursor::Take {
                source,
                remaining,
                active,
            } => {
                if !*active && *remaining == 0 {
                    IteratorPollStep::Complete(None)
                } else {
                    if !*active {
                        *remaining = remaining.saturating_sub(1);
                        *active = true;
                    }
                    let step =
                        source.poll_next_inner(program, heap, budget, operation, true, returned)?;
                    if matches!(step, IteratorPollStep::Complete(_)) {
                        *active = false;
                    }
                    step
                }
            }
            IteratorCursor::Skip { source, remaining } => loop {
                if *remaining == 0 {
                    break source
                        .poll_next_inner(program, heap, budget, operation, true, returned)?;
                }
                match source.poll_next_inner(program, heap, budget, operation, true, returned)? {
                    IteratorPollStep::Complete(Some(_)) => {
                        *remaining = remaining.saturating_sub(1);
                    }
                    IteratorPollStep::Complete(None) => {
                        break IteratorPollStep::Complete(None);
                    }
                    call @ IteratorPollStep::Call { .. } => break call,
                }
            },
            IteratorCursor::Map {
                source,
                callback,
                awaiting,
            } => {
                if *awaiting {
                    *awaiting = false;
                    IteratorPollStep::Complete(Some(returned.take().ok_or_else(|| {
                        VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
                            opcode: "missing iterator map callback result",
                        })
                    })?))
                } else {
                    match source
                        .poll_next_inner(program, heap, budget, operation, true, returned)?
                    {
                        IteratorPollStep::Complete(Some(value)) => {
                            *awaiting = true;
                            IteratorPollStep::Call {
                                callback: *callback,
                                args: vec![value],
                            }
                        }
                        other => other,
                    }
                }
            }
            IteratorCursor::Filter {
                source,
                callback,
                awaiting,
            } => {
                let accepted = if let Some(value) = awaiting.take() {
                    let predicate = returned.take().ok_or_else(|| {
                        VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
                            opcode: "missing iterator filter callback result",
                        })
                    })?;
                    is_truthy(&predicate).then_some(value)
                } else {
                    None
                };
                if let Some(value) = accepted {
                    IteratorPollStep::Complete(Some(value))
                } else {
                    match source
                        .poll_next_inner(program, heap, budget, operation, true, returned)?
                    {
                        IteratorPollStep::Complete(Some(value)) => {
                            *awaiting = Some(value);
                            IteratorPollStep::Call {
                                callback: *callback,
                                args: vec![value],
                            }
                        }
                        other => other,
                    }
                }
            }
        };
        match step {
            IteratorPollStep::Complete(value) => self
                .validate_polled_item(value, program, heap, budget)
                .map(IteratorPollStep::Complete),
            call => Ok(call),
        }
    }

    fn validate_polled_item(
        &self,
        value: Option<Value>,
        program: &LinkedProgram,
        heap: &mut Option<&mut HeapExecution<'_>>,
        budget: &mut Option<&mut ExecutionBudget>,
    ) -> VmResult<Option<Value>> {
        let Some(value) = value else {
            return Ok(None);
        };
        for guard in &self.item_guards {
            let mut runtime = MethodRuntime {
                program,
                heap: heap.as_deref_mut(),
                budget: budget.as_deref_mut(),
            };
            crate::runtime_type_guards::execute_iterator_item_guard(&value, guard, &mut runtime)?;
        }
        Ok(Some(value))
    }

    pub(crate) fn trace_heap_refs(&self, refs: &mut Vec<GcRef>) {
        match &self.cursor {
            IteratorCursor::Values { values, .. } => {
                values.iter().for_each(|value| value.trace_heap_refs(refs))
            }
            IteratorCursor::Array { source, .. }
            | IteratorCursor::Set { source, .. }
            | IteratorCursor::MapValues { source, .. }
            | IteratorCursor::MapKeys { source, .. }
            | IteratorCursor::MapEntries { source, .. }
            | IteratorCursor::StringChars { source, .. }
            | IteratorCursor::StringBytes { source, .. }
            | IteratorCursor::Bytes { source, .. } => refs.push(*source),
            IteratorCursor::Range(_) => {}
            IteratorCursor::Map {
                source, callback, ..
            }
            | IteratorCursor::Filter {
                source, callback, ..
            } => {
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
            IteratorCursor::Array { source, .. }
            | IteratorCursor::Set { source, .. }
            | IteratorCursor::MapValues { source, .. }
            | IteratorCursor::MapKeys { source, .. }
            | IteratorCursor::MapEntries { source, .. }
            | IteratorCursor::StringChars { source, .. }
            | IteratorCursor::StringBytes { source, .. }
            | IteratorCursor::Bytes { source, .. } => protected.push(Value::HeapRef(*source)),
            IteratorCursor::Range(_) => {}
            IteratorCursor::Map {
                source, callback, ..
            }
            | IteratorCursor::Filter {
                source, callback, ..
            } => {
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
            | IteratorCursor::Array { .. }
            | IteratorCursor::Set { .. }
            | IteratorCursor::MapValues { .. }
            | IteratorCursor::MapKeys { .. }
            | IteratorCursor::MapEntries { .. }
            | IteratorCursor::StringChars { .. }
            | IteratorCursor::StringBytes { .. }
            | IteratorCursor::Bytes { .. }
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
            | IteratorCursor::Array { .. }
            | IteratorCursor::Set { .. }
            | IteratorCursor::MapValues { .. }
            | IteratorCursor::MapKeys { .. }
            | IteratorCursor::MapEntries { .. }
            | IteratorCursor::StringChars { .. }
            | IteratorCursor::StringBytes { .. }
            | IteratorCursor::Bytes { .. }
            | IteratorCursor::Map { .. }
            | IteratorCursor::Filter { .. }
            | IteratorCursor::Take { .. }
            | IteratorCursor::Skip { .. } => 0,
        }
    }

    fn validate_item(
        &self,
        value: Option<Value>,
        runtime: &mut MethodRuntime<'_, '_>,
    ) -> VmResult<Option<Value>> {
        let Some(value) = value else {
            return Ok(None);
        };
        for guard in &self.item_guards {
            crate::runtime_type_guards::execute_iterator_item_guard(&value, guard, runtime)?;
        }
        Ok(Some(value))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HeapSequenceKind {
    Array,
    Set,
    Bytes,
}

fn next_indexed_heap_value(
    source: GcRef,
    next: &mut usize,
    len: usize,
    runtime: &MethodRuntime<'_, '_>,
    operation: &'static str,
    kind: HeapSequenceKind,
) -> VmResult<Option<Value>> {
    if *next >= len {
        return Ok(None);
    }
    let index = *next;
    *next = next.saturating_add(1);
    let Some(heap) = runtime.heap.as_deref() else {
        return type_error(operation);
    };
    let Some(value) = heap.heap.get(source) else {
        return type_error(operation);
    };
    match (kind, value) {
        (HeapSequenceKind::Array, HeapValue::Array(values)) => {
            Ok(values.get(index).map(stored_runtime_value))
        }
        (HeapSequenceKind::Set, HeapValue::Set(values)) => Ok(values
            .values()
            .nth(index)
            .copied()
            .map(|value| stored_runtime_value(&value))),
        (HeapSequenceKind::Bytes, HeapValue::Bytes(values)) => {
            Ok(values.get(index).map(|byte| Value::U8(*byte)))
        }
        _ => type_error(operation),
    }
}

fn next_map_value(
    source: GcRef,
    keys: &[ValueKey],
    next: &mut usize,
    runtime: &MethodRuntime<'_, '_>,
    operation: &'static str,
) -> VmResult<Option<Value>> {
    let Some(heap) = runtime.heap.as_deref() else {
        return type_error(operation);
    };
    let Some(HeapValue::Map(values)) = heap.heap.get(source) else {
        return type_error(operation);
    };
    while let Some(key) = keys.get(*next) {
        *next = next.saturating_add(1);
        if let Some(value) = values.get_keyed(key) {
            return Ok(Some(value));
        }
    }
    Ok(None)
}

fn next_map_key(
    source: GcRef,
    keys: &[ValueKey],
    next: &mut usize,
    runtime: &mut MethodRuntime<'_, '_>,
    operation: &'static str,
) -> VmResult<Option<Value>> {
    let Some(heap) = runtime.heap.as_deref() else {
        return type_error(operation);
    };
    let Some(HeapValue::Map(values)) = heap.heap.get(source) else {
        return type_error(operation);
    };
    while let Some(key) = keys.get(*next) {
        *next = next.saturating_add(1);
        if let Some(entry) = values.entry_for_key(key) {
            return Ok(Some(stored_runtime_value(&entry.key)));
        }
    }
    Ok(None)
}

fn next_map_entry(
    source: GcRef,
    keys: &[ValueKey],
    next: &mut usize,
    runtime: &mut MethodRuntime<'_, '_>,
    operation: &'static str,
) -> VmResult<Option<Value>> {
    let Some(heap) = runtime.heap.as_deref() else {
        return type_error(operation);
    };
    let Some(HeapValue::Map(values)) = heap.heap.get(source) else {
        return type_error(operation);
    };
    while let Some(key) = keys.get(*next) {
        *next = next.saturating_add(1);
        if let Some(entry) = values.entry_for_key(key) {
            let key = stored_runtime_value(&entry.key);
            let value = stored_runtime_value(&entry.value);
            let mut heap = runtime.heap.as_deref_mut();
            return crate::map_methods::map_entry(key, value, &mut heap, &mut runtime.budget)
                .map(Some);
        }
    }
    Ok(None)
}

fn next_string_char(
    source: GcRef,
    byte_next: &mut usize,
    runtime: &MethodRuntime<'_, '_>,
    operation: &'static str,
) -> VmResult<Option<Value>> {
    let Some(heap) = runtime.heap.as_deref() else {
        return type_error(operation);
    };
    let Some(HeapValue::String(value)) = heap.heap.get(source) else {
        return type_error(operation);
    };
    let Some(rest) = value.get(*byte_next..) else {
        return type_error(operation);
    };
    let Some(ch) = rest.chars().next() else {
        return Ok(None);
    };
    *byte_next = byte_next.saturating_add(ch.len_utf8());
    Ok(Some(Value::Char(ch)))
}

fn next_string_byte(
    source: GcRef,
    next: &mut usize,
    runtime: &MethodRuntime<'_, '_>,
    operation: &'static str,
) -> VmResult<Option<Value>> {
    let Some(heap) = runtime.heap.as_deref() else {
        return type_error(operation);
    };
    let Some(HeapValue::String(value)) = heap.heap.get(source) else {
        return type_error(operation);
    };
    let Some(byte) = value.as_bytes().get(*next) else {
        return Ok(None);
    };
    *next = next.saturating_add(1);
    Ok(Some(Value::U8(*byte)))
}

fn next_indexed_heap_value_direct(
    source: GcRef,
    next: &mut usize,
    len: usize,
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
    kind: HeapSequenceKind,
) -> VmResult<Option<Value>> {
    if *next >= len {
        return Ok(None);
    }
    let index = *next;
    *next = next.saturating_add(1);
    let Some(value) = heap.and_then(|heap| heap.heap.get(source)) else {
        return type_error(operation);
    };
    match (kind, value) {
        (HeapSequenceKind::Array, HeapValue::Array(values)) => {
            Ok(values.get(index).map(stored_runtime_value))
        }
        (HeapSequenceKind::Set, HeapValue::Set(values)) => Ok(values
            .values()
            .nth(index)
            .copied()
            .map(|value| stored_runtime_value(&value))),
        (HeapSequenceKind::Bytes, HeapValue::Bytes(values)) => {
            Ok(values.get(index).map(|byte| Value::U8(*byte)))
        }
        _ => type_error(operation),
    }
}

fn next_map_value_direct(
    source: GcRef,
    keys: &[ValueKey],
    next: &mut usize,
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<Option<Value>> {
    let Some(HeapValue::Map(values)) = heap.and_then(|heap| heap.heap.get(source)) else {
        return type_error(operation);
    };
    while let Some(key) = keys.get(*next) {
        *next = next.saturating_add(1);
        if let Some(value) = values.get_keyed(key) {
            return Ok(Some(value));
        }
    }
    Ok(None)
}

fn next_map_key_direct(
    source: GcRef,
    keys: &[ValueKey],
    next: &mut usize,
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<Option<Value>> {
    let Some(HeapValue::Map(values)) = heap.and_then(|heap| heap.heap.get(source)) else {
        return type_error(operation);
    };
    while let Some(key) = keys.get(*next) {
        *next = next.saturating_add(1);
        if let Some(entry) = values.entry_for_key(key) {
            return Ok(Some(stored_runtime_value(&entry.key)));
        }
    }
    Ok(None)
}

fn next_map_entry_direct(
    source: GcRef,
    keys: &[ValueKey],
    next: &mut usize,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<Option<Value>> {
    let entry = {
        let Some(HeapValue::Map(values)) = heap.as_deref().and_then(|heap| heap.heap.get(source))
        else {
            return type_error(operation);
        };
        let mut found = None;
        while let Some(key) = keys.get(*next) {
            *next = next.saturating_add(1);
            if let Some(entry) = values.entry_for_key(key) {
                found = Some((
                    stored_runtime_value(&entry.key),
                    stored_runtime_value(&entry.value),
                ));
                break;
            }
        }
        found
    };
    let Some((key, value)) = entry else {
        return Ok(None);
    };
    crate::map_methods::map_entry(key, value, heap, budget).map(Some)
}

fn next_string_char_direct(
    source: GcRef,
    byte_next: &mut usize,
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<Option<Value>> {
    let Some(HeapValue::String(value)) = heap.and_then(|heap| heap.heap.get(source)) else {
        return type_error(operation);
    };
    let Some(rest) = value.get(*byte_next..) else {
        return type_error(operation);
    };
    let Some(ch) = rest.chars().next() else {
        return Ok(None);
    };
    *byte_next = byte_next.saturating_add(ch.len_utf8());
    Ok(Some(Value::Char(ch)))
}

fn next_string_byte_direct(
    source: GcRef,
    next: &mut usize,
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<Option<Value>> {
    let Some(HeapValue::String(value)) = heap.and_then(|heap| heap.heap.get(source)) else {
        return type_error(operation);
    };
    let Some(byte) = value.as_bytes().get(*next) else {
        return Ok(None);
    };
    *next = next.saturating_add(1);
    Ok(Some(Value::U8(*byte)))
}

fn type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
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
