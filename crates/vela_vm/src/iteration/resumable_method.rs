use std::sync::Arc;

use vela_bytecode::{LinkedArtifact, ScriptFunctionHandle};

use crate::heap::HeapValue;
use crate::heap_execution::HeapExecution;
use crate::heap_values::allocate_heap_value;
use crate::runtime_checks::{expect_closure_ref, is_truthy};
use crate::script_map::ScriptMap;
use crate::script_set::ScriptSet;
use crate::{
    CallbackMethodInlineCacheEntry, CallbackMethodInlineCacheTarget, ExecutionBudget,
    StandardMethodReceiver, Value, VmError, VmErrorKind, VmResult,
};

use super::methods::{
    check_collect_array_len, check_collect_map_len, check_collect_set_len, map_entry_value,
};
use super::{ResumableIteratorNext, ResumableIteratorStep};

pub(crate) struct ResumableIteratorMethod {
    receiver: Value,
    target: CallbackMethodInlineCacheTarget,
    operation: &'static str,
    callback_value: Option<Value>,
    callback: Option<PreparedCallback>,
    next: Option<ResumableIteratorNext>,
    awaiting_outer: Option<Value>,
    values: Vec<Value>,
    set: ScriptSet,
    map_entries: Vec<(Value, Value)>,
    count: i64,
    found: Option<Value>,
    decision: Option<bool>,
}

pub(crate) enum ResumableIteratorMethodStep {
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
}

impl ResumableIteratorMethod {
    pub(crate) fn new(
        receiver: Value,
        cache: CallbackMethodInlineCacheEntry,
        args: &[Value],
    ) -> Option<VmResult<Self>> {
        if cache.receiver != StandardMethodReceiver::Iterator {
            return None;
        }
        let (operation, arity) = match cache.target {
            CallbackMethodInlineCacheTarget::Next => ("method next", 0),
            CallbackMethodInlineCacheTarget::Count => ("method count", 0),
            CallbackMethodInlineCacheTarget::CollectArray => ("method collect_array", 0),
            CallbackMethodInlineCacheTarget::CollectSet => ("method collect_set", 0),
            CallbackMethodInlineCacheTarget::CollectMap => ("method collect_map", 0),
            CallbackMethodInlineCacheTarget::Find => ("method find", 1),
            CallbackMethodInlineCacheTarget::Any => ("method any", 1),
            CallbackMethodInlineCacheTarget::All => ("method all", 1),
            CallbackMethodInlineCacheTarget::Map | CallbackMethodInlineCacheTarget::Filter => {
                return None;
            }
            _ => return None,
        };
        if args.len() != arity {
            return Some(Err(VmError::new(VmErrorKind::ArityMismatch {
                name: operation.trim_start_matches("method ").to_owned(),
                expected: arity,
                actual: args.len(),
            })));
        }
        Some(Ok(Self {
            receiver,
            target: cache.target,
            operation,
            callback_value: args.first().copied(),
            callback: None,
            next: None,
            awaiting_outer: None,
            values: Vec::new(),
            set: ScriptSet::new(),
            map_entries: Vec::new(),
            count: 0,
            found: None,
            decision: None,
        }))
    }

    pub(crate) fn step(
        &mut self,
        program_owner: &Arc<LinkedArtifact>,
        heap: &mut Option<&mut HeapExecution<'_>>,
        budget: &mut Option<&mut ExecutionBudget>,
        mut returned: Option<Value>,
    ) -> VmResult<ResumableIteratorMethodStep> {
        if let Some(value) = self.awaiting_outer.take() {
            let predicate = returned.take().ok_or_else(incomplete_iterator_method)?;
            match self.target {
                CallbackMethodInlineCacheTarget::Find if is_truthy(&predicate) => {
                    self.found = Some(value);
                }
                CallbackMethodInlineCacheTarget::Any if is_truthy(&predicate) => {
                    self.decision = Some(true);
                }
                CallbackMethodInlineCacheTarget::All if !is_truthy(&predicate) => {
                    self.decision = Some(false);
                }
                CallbackMethodInlineCacheTarget::Find
                | CallbackMethodInlineCacheTarget::Any
                | CallbackMethodInlineCacheTarget::All => {}
                _ => return Err(incomplete_iterator_method()),
            }
        }

        loop {
            if self.is_terminal() {
                return self.finish(heap, budget);
            }
            let next = self.next.get_or_insert_with(|| {
                ResumableIteratorNext::new(self.receiver, self.operation, true)
            });
            match next.step(program_owner, heap, budget, returned.take())? {
                ResumableIteratorStep::Call {
                    owner,
                    function,
                    captures,
                    args,
                } => {
                    return Ok(ResumableIteratorMethodStep::Call {
                        owner,
                        function,
                        captures,
                        args,
                    });
                }
                ResumableIteratorStep::Complete(Some(value)) => {
                    self.next = None;
                    match self.target {
                        CallbackMethodInlineCacheTarget::Next => {
                            let Some(heap) = heap.as_deref_mut() else {
                                return type_error(self.operation);
                            };
                            let value = crate::option_result::option_value(
                                Some(value),
                                heap,
                                budget.as_deref_mut(),
                            )?;
                            return Ok(ResumableIteratorMethodStep::Complete(value));
                        }
                        CallbackMethodInlineCacheTarget::Count => {
                            self.count = self.count.checked_add(1).ok_or_else(|| {
                                VmError::new(VmErrorKind::TypeMismatch {
                                    operation: self.operation,
                                })
                            })?;
                        }
                        CallbackMethodInlineCacheTarget::CollectArray => self.values.push(value),
                        CallbackMethodInlineCacheTarget::CollectSet => {
                            self.set.insert(value, heap.as_deref(), self.operation)?;
                        }
                        CallbackMethodInlineCacheTarget::CollectMap => {
                            self.map_entries.push(map_entry_value(
                                &value,
                                heap.as_deref(),
                                self.operation,
                            )?);
                        }
                        CallbackMethodInlineCacheTarget::Find
                        | CallbackMethodInlineCacheTarget::Any
                        | CallbackMethodInlineCacheTarget::All => {
                            self.awaiting_outer = Some(value);
                            if let Some(budget) = budget.as_deref_mut() {
                                budget.charge_execution_units(1)?;
                            }
                            let callback = self.prepare_callback(heap.as_deref())?;
                            return Ok(ResumableIteratorMethodStep::Call {
                                owner: Arc::clone(&callback.owner),
                                function: callback.function,
                                captures: callback.captures.clone(),
                                args: vec![value],
                            });
                        }
                        _ => return Err(incomplete_iterator_method()),
                    }
                }
                ResumableIteratorStep::Complete(None) => {
                    self.next = None;
                    return self.finish(heap, budget);
                }
            }
        }
    }

    pub(crate) fn protect_roots(&self, heap: &mut HeapExecution<'_>) {
        heap.protect_values(&[self.receiver]);
        if let Some(callback) = self.callback_value {
            heap.protect_values(&[callback]);
        }
        if let Some(callback) = &self.callback {
            heap.protect_values(&callback.captures);
        }
        if let Some(value) = self.awaiting_outer {
            heap.protect_values(&[value]);
        }
        heap.protect_values(&self.values);
        for value in self.set.values() {
            heap.protect_values(&[*value]);
        }
        for (key, value) in &self.map_entries {
            heap.protect_values(&[*key, *value]);
        }
        if let Some(next) = &self.next {
            next.protect_roots(heap);
        }
    }

    fn is_terminal(&self) -> bool {
        self.target == CallbackMethodInlineCacheTarget::Find && self.found.is_some()
            || self.target == CallbackMethodInlineCacheTarget::Any && self.decision == Some(true)
            || self.target == CallbackMethodInlineCacheTarget::All && self.decision == Some(false)
    }

    fn prepare_callback(
        &mut self,
        heap: Option<&HeapExecution<'_>>,
    ) -> VmResult<&PreparedCallback> {
        if self.callback.is_none() {
            let callback = self.callback_value.ok_or_else(incomplete_iterator_method)?;
            let closure = expect_closure_ref(&callback, heap, self.operation)?;
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
            });
        }
        Ok(self.callback.as_ref().expect("callback was prepared"))
    }

    fn finish(
        &mut self,
        heap: &mut Option<&mut HeapExecution<'_>>,
        budget: &mut Option<&mut ExecutionBudget>,
    ) -> VmResult<ResumableIteratorMethodStep> {
        let value = match self.target {
            CallbackMethodInlineCacheTarget::Next => {
                let Some(heap) = heap.as_deref_mut() else {
                    return type_error(self.operation);
                };
                crate::option_result::option_value(None, heap, budget.as_deref_mut())?
            }
            CallbackMethodInlineCacheTarget::Count => Value::I64(self.count),
            CallbackMethodInlineCacheTarget::Find => {
                let Some(heap) = heap.as_deref_mut() else {
                    return type_error(self.operation);
                };
                crate::option_result::option_value(self.found, heap, budget.as_deref_mut())?
            }
            CallbackMethodInlineCacheTarget::Any => Value::Bool(self.decision == Some(true)),
            CallbackMethodInlineCacheTarget::All => Value::Bool(self.decision != Some(false)),
            CallbackMethodInlineCacheTarget::CollectArray => {
                check_collect_array_len(self.values.len(), budget.as_deref())?;
                let Some(heap) = heap.as_deref_mut() else {
                    return type_error(self.operation);
                };
                allocate_heap_value(
                    HeapValue::Array(std::mem::take(&mut self.values)),
                    heap,
                    budget.as_deref_mut(),
                )?
            }
            CallbackMethodInlineCacheTarget::CollectSet => {
                check_collect_set_len(self.set.len(), budget.as_deref())?;
                let Some(heap) = heap.as_deref_mut() else {
                    return type_error(self.operation);
                };
                allocate_heap_value(
                    HeapValue::Set(std::mem::take(&mut self.set)),
                    heap,
                    budget.as_deref_mut(),
                )?
            }
            CallbackMethodInlineCacheTarget::CollectMap => {
                let Some(heap) = heap.as_deref_mut() else {
                    return type_error(self.operation);
                };
                let map = ScriptMap::from_entries(
                    std::mem::take(&mut self.map_entries),
                    Some(&*heap),
                    self.operation,
                )?;
                check_collect_map_len(map.len(), budget.as_deref())?;
                allocate_heap_value(HeapValue::Map(map), heap, budget.as_deref_mut())?
            }
            _ => return Err(incomplete_iterator_method()),
        };
        Ok(ResumableIteratorMethodStep::Complete(value))
    }
}

fn incomplete_iterator_method() -> VmError {
    VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
        opcode: "incomplete resumable iterator method",
    })
}

fn type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}
