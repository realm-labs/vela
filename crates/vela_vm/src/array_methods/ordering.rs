use std::cmp::Ordering;

use crate::equality::{ResumableComparison, ResumableComparisonStep};
use crate::heap::HeapValue;
use crate::{ExecutionBudget, HeapExecution, Value, Vm, VmResult};

use super::{array_values, expect_arity, make_array_value, option_value, type_error};

#[derive(Clone, Copy)]
pub(crate) enum ResumableArrayOrderingKind {
    Sort,
    Min,
    Max,
}

pub(crate) struct ResumableArrayOrdering {
    state: ResumableArrayOrderingState,
    comparison: Option<ResumableComparison>,
}

pub(crate) enum ResumableArrayOrderingStep {
    Complete(Value),
    Call {
        function: vela_bytecode::ScriptFunctionHandle,
        args: Vec<Value>,
    },
}

enum ResumableArrayOrderingState {
    ReadyArray(Vec<Value>),
    Sort {
        entries: Vec<OrdSortEntry>,
        index: usize,
        current: usize,
    },
    Extremum {
        values: Vec<Value>,
        index: usize,
        best: Value,
        extremum: Extremum,
        operation: &'static str,
    },
    Complete,
}

impl ResumableArrayOrdering {
    pub(crate) fn new(
        kind: ResumableArrayOrderingKind,
        receiver: &Value,
        args: &[Value],
        heap: Option<&HeapExecution<'_>>,
    ) -> VmResult<Self> {
        let (name, operation) = match kind {
            ResumableArrayOrderingKind::Sort => ("sort", "method sort"),
            ResumableArrayOrderingKind::Min => ("min", "method min"),
            ResumableArrayOrderingKind::Max => ("max", "method max"),
        };
        expect_arity(name, args, 0)?;
        let values = array_values(receiver, heap, operation)?;
        let state = match kind {
            ResumableArrayOrderingKind::Sort => {
                match sort_values_by_key(values.clone(), heap, operation, |value, _| Ok(*value)) {
                    Ok(sorted) => ResumableArrayOrderingState::ReadyArray(sorted),
                    Err(_) => ResumableArrayOrderingState::Sort {
                        entries: values
                            .into_iter()
                            .map(|value| OrdSortEntry { key: value, value })
                            .collect(),
                        index: 1,
                        current: 1,
                    },
                }
            }
            ResumableArrayOrderingKind::Min | ResumableArrayOrderingKind::Max => {
                let Some(first) = values.first().copied() else {
                    return Ok(Self {
                        state: ResumableArrayOrderingState::Extremum {
                            values,
                            index: 0,
                            best: Value::Unit,
                            extremum: match kind {
                                ResumableArrayOrderingKind::Min => Extremum::Min,
                                ResumableArrayOrderingKind::Max => Extremum::Max,
                                ResumableArrayOrderingKind::Sort => unreachable!(),
                            },
                            operation,
                        },
                        comparison: None,
                    });
                };
                ResumableArrayOrderingState::Extremum {
                    values,
                    index: 1,
                    best: first,
                    extremum: match kind {
                        ResumableArrayOrderingKind::Min => Extremum::Min,
                        ResumableArrayOrderingKind::Max => Extremum::Max,
                        ResumableArrayOrderingKind::Sort => unreachable!(),
                    },
                    operation,
                }
            }
        };
        Ok(Self {
            state,
            comparison: None,
        })
    }

    pub(crate) fn step(
        &mut self,
        vm: &Vm,
        program: &vela_bytecode::LinkedProgram,
        host: Option<&crate::HostExecution<'_>>,
        heap: &mut Option<&mut HeapExecution<'_>>,
        budget: &mut Option<&mut ExecutionBudget>,
        returned: Option<Value>,
    ) -> VmResult<ResumableArrayOrderingStep> {
        if let Some(comparison) = self.comparison.as_mut() {
            match comparison.step(vm, program, host, heap, budget, returned)? {
                ResumableComparisonStep::Call { function, args } => {
                    return Ok(ResumableArrayOrderingStep::Call { function, args });
                }
                ResumableComparisonStep::CompleteOrdering(ordering) => {
                    self.comparison = None;
                    match &mut self.state {
                        ResumableArrayOrderingState::Sort {
                            entries,
                            index,
                            current,
                        } => {
                            if ordering == Ordering::Less {
                                entries.swap(*current, current.saturating_sub(1));
                                *current = current.saturating_sub(1);
                            } else {
                                *index = index.saturating_add(1);
                                *current = *index;
                            }
                        }
                        ResumableArrayOrderingState::Extremum {
                            values,
                            index,
                            best,
                            extremum,
                            ..
                        } => {
                            let replace = match extremum {
                                Extremum::Min => ordering.is_lt(),
                                Extremum::Max => ordering.is_gt(),
                            };
                            if replace {
                                *best = values[*index];
                            }
                            *index = index.saturating_add(1);
                        }
                        ResumableArrayOrderingState::ReadyArray(_)
                        | ResumableArrayOrderingState::Complete => {
                            return type_error("resumable array ordering");
                        }
                    }
                }
                ResumableComparisonStep::Complete(_) => {
                    return type_error("resumable array ordering");
                }
            }
        } else if returned.is_some() {
            return type_error("resumable array ordering");
        }

        loop {
            match &mut self.state {
                ResumableArrayOrderingState::ReadyArray(_) => {
                    let ResumableArrayOrderingState::ReadyArray(values) =
                        std::mem::replace(&mut self.state, ResumableArrayOrderingState::Complete)
                    else {
                        unreachable!()
                    };
                    return make_array_value(values, heap, budget, "method sort")
                        .map(ResumableArrayOrderingStep::Complete);
                }
                ResumableArrayOrderingState::Sort {
                    entries,
                    index,
                    current,
                } => {
                    if *index >= entries.len() {
                        let ResumableArrayOrderingState::Sort { entries, .. } = std::mem::replace(
                            &mut self.state,
                            ResumableArrayOrderingState::Complete,
                        ) else {
                            unreachable!()
                        };
                        let values = entries.into_iter().map(|entry| entry.value).collect();
                        return make_array_value(values, heap, budget, "method sort")
                            .map(ResumableArrayOrderingStep::Complete);
                    }
                    if *current == 0 {
                        *index = index.saturating_add(1);
                        *current = *index;
                        continue;
                    }
                    self.comparison = Some(ResumableComparison::total(
                        entries[*current].key,
                        entries[current.saturating_sub(1)].key,
                        "method sort",
                    ));
                }
                ResumableArrayOrderingState::Extremum {
                    values,
                    index,
                    best,
                    operation,
                    ..
                } => {
                    if values.is_empty() {
                        self.state = ResumableArrayOrderingState::Complete;
                        return option_value("None", None, heap, budget)
                            .map(ResumableArrayOrderingStep::Complete);
                    }
                    if *index >= values.len() {
                        let best = *best;
                        self.state = ResumableArrayOrderingState::Complete;
                        return option_value("Some", Some(best), heap, budget)
                            .map(ResumableArrayOrderingStep::Complete);
                    }
                    self.comparison =
                        Some(ResumableComparison::total(values[*index], *best, operation));
                }
                ResumableArrayOrderingState::Complete => {
                    return type_error("completed resumable array ordering");
                }
            }
            let comparison = self
                .comparison
                .as_mut()
                .expect("array ordering schedules a comparison");
            match comparison.step(vm, program, host, heap, budget, None)? {
                ResumableComparisonStep::Call { function, args } => {
                    return Ok(ResumableArrayOrderingStep::Call { function, args });
                }
                ResumableComparisonStep::CompleteOrdering(ordering) => {
                    self.comparison = None;
                    match &mut self.state {
                        ResumableArrayOrderingState::Sort {
                            entries,
                            index,
                            current,
                        } => {
                            if ordering == Ordering::Less {
                                entries.swap(*current, current.saturating_sub(1));
                                *current = current.saturating_sub(1);
                            } else {
                                *index = index.saturating_add(1);
                                *current = *index;
                            }
                        }
                        ResumableArrayOrderingState::Extremum {
                            values,
                            index,
                            best,
                            extremum,
                            ..
                        } => {
                            let replace = match extremum {
                                Extremum::Min => ordering.is_lt(),
                                Extremum::Max => ordering.is_gt(),
                            };
                            if replace {
                                *best = values[*index];
                            }
                            *index = index.saturating_add(1);
                        }
                        ResumableArrayOrderingState::ReadyArray(_)
                        | ResumableArrayOrderingState::Complete => {
                            return type_error("resumable array ordering");
                        }
                    }
                }
                ResumableComparisonStep::Complete(_) => {
                    return type_error("resumable array ordering");
                }
            }
        }
    }
}

fn sort_values_by_key(
    values: Vec<Value>,
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
    mut key_fn: impl FnMut(&Value, &[SortEntry]) -> VmResult<Value>,
) -> VmResult<Vec<Value>> {
    let mut entries = Vec::<SortEntry>::with_capacity(values.len());
    let mut key_kind = None;
    for value in values {
        let key_value = key_fn(&value, &entries)?;
        push_sort_entry(
            &mut entries,
            &mut key_kind,
            value,
            key_value,
            heap,
            operation,
        )?;
    }
    Ok(sort_entries(entries))
}

fn push_sort_entry(
    entries: &mut Vec<SortEntry>,
    key_kind: &mut Option<SortKeyKind>,
    value: Value,
    key_value: Value,
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<()> {
    let key = sort_key(&key_value, heap, operation)?;
    if let Some(expected) = *key_kind {
        if key.kind() != expected {
            return type_error(operation);
        }
    } else {
        *key_kind = Some(key.kind());
    }
    entries.push(SortEntry {
        key,
        value,
        index: entries.len(),
    });
    Ok(())
}

fn sort_entries(mut entries: Vec<SortEntry>) -> Vec<Value> {
    entries.sort_by(|left, right| {
        left.key
            .compare(&right.key)
            .then_with(|| left.index.cmp(&right.index))
    });
    entries.into_iter().map(|entry| entry.value).collect()
}

#[derive(Clone, Copy)]
enum Extremum {
    Min,
    Max,
}

struct SortEntry {
    key: SortKey,
    value: Value,
    index: usize,
}

struct OrdSortEntry {
    key: Value,
    value: Value,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum SortKeyKind {
    Numeric,
    String,
}

enum SortKey {
    Int(i64),
    String(String),
}

impl SortKey {
    fn kind(&self) -> SortKeyKind {
        match self {
            Self::Int(_) => SortKeyKind::Numeric,
            Self::String(_) => SortKeyKind::String,
        }
    }

    fn compare(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Int(left), Self::Int(right)) => left.cmp(right),
            (Self::String(left), Self::String(right)) => left.cmp(right),
            (Self::Int(_), Self::String(_)) | (Self::String(_), Self::Int(_)) => Ordering::Equal,
        }
    }
}

fn sort_key(
    value: &Value,
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<SortKey> {
    match value {
        Value::I64(value) => Ok(SortKey::Int(*value)),
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::String(value)) => Ok(SortKey::String(value.clone())),
            _ => type_error(operation),
        },
        _ => type_error(operation),
    }
}
