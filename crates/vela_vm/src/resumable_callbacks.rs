use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::sync::Arc;

use vela_bytecode::{LinkedArtifact, ScriptFunctionHandle};

use crate::collection_mutation::check_collection_len;
use crate::equality::{ResumableComparison, ResumableComparisonStep};
use crate::heap::HeapValue;
use crate::heap_execution::HeapExecution;
use crate::heap_values::allocate_heap_value;
use crate::option_result::{StdEnumKind, StdEnumVariant, option_value, result_value, std_enum_tag};
use crate::runtime_checks::{expect_closure_ref, is_truthy};
use crate::script_set::ScriptSet;
use crate::value_key::ValueKey;
use crate::{
    CallbackMethodInlineCacheEntry, CallbackMethodInlineCacheTarget, ExecutionBudget,
    StandardMethodReceiver, Value, VmError, VmErrorKind, VmResult, array_methods, map_methods,
    set_methods,
};

pub(crate) struct ResumableCallbackMethod {
    callback_value: Value,
    callback: Option<PreparedCallback>,
    state: CallbackState,
}

pub(crate) enum ResumableCallbackStep {
    Complete(Value),
    Call {
        owner: Arc<LinkedArtifact>,
        function: ScriptFunctionHandle,
        captures: Vec<Value>,
        args: Vec<Value>,
    },
}

struct PreparedCallback {
    owner: Arc<LinkedArtifact>,
    function: ScriptFunctionHandle,
    captures: Vec<Value>,
    param_len: usize,
}

enum CallbackState {
    Iterator(Box<crate::iteration::ResumableIteratorMethod>),
    Sequence {
        receiver: StandardMethodReceiver,
        target: CallbackMethodInlineCacheTarget,
        operation: &'static str,
        values: Vec<Value>,
        index: usize,
        output: Vec<Value>,
        count: i64,
        total: NumericTotal,
        found: Option<Value>,
        decision: Option<bool>,
        awaiting: Option<Value>,
    },
    Map {
        target: CallbackMethodInlineCacheTarget,
        operation: &'static str,
        entries: Vec<(Value, Value)>,
        index: usize,
        output: Vec<(Value, Value)>,
        count: i64,
        found: Option<(Value, Value)>,
        decision: Option<bool>,
        awaiting: Option<(Value, Value)>,
    },
    GroupBy {
        operation: &'static str,
        values: Vec<Value>,
        index: usize,
        groups: BTreeMap<ValueKey, GroupValues>,
        awaiting: Option<Value>,
    },
    SortBy(SortByState),
    Enum {
        receiver_kind: StandardMethodReceiver,
        target: CallbackMethodInlineCacheTarget,
        operation: &'static str,
        receiver: Value,
        variant: StdEnumVariant,
        payload: Option<Value>,
        active: bool,
        awaiting: bool,
    },
    Complete,
}

struct GroupValues {
    key: Value,
    values: Vec<Value>,
}

struct SortByState {
    operation: &'static str,
    values: Vec<Value>,
    index: usize,
    entries: Vec<SortByEntry>,
    awaiting_callback: Option<Value>,
    collecting: bool,
    sort_index: usize,
    current: usize,
    comparison: Option<ResumableComparison>,
}

struct SortByEntry {
    key: Value,
    value: Value,
}

enum NumericTotal {
    Int(i64),
    Float(f64),
}

impl NumericTotal {
    fn add_value(&mut self, value: &Value, operation: &'static str) -> VmResult<()> {
        match (&mut *self, value) {
            (Self::Int(total), Value::I64(value)) => {
                *total = total
                    .checked_add(*value)
                    .ok_or_else(|| VmError::new(VmErrorKind::TypeMismatch { operation }))?;
            }
            (Self::Int(total), Value::F64(value)) => {
                *self = Self::Float(*total as f64 + *value);
            }
            (Self::Float(total), Value::I64(value)) => *total += *value as f64,
            (Self::Float(total), Value::F64(value)) => *total += *value,
            _ => return Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
        }
        Ok(())
    }

    fn into_value(self) -> Value {
        match self {
            Self::Int(value) => Value::I64(value),
            Self::Float(value) => Value::F64(value),
        }
    }
}

impl ResumableCallbackMethod {
    pub(crate) fn new(
        receiver: &Value,
        cache: CallbackMethodInlineCacheEntry,
        args: &[Value],
        heap: Option<&HeapExecution<'_>>,
    ) -> Option<VmResult<Self>> {
        if let Some(iterator) =
            crate::iteration::ResumableIteratorMethod::new(*receiver, cache, args)
        {
            return Some(iterator.map(|iterator| Self {
                callback_value: args.first().copied().unwrap_or(Value::Missing),
                callback: None,
                state: CallbackState::Iterator(Box::new(iterator)),
            }));
        }
        let operation = callback_operation(cache.receiver, cache.target)?;
        if cache.receiver == StandardMethodReceiver::Array
            && cache.target == CallbackMethodInlineCacheTarget::Sum
            && args.is_empty()
        {
            return None;
        }
        if args.len() != 1 {
            return Some(Err(VmError::new(VmErrorKind::ArityMismatch {
                name: operation.trim_start_matches("method ").to_owned(),
                expected: 1,
                actual: args.len(),
            })));
        }
        let state = match (cache.receiver, cache.target) {
            (StandardMethodReceiver::Array, CallbackMethodInlineCacheTarget::SortBy) => {
                CallbackState::SortBy(SortByState {
                    operation,
                    values: match array_methods::array_values(receiver, heap, operation) {
                        Ok(values) => values,
                        Err(error) => return Some(Err(error)),
                    },
                    index: 0,
                    entries: Vec::new(),
                    awaiting_callback: None,
                    collecting: true,
                    sort_index: 1,
                    current: 1,
                    comparison: None,
                })
            }
            (StandardMethodReceiver::Array, CallbackMethodInlineCacheTarget::GroupBy) => {
                CallbackState::GroupBy {
                    operation,
                    values: match array_methods::array_values(receiver, heap, operation) {
                        Ok(values) => values,
                        Err(error) => return Some(Err(error)),
                    },
                    index: 0,
                    groups: BTreeMap::new(),
                    awaiting: None,
                }
            }
            (StandardMethodReceiver::Array, _) => CallbackState::Sequence {
                receiver: cache.receiver,
                target: cache.target,
                operation,
                values: match array_methods::array_values(receiver, heap, operation) {
                    Ok(values) => values,
                    Err(error) => return Some(Err(error)),
                },
                index: 0,
                output: Vec::new(),
                count: 0,
                total: NumericTotal::Int(0),
                found: None,
                decision: None,
                awaiting: None,
            },
            (StandardMethodReceiver::Set, _) => CallbackState::Sequence {
                receiver: cache.receiver,
                target: cache.target,
                operation,
                values: match set_methods::set_values(receiver, heap, operation) {
                    Ok(values) => values,
                    Err(error) => return Some(Err(error)),
                },
                index: 0,
                output: Vec::new(),
                count: 0,
                total: NumericTotal::Int(0),
                found: None,
                decision: None,
                awaiting: None,
            },
            (StandardMethodReceiver::Map, _) => CallbackState::Map {
                target: cache.target,
                operation,
                entries: match map_methods::map_entries(receiver, heap, operation) {
                    Ok(entries) => entries,
                    Err(error) => return Some(Err(error)),
                },
                index: 0,
                output: Vec::new(),
                count: 0,
                found: None,
                decision: None,
                awaiting: None,
            },
            (StandardMethodReceiver::Option | StandardMethodReceiver::Result, _) => {
                let (variant, payload) = match enum_value(receiver, heap, operation) {
                    Ok(value) => value,
                    Err(error) => return Some(Err(error)),
                };
                let active = enum_callback_is_active(cache.receiver, cache.target, variant);
                CallbackState::Enum {
                    receiver_kind: cache.receiver,
                    target: cache.target,
                    operation,
                    receiver: *receiver,
                    variant,
                    payload,
                    active,
                    awaiting: false,
                }
            }
            _ => return None,
        };
        Some(Ok(Self {
            callback_value: args[0],
            callback: None,
            state,
        }))
    }

    pub(crate) fn step(
        &mut self,
        vm: &crate::Vm,
        program_owner: &Arc<LinkedArtifact>,
        heap: &mut Option<&mut HeapExecution<'_>>,
        budget: &mut Option<&mut ExecutionBudget>,
        returned: Option<Value>,
    ) -> VmResult<ResumableCallbackStep> {
        if matches!(self.state, CallbackState::Iterator(_)) {
            return self.step_iterator(program_owner, heap, budget, returned);
        }
        if matches!(self.state, CallbackState::Enum { .. }) {
            return self.step_enum(heap, budget, returned);
        }
        if matches!(self.state, CallbackState::SortBy(_)) {
            return self.step_sort_by(vm, program_owner, heap, budget, returned);
        }
        match &mut self.state {
            CallbackState::Sequence {
                target,
                output,
                count,
                total,
                found,
                decision,
                awaiting,
                ..
            } => {
                if let Some(returned) = returned {
                    let value = awaiting.take().ok_or_else(incomplete_callback)?;
                    match target {
                        CallbackMethodInlineCacheTarget::Map => output.push(returned),
                        CallbackMethodInlineCacheTarget::Filter => {
                            if is_truthy(&returned) {
                                output.push(value);
                            }
                        }
                        CallbackMethodInlineCacheTarget::Find => {
                            if is_truthy(&returned) {
                                *found = Some(value);
                            }
                        }
                        CallbackMethodInlineCacheTarget::Any => {
                            if is_truthy(&returned) {
                                *decision = Some(true);
                            }
                        }
                        CallbackMethodInlineCacheTarget::All => {
                            if !is_truthy(&returned) {
                                *decision = Some(false);
                            }
                        }
                        CallbackMethodInlineCacheTarget::Count => {
                            if is_truthy(&returned) {
                                *count = count.saturating_add(1);
                            }
                        }
                        CallbackMethodInlineCacheTarget::Sum => {
                            total.add_value(&returned, "method sum")?;
                        }
                        _ => return Err(incomplete_callback()),
                    }
                } else if awaiting.is_some() {
                    return Err(incomplete_callback());
                }
            }
            CallbackState::Map {
                target,
                output,
                count,
                found,
                decision,
                awaiting,
                ..
            } => {
                if let Some(returned) = returned {
                    let (key, value) = awaiting.take().ok_or_else(incomplete_callback)?;
                    match target {
                        CallbackMethodInlineCacheTarget::MapValues => {
                            output.push((key, returned));
                        }
                        CallbackMethodInlineCacheTarget::Filter => {
                            if is_truthy(&returned) {
                                output.push((key, value));
                            }
                        }
                        CallbackMethodInlineCacheTarget::Find => {
                            if is_truthy(&returned) {
                                *found = Some((key, value));
                            }
                        }
                        CallbackMethodInlineCacheTarget::Any => {
                            if is_truthy(&returned) {
                                *decision = Some(true);
                            }
                        }
                        CallbackMethodInlineCacheTarget::All => {
                            if !is_truthy(&returned) {
                                *decision = Some(false);
                            }
                        }
                        CallbackMethodInlineCacheTarget::Count => {
                            if is_truthy(&returned) {
                                *count = count.saturating_add(1);
                            }
                        }
                        _ => return Err(incomplete_callback()),
                    }
                } else if awaiting.is_some() {
                    return Err(incomplete_callback());
                }
            }
            CallbackState::GroupBy {
                groups, awaiting, ..
            } => {
                if let Some(returned) = returned {
                    let value = awaiting.take().ok_or_else(incomplete_callback)?;
                    let key = ValueKey::from_value(&returned, heap.as_deref(), "method group_by")?;
                    match groups.entry(key) {
                        std::collections::btree_map::Entry::Vacant(entry) => {
                            entry.insert(GroupValues {
                                key: returned,
                                values: vec![value],
                            });
                        }
                        std::collections::btree_map::Entry::Occupied(mut entry) => {
                            entry.get_mut().values.push(value);
                        }
                    }
                } else if awaiting.is_some() {
                    return Err(incomplete_callback());
                }
            }
            CallbackState::Iterator(_)
            | CallbackState::SortBy(_)
            | CallbackState::Enum { .. }
            | CallbackState::Complete => {
                return Err(incomplete_callback());
            }
        }

        let has_next = match &self.state {
            CallbackState::Sequence { values, index, .. } => *index < values.len(),
            CallbackState::Map { entries, index, .. } => *index < entries.len(),
            CallbackState::GroupBy { values, index, .. } => *index < values.len(),
            CallbackState::Iterator(_) | CallbackState::SortBy(_) => false,
            CallbackState::Enum { .. } => false,
            CallbackState::Complete => false,
        };
        let callback_param_len = if has_next {
            Some(self.prepare_callback(heap.as_deref())?.param_len)
        } else {
            None
        };
        let next = match &mut self.state {
            CallbackState::Sequence {
                target,
                values,
                index,
                found,
                decision,
                awaiting,
                ..
            } => {
                let terminal = matches!(target, CallbackMethodInlineCacheTarget::Find)
                    && found.is_some()
                    || matches!(target, CallbackMethodInlineCacheTarget::Any)
                        && *decision == Some(true)
                    || matches!(target, CallbackMethodInlineCacheTarget::All)
                        && *decision == Some(false);
                if terminal || *index >= values.len() {
                    return self.finish(heap, budget);
                }
                let value = values[*index];
                *index = index.saturating_add(1);
                *awaiting = Some(value);
                vec![value]
            }
            CallbackState::Map {
                target,
                entries,
                index,
                found,
                decision,
                awaiting,
                ..
            } => {
                let terminal = matches!(target, CallbackMethodInlineCacheTarget::Find)
                    && found.is_some()
                    || matches!(target, CallbackMethodInlineCacheTarget::Any)
                        && *decision == Some(true)
                    || matches!(target, CallbackMethodInlineCacheTarget::All)
                        && *decision == Some(false);
                if terminal || *index >= entries.len() {
                    return self.finish(heap, budget);
                }
                let entry = entries[*index];
                *index = index.saturating_add(1);
                *awaiting = Some(entry);
                match callback_param_len.expect("a pending map entry prepares its callback") {
                    0 => Vec::new(),
                    1 => vec![entry.1],
                    _ => vec![entry.0, entry.1],
                }
            }
            CallbackState::GroupBy {
                values,
                index,
                awaiting,
                ..
            } => {
                if *index >= values.len() {
                    return self.finish(heap, budget);
                }
                let value = values[*index];
                *index = index.saturating_add(1);
                *awaiting = Some(value);
                vec![value]
            }
            CallbackState::Iterator(_)
            | CallbackState::SortBy(_)
            | CallbackState::Enum { .. }
            | CallbackState::Complete => {
                return Err(incomplete_callback());
            }
        };
        if let Some(budget) = budget.as_deref_mut() {
            // Preserve the legacy iterator/callback accounting as four
            // individually interruptible cooperative units per invocation.
            for _ in 0..4 {
                budget.charge_execution_units(1)?;
            }
        }
        let callback = self.prepare_callback(heap.as_deref())?;
        Ok(ResumableCallbackStep::Call {
            owner: Arc::clone(&callback.owner),
            function: callback.function,
            captures: callback.captures.clone(),
            args: next,
        })
    }

    pub(crate) fn protect_roots(&self, heap: &mut HeapExecution<'_>) {
        heap.protect_values(&[self.callback_value]);
        if let Some(callback) = &self.callback {
            heap.protect_values(&callback.captures);
        }
        match &self.state {
            CallbackState::Iterator(iterator) => iterator.protect_roots(heap),
            CallbackState::Sequence {
                values,
                output,
                found,
                awaiting,
                ..
            } => {
                heap.protect_values(values);
                heap.protect_values(output);
                if let Some(value) = found {
                    heap.protect_values(&[*value]);
                }
                if let Some(value) = awaiting {
                    heap.protect_values(&[*value]);
                }
            }
            CallbackState::Map {
                entries,
                output,
                found,
                awaiting,
                ..
            } => {
                for (key, value) in entries.iter().chain(output) {
                    heap.protect_values(&[*key, *value]);
                }
                if let Some((key, value)) = found {
                    heap.protect_values(&[*key, *value]);
                }
                if let Some((key, value)) = awaiting {
                    heap.protect_values(&[*key, *value]);
                }
            }
            CallbackState::GroupBy {
                values,
                groups,
                awaiting,
                ..
            } => {
                heap.protect_values(values);
                for group in groups.values() {
                    heap.protect_values(&[group.key]);
                    heap.protect_values(&group.values);
                }
                if let Some(value) = awaiting {
                    heap.protect_values(&[*value]);
                }
            }
            CallbackState::SortBy(state) => {
                heap.protect_values(&state.values);
                for entry in &state.entries {
                    heap.protect_values(&[entry.key, entry.value]);
                }
                if let Some(value) = state.awaiting_callback {
                    heap.protect_values(&[value]);
                }
            }
            CallbackState::Enum {
                receiver, payload, ..
            } => {
                heap.protect_values(&[*receiver]);
                if let Some(payload) = payload {
                    heap.protect_values(&[*payload]);
                }
            }
            CallbackState::Complete => {}
        }
    }

    fn prepare_callback(
        &mut self,
        heap: Option<&HeapExecution<'_>>,
    ) -> VmResult<&PreparedCallback> {
        if self.callback.is_none() {
            let closure = expect_closure_ref(&self.callback_value, heap, "callback method")?;
            let code = closure.owner.function(closure.function).ok_or_else(|| {
                VmError::new(VmErrorKind::UnknownFunction {
                    name: format!("<linked closure#{}>", closure.function.index()),
                })
            })?;
            if code.asyncness.is_async() {
                return Err(VmError::new(VmErrorKind::AsyncCallRequiresAwait {
                    name: closure
                        .owner
                        .program()
                        .debug_name(code.debug_name)
                        .to_owned(),
                }));
            }
            self.callback = Some(PreparedCallback {
                owner: Arc::clone(&closure.owner),
                function: closure.function,
                captures: closure.captures.as_slice().to_vec(),
                param_len: code.params.len(),
            });
        }
        Ok(self.callback.as_ref().expect("callback was prepared"))
    }

    fn step_iterator(
        &mut self,
        program_owner: &Arc<LinkedArtifact>,
        heap: &mut Option<&mut HeapExecution<'_>>,
        budget: &mut Option<&mut ExecutionBudget>,
        returned: Option<Value>,
    ) -> VmResult<ResumableCallbackStep> {
        let CallbackState::Iterator(iterator) = &mut self.state else {
            return Err(incomplete_callback());
        };
        match iterator.step(program_owner, heap, budget, returned)? {
            crate::iteration::ResumableIteratorMethodStep::Complete(value) => {
                Ok(ResumableCallbackStep::Complete(value))
            }
            crate::iteration::ResumableIteratorMethodStep::Call {
                owner,
                function,
                captures,
                args,
            } => Ok(ResumableCallbackStep::Call {
                owner,
                function,
                captures,
                args,
            }),
        }
    }

    fn step_enum(
        &mut self,
        heap: &mut Option<&mut HeapExecution<'_>>,
        budget: &mut Option<&mut ExecutionBudget>,
        returned: Option<Value>,
    ) -> VmResult<ResumableCallbackStep> {
        let state = std::mem::replace(&mut self.state, CallbackState::Complete);
        let CallbackState::Enum {
            receiver_kind,
            target,
            operation,
            receiver,
            variant,
            payload,
            active,
            awaiting,
        } = state
        else {
            return Err(incomplete_callback());
        };
        if let Some(returned) = returned {
            if !awaiting {
                return Err(incomplete_callback());
            }
            let value = finish_active_enum_callback(
                receiver_kind,
                target,
                variant,
                payload,
                returned,
                operation,
                heap,
                budget,
            )?;
            return Ok(ResumableCallbackStep::Complete(value));
        }
        if awaiting {
            return Err(incomplete_callback());
        }
        if !active {
            let value = copy_enum_value(variant, payload, operation, heap, budget)?;
            return Ok(ResumableCallbackStep::Complete(value));
        }
        self.state = CallbackState::Enum {
            receiver_kind,
            target,
            operation,
            receiver,
            variant,
            payload,
            active,
            awaiting: true,
        };
        if let Some(budget) = budget.as_deref_mut() {
            budget.charge_execution_units(1)?;
        }
        let callback_args = if receiver_kind == StandardMethodReceiver::Option
            && target == CallbackMethodInlineCacheTarget::OrElse
            && variant == StdEnumVariant::None
        {
            Vec::new()
        } else {
            vec![payload.ok_or_else(incomplete_callback)?]
        };
        let callback = self.prepare_callback(heap.as_deref())?;
        Ok(ResumableCallbackStep::Call {
            owner: Arc::clone(&callback.owner),
            function: callback.function,
            captures: callback.captures.clone(),
            args: callback_args,
        })
    }

    fn step_sort_by(
        &mut self,
        vm: &crate::Vm,
        program_owner: &Arc<LinkedArtifact>,
        heap: &mut Option<&mut HeapExecution<'_>>,
        budget: &mut Option<&mut ExecutionBudget>,
        mut returned: Option<Value>,
    ) -> VmResult<ResumableCallbackStep> {
        let state = std::mem::replace(&mut self.state, CallbackState::Complete);
        let CallbackState::SortBy(mut state) = state else {
            return Err(incomplete_callback());
        };
        if let Some(comparison) = state.comparison.as_mut() {
            match comparison.step(vm, program_owner.program(), heap, budget, returned.take())? {
                ResumableComparisonStep::Call { function, args } => {
                    self.state = CallbackState::SortBy(state);
                    return Ok(ResumableCallbackStep::Call {
                        owner: Arc::clone(program_owner),
                        function,
                        captures: Vec::new(),
                        args,
                    });
                }
                ResumableComparisonStep::CompleteOrdering(ordering) => {
                    state.comparison = None;
                    update_sort_position(&mut state, ordering);
                }
                ResumableComparisonStep::Complete(_) => return Err(incomplete_callback()),
            }
        } else if let Some(value) = state.awaiting_callback.take() {
            let key = returned.take().ok_or_else(incomplete_callback)?;
            state.entries.push(SortByEntry { key, value });
        } else if returned.is_some() {
            return Err(incomplete_callback());
        }

        loop {
            if state.collecting {
                if state.index < state.values.len() {
                    let value = state.values[state.index];
                    state.index = state.index.saturating_add(1);
                    state.awaiting_callback = Some(value);
                    self.state = CallbackState::SortBy(state);
                    if let Some(budget) = budget.as_deref_mut() {
                        for _ in 0..4 {
                            budget.charge_execution_units(1)?;
                        }
                    }
                    let callback = self.prepare_callback(heap.as_deref())?;
                    return Ok(ResumableCallbackStep::Call {
                        owner: Arc::clone(&callback.owner),
                        function: callback.function,
                        captures: callback.captures.clone(),
                        args: vec![value],
                    });
                }
                state.collecting = false;
                state.sort_index = 1;
                state.current = 1;
            }
            if state.sort_index >= state.entries.len() {
                let values = state.entries.into_iter().map(|entry| entry.value).collect();
                return array_methods::make_array_value(values, heap, budget, state.operation)
                    .map(ResumableCallbackStep::Complete);
            }
            if state.current == 0 {
                state.sort_index = state.sort_index.saturating_add(1);
                state.current = state.sort_index;
                continue;
            }
            state.comparison = Some(ResumableComparison::total(
                state.entries[state.current].key,
                state.entries[state.current.saturating_sub(1)].key,
                state.operation,
            ));
            let comparison = state
                .comparison
                .as_mut()
                .expect("sort_by scheduled a comparison");
            match comparison.step(vm, program_owner.program(), heap, budget, None)? {
                ResumableComparisonStep::Call { function, args } => {
                    self.state = CallbackState::SortBy(state);
                    return Ok(ResumableCallbackStep::Call {
                        owner: Arc::clone(program_owner),
                        function,
                        captures: Vec::new(),
                        args,
                    });
                }
                ResumableComparisonStep::CompleteOrdering(ordering) => {
                    state.comparison = None;
                    update_sort_position(&mut state, ordering);
                }
                ResumableComparisonStep::Complete(_) => return Err(incomplete_callback()),
            }
        }
    }

    fn finish(
        &mut self,
        heap: &mut Option<&mut HeapExecution<'_>>,
        budget: &mut Option<&mut ExecutionBudget>,
    ) -> VmResult<ResumableCallbackStep> {
        let state = std::mem::replace(&mut self.state, CallbackState::Complete);
        let value = match state {
            CallbackState::Sequence {
                receiver,
                target,
                operation,
                output,
                count,
                total,
                found,
                decision,
                ..
            } => match target {
                CallbackMethodInlineCacheTarget::Map | CallbackMethodInlineCacheTarget::Filter => {
                    match receiver {
                        StandardMethodReceiver::Array => {
                            array_methods::make_array_value(output, heap, budget, operation)?
                        }
                        StandardMethodReceiver::Set => {
                            make_set_value(output, heap, budget, operation)?
                        }
                        _ => return Err(incomplete_callback()),
                    }
                }
                CallbackMethodInlineCacheTarget::Find => {
                    make_option_value(found, heap, budget, operation)?
                }
                CallbackMethodInlineCacheTarget::Any => Value::Bool(decision.unwrap_or(false)),
                CallbackMethodInlineCacheTarget::All => Value::Bool(decision.unwrap_or(true)),
                CallbackMethodInlineCacheTarget::Count => Value::i64(count),
                CallbackMethodInlineCacheTarget::Sum => total.into_value(),
                _ => return Err(incomplete_callback()),
            },
            CallbackState::Map {
                target,
                operation,
                output,
                count,
                found,
                decision,
                ..
            } => match target {
                CallbackMethodInlineCacheTarget::MapValues
                | CallbackMethodInlineCacheTarget::Filter => {
                    map_methods::make_map_from_entries(output, heap, budget, operation)?
                }
                CallbackMethodInlineCacheTarget::Find => {
                    let payload = match found {
                        Some((key, value)) => {
                            Some(map_methods::map_entry(key, value, heap, budget)?)
                        }
                        None => None,
                    };
                    make_option_value(payload, heap, budget, operation)?
                }
                CallbackMethodInlineCacheTarget::Any => Value::Bool(decision.unwrap_or(false)),
                CallbackMethodInlineCacheTarget::All => Value::Bool(decision.unwrap_or(true)),
                CallbackMethodInlineCacheTarget::Count => Value::i64(count),
                _ => return Err(incomplete_callback()),
            },
            CallbackState::GroupBy {
                operation, groups, ..
            } => {
                let mut entries = Vec::with_capacity(groups.len());
                for group in groups.into_values() {
                    let values =
                        array_methods::make_array_value(group.values, heap, budget, operation)?;
                    entries.push((group.key, values));
                }
                map_methods::make_map_from_entries(entries, heap, budget, operation)?
            }
            CallbackState::Iterator(_)
            | CallbackState::SortBy(_)
            | CallbackState::Enum { .. }
            | CallbackState::Complete => {
                return Err(incomplete_callback());
            }
        };
        Ok(ResumableCallbackStep::Complete(value))
    }
}

fn callback_operation(
    receiver: StandardMethodReceiver,
    target: CallbackMethodInlineCacheTarget,
) -> Option<&'static str> {
    let supported = match receiver {
        StandardMethodReceiver::Array | StandardMethodReceiver::Set => matches!(
            target,
            CallbackMethodInlineCacheTarget::Map
                | CallbackMethodInlineCacheTarget::Filter
                | CallbackMethodInlineCacheTarget::Find
                | CallbackMethodInlineCacheTarget::Any
                | CallbackMethodInlineCacheTarget::All
                | CallbackMethodInlineCacheTarget::Count
                | CallbackMethodInlineCacheTarget::Sum
                | CallbackMethodInlineCacheTarget::GroupBy
                | CallbackMethodInlineCacheTarget::SortBy
        ),
        StandardMethodReceiver::Map => matches!(
            target,
            CallbackMethodInlineCacheTarget::MapValues
                | CallbackMethodInlineCacheTarget::Filter
                | CallbackMethodInlineCacheTarget::Find
                | CallbackMethodInlineCacheTarget::Any
                | CallbackMethodInlineCacheTarget::All
                | CallbackMethodInlineCacheTarget::Count
        ),
        StandardMethodReceiver::Option => matches!(
            target,
            CallbackMethodInlineCacheTarget::Map
                | CallbackMethodInlineCacheTarget::AndThen
                | CallbackMethodInlineCacheTarget::OrElse
                | CallbackMethodInlineCacheTarget::Filter
        ),
        StandardMethodReceiver::Result => matches!(
            target,
            CallbackMethodInlineCacheTarget::Map
                | CallbackMethodInlineCacheTarget::MapErr
                | CallbackMethodInlineCacheTarget::AndThen
                | CallbackMethodInlineCacheTarget::OrElse
        ),
        _ => false,
    };
    supported.then(|| match target {
        CallbackMethodInlineCacheTarget::Map => "method map",
        CallbackMethodInlineCacheTarget::MapValues => "method map_values",
        CallbackMethodInlineCacheTarget::MapErr => "method map_err",
        CallbackMethodInlineCacheTarget::AndThen => "method and_then",
        CallbackMethodInlineCacheTarget::OrElse => "method or_else",
        CallbackMethodInlineCacheTarget::Filter => "method filter",
        CallbackMethodInlineCacheTarget::Find => "method find",
        CallbackMethodInlineCacheTarget::Any => "method any",
        CallbackMethodInlineCacheTarget::All => "method all",
        CallbackMethodInlineCacheTarget::Count => "method count",
        CallbackMethodInlineCacheTarget::Sum => "method sum",
        CallbackMethodInlineCacheTarget::GroupBy => "method group_by",
        CallbackMethodInlineCacheTarget::SortBy => "method sort_by",
        _ => unreachable!(),
    })
}

fn enum_callback_is_active(
    receiver: StandardMethodReceiver,
    target: CallbackMethodInlineCacheTarget,
    variant: StdEnumVariant,
) -> bool {
    matches!(
        (receiver, target, variant),
        (
            StandardMethodReceiver::Option,
            CallbackMethodInlineCacheTarget::Map
                | CallbackMethodInlineCacheTarget::AndThen
                | CallbackMethodInlineCacheTarget::Filter,
            StdEnumVariant::Some,
        ) | (
            StandardMethodReceiver::Option,
            CallbackMethodInlineCacheTarget::OrElse,
            StdEnumVariant::None,
        ) | (
            StandardMethodReceiver::Result,
            CallbackMethodInlineCacheTarget::Map | CallbackMethodInlineCacheTarget::AndThen,
            StdEnumVariant::Ok,
        ) | (
            StandardMethodReceiver::Result,
            CallbackMethodInlineCacheTarget::MapErr | CallbackMethodInlineCacheTarget::OrElse,
            StdEnumVariant::Err,
        )
    )
}

fn enum_value(
    receiver: &Value,
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<(StdEnumVariant, Option<Value>)> {
    let Value::HeapRef(reference) = receiver else {
        return Err(VmError::new(VmErrorKind::TypeMismatch { operation }));
    };
    let Some(HeapValue::Enum {
        identity: Some(identity),
        fields,
        ..
    }) = heap.and_then(|heap| heap.heap.get(*reference))
    else {
        return Err(VmError::new(VmErrorKind::TypeMismatch { operation }));
    };
    let Some((_, variant)) = std_enum_tag(*identity) else {
        return Err(VmError::new(VmErrorKind::TypeMismatch { operation }));
    };
    let payload = if variant.has_payload() {
        Some(
            fields
                .get_slot(0, "0")
                .map(crate::stored_runtime_value)
                .ok_or_else(|| VmError::new(VmErrorKind::TypeMismatch { operation }))?,
        )
    } else {
        None
    };
    Ok((variant, payload))
}

#[allow(clippy::too_many_arguments)]
fn finish_active_enum_callback(
    receiver: StandardMethodReceiver,
    target: CallbackMethodInlineCacheTarget,
    variant: StdEnumVariant,
    payload: Option<Value>,
    returned: Value,
    operation: &'static str,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    match (receiver, target) {
        (StandardMethodReceiver::Option, CallbackMethodInlineCacheTarget::Map) => copy_enum_value(
            StdEnumVariant::Some,
            Some(returned),
            operation,
            heap,
            budget,
        ),
        (StandardMethodReceiver::Result, CallbackMethodInlineCacheTarget::Map) => {
            copy_enum_value(StdEnumVariant::Ok, Some(returned), operation, heap, budget)
        }
        (StandardMethodReceiver::Result, CallbackMethodInlineCacheTarget::MapErr) => {
            copy_enum_value(StdEnumVariant::Err, Some(returned), operation, heap, budget)
        }
        (
            StandardMethodReceiver::Option | StandardMethodReceiver::Result,
            CallbackMethodInlineCacheTarget::AndThen | CallbackMethodInlineCacheTarget::OrElse,
        ) => {
            let expected = match receiver {
                StandardMethodReceiver::Option => StdEnumKind::Option,
                StandardMethodReceiver::Result => StdEnumKind::Result,
                _ => unreachable!(),
            };
            let Some((returned_variant, _)) =
                enum_value(&returned, heap.as_deref(), operation).ok()
            else {
                return Err(VmError::new(VmErrorKind::TypeMismatch { operation }));
            };
            let actual = match returned_variant {
                StdEnumVariant::Some | StdEnumVariant::None => StdEnumKind::Option,
                StdEnumVariant::Ok | StdEnumVariant::Err => StdEnumKind::Result,
            };
            if actual != expected {
                return Err(VmError::new(VmErrorKind::TypeMismatch { operation }));
            }
            Ok(returned)
        }
        (StandardMethodReceiver::Option, CallbackMethodInlineCacheTarget::Filter) => {
            let payload = payload.ok_or_else(incomplete_callback)?;
            copy_enum_value(
                if is_truthy(&returned) {
                    StdEnumVariant::Some
                } else {
                    StdEnumVariant::None
                },
                is_truthy(&returned).then_some(payload),
                operation,
                heap,
                budget,
            )
        }
        _ => {
            let _ = variant;
            Err(incomplete_callback())
        }
    }
}

fn copy_enum_value(
    variant: StdEnumVariant,
    payload: Option<Value>,
    operation: &'static str,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    let Some(heap) = heap.as_deref_mut() else {
        return Err(VmError::new(VmErrorKind::TypeMismatch { operation }));
    };
    match variant {
        StdEnumVariant::Some => option_value(
            Some(payload.ok_or_else(incomplete_callback)?),
            heap,
            budget.as_deref_mut(),
        ),
        StdEnumVariant::None => option_value(None, heap, budget.as_deref_mut()),
        StdEnumVariant::Ok | StdEnumVariant::Err => result_value(
            variant,
            payload.ok_or_else(incomplete_callback)?,
            heap,
            budget.as_deref_mut(),
        ),
    }
}

fn make_option_value(
    payload: Option<Value>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<Value> {
    let Some(heap) = heap.as_deref_mut() else {
        return Err(VmError::new(VmErrorKind::TypeMismatch { operation }));
    };
    option_value(payload, heap, budget.as_deref_mut())
}

fn make_set_value(
    values: Vec<Value>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<Value> {
    let Some(heap) = heap.as_deref_mut() else {
        return Err(VmError::new(VmErrorKind::TypeMismatch { operation }));
    };
    let values = ScriptSet::from_values(values, Some(&*heap), operation)?;
    check_collection_len("set", 0, values.len(), budget.as_deref(), |budget| {
        budget.collection_limits().max_set_len
    })?;
    allocate_heap_value(HeapValue::Set(values), heap, budget.as_deref_mut())
}

fn update_sort_position(state: &mut SortByState, ordering: Ordering) {
    if ordering == Ordering::Less {
        state
            .entries
            .swap(state.current, state.current.saturating_sub(1));
        state.current = state.current.saturating_sub(1);
    } else {
        state.sort_index = state.sort_index.saturating_add(1);
        state.current = state.sort_index;
    }
}

fn incomplete_callback() -> VmError {
    VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
        opcode: "incomplete resumable callback method",
    })
}
