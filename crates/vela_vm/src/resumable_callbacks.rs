use std::sync::Arc;

use vela_bytecode::{LinkedArtifact, ScriptFunctionHandle};

use crate::collection_mutation::check_collection_len;
use crate::heap::HeapValue;
use crate::heap_execution::HeapExecution;
use crate::heap_values::allocate_heap_value;
use crate::option_result::option_value;
use crate::runtime_checks::{expect_closure_ref, is_truthy};
use crate::script_set::ScriptSet;
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
    Sequence {
        receiver: StandardMethodReceiver,
        target: CallbackMethodInlineCacheTarget,
        operation: &'static str,
        values: Vec<Value>,
        index: usize,
        output: Vec<Value>,
        count: i64,
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
    Complete,
}

impl ResumableCallbackMethod {
    pub(crate) fn new(
        receiver: &Value,
        cache: CallbackMethodInlineCacheEntry,
        args: &[Value],
        heap: Option<&HeapExecution<'_>>,
    ) -> Option<VmResult<Self>> {
        let operation = callback_operation(cache.receiver, cache.target)?;
        if args.len() != 1 {
            return Some(Err(VmError::new(VmErrorKind::ArityMismatch {
                name: operation.trim_start_matches("method ").to_owned(),
                expected: 1,
                actual: args.len(),
            })));
        }
        let state = match cache.receiver {
            StandardMethodReceiver::Array => CallbackState::Sequence {
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
                found: None,
                decision: None,
                awaiting: None,
            },
            StandardMethodReceiver::Set => CallbackState::Sequence {
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
                found: None,
                decision: None,
                awaiting: None,
            },
            StandardMethodReceiver::Map => CallbackState::Map {
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
        heap: &mut Option<&mut HeapExecution<'_>>,
        budget: &mut Option<&mut ExecutionBudget>,
        returned: Option<Value>,
    ) -> VmResult<ResumableCallbackStep> {
        match &mut self.state {
            CallbackState::Sequence {
                target,
                output,
                count,
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
            CallbackState::Complete => return Err(incomplete_callback()),
        }

        let has_next = match &self.state {
            CallbackState::Sequence { values, index, .. } => *index < values.len(),
            CallbackState::Map { entries, index, .. } => *index < entries.len(),
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
            CallbackState::Complete => return Err(incomplete_callback()),
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
            CallbackState::Complete => return Err(incomplete_callback()),
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
        _ => false,
    };
    supported.then(|| match target {
        CallbackMethodInlineCacheTarget::Map => "method map",
        CallbackMethodInlineCacheTarget::MapValues => "method map_values",
        CallbackMethodInlineCacheTarget::Filter => "method filter",
        CallbackMethodInlineCacheTarget::Find => "method find",
        CallbackMethodInlineCacheTarget::Any => "method any",
        CallbackMethodInlineCacheTarget::All => "method all",
        CallbackMethodInlineCacheTarget::Count => "method count",
        _ => unreachable!(),
    })
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

fn incomplete_callback() -> VmError {
    VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
        opcode: "incomplete resumable callback method",
    })
}
