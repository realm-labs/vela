use std::cmp::Ordering;

use crate::equality::{ResumableComparison, ResumableComparisonStep};
use crate::heap::HeapValue;
use crate::iteration;
use crate::method_runtime::{MethodRuntime, call_callback_with_protected_values};
use crate::{
    EqualityRuntime, ExecutionBudget, HeapExecution, Value, Vm, VmResult,
    values_total_cmp_with_traits,
};

use super::{
    array_values, call_unary_callback, expect_arity, make_array_value, option_value, type_error,
};

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
        heap: &mut Option<&mut HeapExecution<'_>>,
        budget: &mut Option<&mut ExecutionBudget>,
        returned: Option<Value>,
    ) -> VmResult<ResumableArrayOrderingStep> {
        if let Some(comparison) = self.comparison.as_mut() {
            match comparison.step(vm, program, heap, budget, returned)? {
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
            match comparison.step(vm, program, heap, budget, None)? {
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

pub(crate) fn sort_by(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("sort_by", args, 1)?;
    let values = array_values(receiver, runtime.heap.as_deref(), "method sort_by")?;
    let mut key_entries = Vec::<SortEntry>::with_capacity(values.len());
    let mut ord_entries = Vec::<OrdSortEntry>::with_capacity(values.len());
    let mut key_kind = None;
    let mut can_key_sort = true;
    iteration::try_for_each_over(values, &mut runtime, "method sort_by", |runtime, value| {
        let key_value = if runtime.heap.is_some() {
            call_callback_with_protected_values(
                runtime,
                "method sort_by",
                &args[0],
                std::slice::from_ref(&value),
                ord_entries
                    .iter()
                    .flat_map(|entry| [&entry.value, &entry.key]),
            )?
        } else {
            call_unary_callback(runtime, "method sort_by", &args[0], value, &[])?
        };
        if can_key_sort {
            match sort_key(&key_value, runtime.heap.as_deref(), "method sort_by") {
                Ok(key) => {
                    if let Some(expected) = key_kind {
                        if key.kind() != expected {
                            can_key_sort = false;
                        }
                    } else {
                        key_kind = Some(key.kind());
                    }
                    if can_key_sort {
                        key_entries.push(SortEntry {
                            key,
                            value,
                            index: key_entries.len(),
                        });
                    }
                }
                Err(_) => {
                    can_key_sort = false;
                }
            }
        }
        ord_entries.push(OrdSortEntry {
            key: key_value,
            value,
        });
        Ok(())
    })?;
    let values = if can_key_sort {
        sort_entries(key_entries)
    } else {
        let mut equality_runtime = EqualityRuntime {
            vm: runtime.vm,
            program: runtime.program,
            host: runtime.host.as_deref_mut(),
            heap: runtime.heap.as_deref_mut(),
            budget: runtime.budget.as_deref_mut(),
            caller_roots: runtime.caller_roots,
            inline_caches: runtime.inline_caches,
            bytecode_profiler: runtime.bytecode_profiler,
        };
        sort_entries_by_ord(ord_entries, &mut equality_runtime, "method sort_by")?
    };
    make_array_value(
        values,
        &mut runtime.heap,
        &mut runtime.budget,
        "method sort_by",
    )
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

fn sort_entries_by_ord(
    mut entries: Vec<OrdSortEntry>,
    runtime: &mut EqualityRuntime<'_, '_, '_>,
    operation: &'static str,
) -> VmResult<Vec<Value>> {
    let protected_root_len = runtime.heap.as_deref_mut().map(|heap| {
        let protected_root_len = runtime.caller_roots.push_to_heap(heap);
        for entry in &entries {
            heap.protect_values(&[entry.key, entry.value]);
        }
        protected_root_len
    });
    let result: VmResult<()> = (|| {
        for index in 1..entries.len() {
            let mut current = index;
            while current > 0 {
                let ordering = values_total_cmp_with_traits(
                    &entries[current].key,
                    &entries[current - 1].key,
                    runtime,
                    operation,
                )?;
                if ordering != Ordering::Less {
                    break;
                }
                entries.swap(current, current - 1);
                current -= 1;
            }
        }
        Ok(())
    })();
    if let (Some(heap), Some(protected_root_len)) =
        (runtime.heap.as_deref_mut(), protected_root_len)
    {
        heap.truncate_protected_roots(protected_root_len);
    }
    result?;
    Ok(entries.into_iter().map(|entry| entry.value).collect())
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
