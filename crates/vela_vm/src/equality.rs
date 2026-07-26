use std::cmp::Ordering;
use vela_bytecode::linked::LinkedMethodDispatchKind;
use vela_bytecode::{LinkedProgram, derived_linked_record_trait_fields};
use vela_def::TypeId;
use vela_reflect::registry::TypeRegistry;

use crate::heap::{GcRef, HeapValue};
use crate::numeric_ops::{
    greater_equal_numeric, greater_numeric, less_equal_numeric, less_numeric,
};
use crate::option_result::{StdEnumKind, StdEnumVariant, std_enum_tag};
use crate::{
    ExecutionBudget, HeapExecution, HostExecution, Value, Vm, VmError, VmErrorKind, VmResult,
    store_value_in_heap_if_needed, stored_runtime_value,
};

const PARTIAL_EQ_METHOD: &str = "eq";
const PARTIAL_ORD_METHOD: &str = "partial_cmp";
const ORD_METHOD: &str = "cmp";

#[cfg(test)]
fn values_equal(lhs: &Value, rhs: &Value, heap: Option<&HeapExecution<'_>>) -> VmResult<bool> {
    if let Some(equal) = leaf_values_equal(lhs, rhs, heap)? {
        return Ok(equal);
    }
    non_comparable("equal")
}

#[derive(Clone, Copy)]
pub(crate) enum ResumableComparisonKind {
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

pub(crate) struct ResumableComparison {
    work: Vec<ComparisonWork>,
    result: Option<ComparisonResult>,
    awaiting: Option<AwaitedComparisonResult>,
}

pub(crate) enum ResumableComparisonStep {
    Complete(Value),
    CompleteOrdering(Ordering),
    Call {
        function: vela_bytecode::ScriptFunctionHandle,
        args: Vec<Value>,
    },
}

enum ComparisonWork {
    Direct {
        kind: ResumableComparisonKind,
        lhs: Value,
        rhs: Value,
    },
    FinishEqual {
        invert: bool,
    },
    FinishOrdering {
        op: OrderingOp,
    },
    FinishTotal,
    Evaluate {
        mode: ComparisonMode,
        lhs: Value,
        rhs: Value,
    },
    ReduceEqual {
        remaining: std::vec::IntoIter<(Value, Value)>,
    },
    ReducePartial {
        remaining: std::vec::IntoIter<(Value, Value)>,
        operation: &'static str,
    },
    ReduceTotal {
        remaining: std::vec::IntoIter<(Value, Value)>,
        operation: &'static str,
    },
}

#[derive(Clone, Copy)]
enum ComparisonMode {
    Equal,
    Partial { operation: &'static str },
    Total { operation: &'static str },
}

enum ComparisonResult {
    Equal(bool),
    Partial(Option<Ordering>),
    Total(Ordering),
    Final(Value),
    FinalOrdering(Ordering),
}

#[derive(Clone, Copy)]
enum AwaitedComparisonResult {
    Equal,
    Partial { operation: &'static str },
    Total { operation: &'static str },
}

impl ResumableComparison {
    pub(crate) fn new(kind: ResumableComparisonKind, lhs: Value, rhs: Value) -> Self {
        Self {
            work: vec![ComparisonWork::Direct { kind, lhs, rhs }],
            result: None,
            awaiting: None,
        }
    }

    pub(crate) fn total(lhs: Value, rhs: Value, operation: &'static str) -> Self {
        Self {
            work: vec![
                ComparisonWork::FinishTotal,
                ComparisonWork::Evaluate {
                    mode: ComparisonMode::Total { operation },
                    lhs,
                    rhs,
                },
            ],
            result: None,
            awaiting: None,
        }
    }

    pub(crate) fn step(
        &mut self,
        vm: &Vm,
        program: &LinkedProgram,
        host: Option<&HostExecution<'_>>,
        heap: &mut Option<&mut HeapExecution<'_>>,
        budget: &mut Option<&mut ExecutionBudget>,
        returned: Option<Value>,
    ) -> VmResult<ResumableComparisonStep> {
        if let Some(awaiting) = self.awaiting.take() {
            let returned = returned.expect("a resumed comparison has a child result");
            self.result = Some(match awaiting {
                AwaitedComparisonResult::Equal => {
                    let returned = store_value_in_heap_if_needed(
                        returned,
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                    )?;
                    let Value::Bool(value) = returned else {
                        return Err(VmError::new(VmErrorKind::TypeMismatch {
                            operation: "equal",
                        }));
                    };
                    ComparisonResult::Equal(value)
                }
                AwaitedComparisonResult::Partial { operation } => ComparisonResult::Partial(
                    partial_cmp_result(returned, heap.as_deref(), operation)?,
                ),
                AwaitedComparisonResult::Total { operation } => {
                    let returned = store_value_in_heap_if_needed(
                        returned,
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                    )?;
                    ComparisonResult::Total(total_cmp_result(returned, operation)?)
                }
            });
        } else {
            debug_assert!(returned.is_none());
        }

        loop {
            let Some(work) = self.work.pop() else {
                return match self.result.take() {
                    Some(ComparisonResult::Final(value)) => {
                        Ok(ResumableComparisonStep::Complete(value))
                    }
                    Some(ComparisonResult::FinalOrdering(ordering)) => {
                        Ok(ResumableComparisonStep::CompleteOrdering(ordering))
                    }
                    _ => Err(VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
                        opcode: "incomplete resumable comparison",
                    })),
                };
            };
            match work {
                ComparisonWork::Direct { kind, lhs, rhs } => match kind {
                    ResumableComparisonKind::Equal | ResumableComparisonKind::NotEqual => {
                        self.work.push(ComparisonWork::FinishEqual {
                            invert: matches!(kind, ResumableComparisonKind::NotEqual),
                        });
                        self.work.push(ComparisonWork::Evaluate {
                            mode: ComparisonMode::Equal,
                            lhs,
                            rhs,
                        });
                    }
                    ResumableComparisonKind::Less
                    | ResumableComparisonKind::LessEqual
                    | ResumableComparisonKind::Greater
                    | ResumableComparisonKind::GreaterEqual => {
                        let op = match kind {
                            ResumableComparisonKind::Less => OrderingOp::Less,
                            ResumableComparisonKind::LessEqual => OrderingOp::LessEqual,
                            ResumableComparisonKind::Greater => OrderingOp::Greater,
                            ResumableComparisonKind::GreaterEqual => OrderingOp::GreaterEqual,
                            ResumableComparisonKind::Equal | ResumableComparisonKind::NotEqual => {
                                unreachable!()
                            }
                        };
                        if let Ok(value) = op.numeric(&lhs, &rhs) {
                            self.result = Some(ComparisonResult::Final(Value::Bool(value)));
                        } else {
                            self.work.push(ComparisonWork::FinishOrdering { op });
                            self.work.push(ComparisonWork::Evaluate {
                                mode: ComparisonMode::Partial {
                                    operation: op.operation(),
                                },
                                lhs,
                                rhs,
                            });
                        }
                    }
                },
                ComparisonWork::FinishEqual { invert } => {
                    let Some(ComparisonResult::Equal(value)) = self.result.take() else {
                        return incomplete_comparison();
                    };
                    self.result = Some(ComparisonResult::Final(Value::Bool(value ^ invert)));
                }
                ComparisonWork::FinishOrdering { op } => {
                    let Some(ComparisonResult::Partial(ordering)) = self.result.take() else {
                        return incomplete_comparison();
                    };
                    self.result = Some(ComparisonResult::Final(Value::Bool(
                        ordering.is_some_and(|ordering| op.matches(ordering)),
                    )));
                }
                ComparisonWork::FinishTotal => {
                    let Some(ComparisonResult::Total(ordering)) = self.result.take() else {
                        return incomplete_comparison();
                    };
                    self.result = Some(ComparisonResult::FinalOrdering(ordering));
                }
                ComparisonWork::Evaluate { mode, lhs, rhs } => {
                    if let Some(result) =
                        immediate_comparison_result(mode, &lhs, &rhs, heap.as_deref())?
                    {
                        self.result = Some(result);
                        continue;
                    }
                    let Some((type_id, type_name)) =
                        receiver_type_identity(&lhs, heap.as_deref(), host, vm.type_registry())
                            .map(|(id, name)| (id, name.to_owned()))
                    else {
                        return Err(comparable_error(mode.operation()));
                    };
                    let method_name = mode.method_name();
                    if let Some(target) = linked_builtin_trait_target(program, type_id, method_name)
                    {
                        program.function(target.function).ok_or_else(|| {
                            VmError::new(VmErrorKind::UnknownMethod {
                                method: method_name.to_owned(),
                            })
                        })?;
                        self.awaiting = Some(mode.awaited_result());
                        return Ok(ResumableComparisonStep::Call {
                            function: target.function,
                            args: vec![lhs, rhs],
                        });
                    }
                    let trait_name = mode.trait_name();
                    let Some(field_names) =
                        derived_linked_record_trait_fields(program, &type_name, trait_name)
                    else {
                        return Err(comparable_error(mode.operation()));
                    };
                    let Some(field_pairs) =
                        record_field_pairs(&lhs, &rhs, heap.as_deref(), &type_name, &field_names)?
                    else {
                        return Err(comparable_error(mode.operation()));
                    };
                    self.begin_derived(mode, field_pairs);
                }
                ComparisonWork::ReduceEqual { mut remaining } => {
                    let Some(ComparisonResult::Equal(equal)) = self.result.take() else {
                        return incomplete_comparison();
                    };
                    if !equal {
                        self.result = Some(ComparisonResult::Equal(false));
                    } else if let Some((lhs, rhs)) = remaining.next() {
                        self.work.push(ComparisonWork::ReduceEqual { remaining });
                        self.work.push(ComparisonWork::Evaluate {
                            mode: ComparisonMode::Equal,
                            lhs,
                            rhs,
                        });
                    } else {
                        self.result = Some(ComparisonResult::Equal(true));
                    }
                }
                ComparisonWork::ReducePartial {
                    mut remaining,
                    operation,
                } => {
                    let Some(ComparisonResult::Partial(ordering)) = self.result.take() else {
                        return incomplete_comparison();
                    };
                    match ordering {
                        None => self.result = Some(ComparisonResult::Partial(None)),
                        Some(ordering) if ordering != Ordering::Equal => {
                            self.result = Some(ComparisonResult::Partial(Some(ordering)));
                        }
                        Some(_) => {
                            if let Some((lhs, rhs)) = remaining.next() {
                                self.work.push(ComparisonWork::ReducePartial {
                                    remaining,
                                    operation,
                                });
                                self.work.push(ComparisonWork::Evaluate {
                                    mode: ComparisonMode::Partial { operation },
                                    lhs,
                                    rhs,
                                });
                            } else {
                                self.result =
                                    Some(ComparisonResult::Partial(Some(Ordering::Equal)));
                            }
                        }
                    }
                }
                ComparisonWork::ReduceTotal {
                    mut remaining,
                    operation,
                } => {
                    let Some(ComparisonResult::Total(ordering)) = self.result.take() else {
                        return incomplete_comparison();
                    };
                    if ordering != Ordering::Equal {
                        self.result = Some(ComparisonResult::Total(ordering));
                    } else if let Some((lhs, rhs)) = remaining.next() {
                        self.work.push(ComparisonWork::ReduceTotal {
                            remaining,
                            operation,
                        });
                        self.work.push(ComparisonWork::Evaluate {
                            mode: ComparisonMode::Total { operation },
                            lhs,
                            rhs,
                        });
                    } else {
                        self.result = Some(ComparisonResult::Total(Ordering::Equal));
                    }
                }
            }
        }
    }

    fn begin_derived(&mut self, mode: ComparisonMode, field_pairs: Vec<(Value, Value)>) {
        let mut remaining = field_pairs.into_iter();
        let Some((lhs, rhs)) = remaining.next() else {
            self.result = Some(match mode {
                ComparisonMode::Equal => ComparisonResult::Equal(true),
                ComparisonMode::Partial { .. } => ComparisonResult::Partial(Some(Ordering::Equal)),
                ComparisonMode::Total { .. } => ComparisonResult::Total(Ordering::Equal),
            });
            return;
        };
        self.work.push(match mode {
            ComparisonMode::Equal => ComparisonWork::ReduceEqual { remaining },
            ComparisonMode::Partial { operation } => ComparisonWork::ReducePartial {
                remaining,
                operation,
            },
            ComparisonMode::Total { operation } => ComparisonWork::ReduceTotal {
                remaining,
                operation,
            },
        });
        self.work.push(ComparisonWork::Evaluate { mode, lhs, rhs });
    }
}

impl ComparisonMode {
    const fn operation(self) -> &'static str {
        match self {
            Self::Equal => "equal",
            Self::Partial { operation } | Self::Total { operation } => operation,
        }
    }

    const fn method_name(self) -> &'static str {
        match self {
            Self::Equal => PARTIAL_EQ_METHOD,
            Self::Partial { .. } => PARTIAL_ORD_METHOD,
            Self::Total { .. } => ORD_METHOD,
        }
    }

    const fn trait_name(self) -> &'static str {
        match self {
            Self::Equal => "PartialEq",
            Self::Partial { .. } => "PartialOrd",
            Self::Total { .. } => "Ord",
        }
    }

    const fn awaited_result(self) -> AwaitedComparisonResult {
        match self {
            Self::Equal => AwaitedComparisonResult::Equal,
            Self::Partial { operation } => AwaitedComparisonResult::Partial { operation },
            Self::Total { operation } => AwaitedComparisonResult::Total { operation },
        }
    }
}

fn immediate_comparison_result(
    mode: ComparisonMode,
    lhs: &Value,
    rhs: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Option<ComparisonResult>> {
    match mode {
        ComparisonMode::Equal => {
            leaf_values_equal(lhs, rhs, heap).map(|result| result.map(ComparisonResult::Equal))
        }
        ComparisonMode::Partial { operation } => leaf_values_partial_cmp(lhs, rhs, heap, operation)
            .map(|result| result.map(ComparisonResult::Partial)),
        ComparisonMode::Total { operation } => leaf_values_total_cmp(lhs, rhs, heap, operation)
            .map(|result| result.map(ComparisonResult::Total)),
    }
}

fn incomplete_comparison<T>() -> VmResult<T> {
    Err(VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
        opcode: "incomplete resumable comparison",
    }))
}

pub(crate) fn identity_equal(
    lhs: &Value,
    rhs: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<bool> {
    match (identity_key(lhs, heap)?, identity_key(rhs, heap)?) {
        (IdentityKey::Heap(lhs), IdentityKey::Heap(rhs)) => Ok(lhs == rhs),
        (IdentityKey::Host(lhs), IdentityKey::Host(rhs)) => Ok(lhs == rhs),
        (IdentityKey::Heap(_), IdentityKey::Host(_))
        | (IdentityKey::Host(_), IdentityKey::Heap(_)) => Ok(false),
    }
}

pub(crate) fn identity_not_equal(
    lhs: &Value,
    rhs: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<bool> {
    identity_equal(lhs, rhs, heap).map(|equal| !equal)
}

fn record_field_pairs(
    lhs: &Value,
    rhs: &Value,
    heap: Option<&HeapExecution<'_>>,
    type_name: &str,
    field_names: &[String],
) -> VmResult<Option<Vec<(Value, Value)>>> {
    let Some(heap) = heap else {
        return Ok(None);
    };
    let (Value::HeapRef(lhs), Value::HeapRef(rhs)) = (lhs, rhs) else {
        return Ok(None);
    };
    let Some(HeapValue::Record {
        fields: lhs_fields, ..
    }) = heap.heap.get(*lhs)
    else {
        return Ok(None);
    };
    let Some(HeapValue::Record {
        fields: rhs_fields, ..
    }) = heap.heap.get(*rhs)
    else {
        return Ok(None);
    };
    if lhs_fields.owner_name() != type_name || rhs_fields.owner_name() != type_name {
        return Ok(None);
    }
    field_names
        .iter()
        .map(|field_name| {
            let left = lhs_fields
                .get(field_name)
                .copied()
                .ok_or_else(|| comparable_error("equal"))?;
            let right = rhs_fields
                .get(field_name)
                .copied()
                .ok_or_else(|| comparable_error("equal"))?;
            Ok((left, right))
        })
        .collect::<VmResult<Vec<_>>>()
        .map(Some)
}

#[derive(Clone, Copy)]
struct LinkedBuiltinTraitTarget {
    function: vela_bytecode::ScriptFunctionHandle,
}

fn linked_builtin_trait_target(
    program: &LinkedProgram,
    owner: TypeId,
    method_name: &str,
) -> Option<LinkedBuiltinTraitTarget> {
    let dispatch = program.script_method_dispatch(owner, method_name)?;
    let dispatch = program.method_dispatch(dispatch)?;
    match &dispatch.kind {
        LinkedMethodDispatchKind::Script {
            method_id: _,
            function,
        } => Some(LinkedBuiltinTraitTarget {
            function: *function,
        }),
        _ => None,
    }
}

fn partial_cmp_result(
    result: Value,
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<Option<Ordering>> {
    let Value::HeapRef(reference) = result else {
        return non_comparable(operation);
    };
    let Some(HeapValue::Enum {
        identity: Some(identity),
        fields,
        ..
    }) = heap.and_then(|heap| heap.heap.get(reference))
    else {
        return non_comparable(operation);
    };
    match std_enum_tag(*identity) {
        Some((StdEnumKind::Option, StdEnumVariant::None)) => Ok(None),
        Some((StdEnumKind::Option, StdEnumVariant::Some)) => {
            let payload = fields
                .get_slot(0, "0")
                .map(stored_runtime_value)
                .ok_or_else(|| comparable_error(operation))?;
            partial_cmp_payload_ordering(payload, operation).map(Some)
        }
        _ => non_comparable(operation),
    }
}

fn partial_cmp_payload_ordering(value: Value, operation: &'static str) -> VmResult<Ordering> {
    let Value::I64(value) = value else {
        return non_comparable(operation);
    };
    Ok(value.cmp(&0))
}

fn total_cmp_result(value: Value, operation: &'static str) -> VmResult<Ordering> {
    let Value::I64(value) = value else {
        return non_comparable(operation);
    };
    Ok(value.cmp(&0))
}

#[derive(Clone, Copy)]
enum OrderingOp {
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

impl OrderingOp {
    fn operation(self) -> &'static str {
        match self {
            Self::Less => "less",
            Self::LessEqual => "less_equal",
            Self::Greater => "greater",
            Self::GreaterEqual => "greater_equal",
        }
    }

    fn numeric(self, lhs: &Value, rhs: &Value) -> VmResult<bool> {
        match self {
            Self::Less => less_numeric(lhs, rhs),
            Self::LessEqual => less_equal_numeric(lhs, rhs),
            Self::Greater => greater_numeric(lhs, rhs),
            Self::GreaterEqual => greater_equal_numeric(lhs, rhs),
        }
    }

    fn matches(self, ordering: Ordering) -> bool {
        match self {
            Self::Less => ordering == Ordering::Less,
            Self::LessEqual => matches!(ordering, Ordering::Less | Ordering::Equal),
            Self::Greater => ordering == Ordering::Greater,
            Self::GreaterEqual => matches!(ordering, Ordering::Greater | Ordering::Equal),
        }
    }
}

fn receiver_type_identity<'a>(
    receiver: &Value,
    heap: Option<&'a HeapExecution<'_>>,
    host: Option<&HostExecution<'_>>,
    registry: Option<&'a TypeRegistry>,
) -> Option<(TypeId, &'a str)> {
    match receiver {
        Value::HostRef(reference) => {
            let reference = host?.resolve_host_ref(*reference).ok()?;
            let desc = registry?.type_of_host(reference)?;
            Some((desc.key.id, desc.key.name.as_str()))
        }
        Value::HeapRef(reference) => match heap?.heap.get(*reference)? {
            HeapValue::Record {
                fields,
                identity: Some(identity),
            } => Some((identity.type_id, fields.owner_name())),
            HeapValue::Enum {
                enum_name,
                identity: Some(identity),
                ..
            } => Some((identity.type_id, enum_name.as_str())),
            _ => None,
        },
        _ => None,
    }
}

fn leaf_values_equal(
    lhs: &Value,
    rhs: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Option<bool>> {
    if let Some(equal) = immediate_leaf_values_equal(lhs, rhs) {
        return Ok(Some(equal));
    }

    match (heap_leaf(lhs, heap)?, heap_leaf(rhs, heap)?) {
        (Some(HeapLeaf::String(lhs)), Some(HeapLeaf::String(rhs))) => Ok(Some(lhs == rhs)),
        (Some(HeapLeaf::Bytes(lhs)), Some(HeapLeaf::Bytes(rhs))) => Ok(Some(lhs == rhs)),
        (Some(HeapLeaf::Range(lhs)), Some(HeapLeaf::Range(rhs))) => Ok(Some(lhs == rhs)),
        (Some(_), Some(_)) => Ok(Some(false)),
        (Some(_), None) | (None, Some(_)) if is_immediate_comparable_leaf(lhs, rhs) => {
            Ok(Some(false))
        }
        (Some(_), None) | (None, Some(_)) => Ok(None),
        (None, None) => Ok(None),
    }
}

fn leaf_values_total_cmp(
    lhs: &Value,
    rhs: &Value,
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<Option<Ordering>> {
    if let Some(ordering) = immediate_leaf_values_total_cmp(lhs, rhs) {
        return Ok(Some(ordering));
    }

    match (heap_leaf(lhs, heap)?, heap_leaf(rhs, heap)?) {
        (Some(HeapLeaf::String(lhs)), Some(HeapLeaf::String(rhs))) => Ok(Some(lhs.cmp(rhs))),
        (Some(HeapLeaf::Bytes(lhs)), Some(HeapLeaf::Bytes(rhs))) => Ok(Some(lhs.cmp(rhs))),
        (Some(_), Some(_)) => Ok(None),
        (Some(_), None) | (None, Some(_)) if is_immediate_leaf(lhs) || is_immediate_leaf(rhs) => {
            non_comparable(operation)
        }
        (Some(_), None) | (None, Some(_)) => Ok(None),
        (None, None) => Ok(None),
    }
}

fn leaf_values_partial_cmp(
    lhs: &Value,
    rhs: &Value,
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<Option<Option<Ordering>>> {
    if let Some(ordering) = immediate_leaf_values_partial_cmp(lhs, rhs, operation)? {
        return Ok(Some(ordering));
    }

    match (heap_leaf(lhs, heap)?, heap_leaf(rhs, heap)?) {
        (Some(HeapLeaf::String(lhs)), Some(HeapLeaf::String(rhs))) => Ok(Some(Some(lhs.cmp(rhs)))),
        (Some(HeapLeaf::Bytes(lhs)), Some(HeapLeaf::Bytes(rhs))) => Ok(Some(Some(lhs.cmp(rhs)))),
        (Some(_), Some(_)) => Ok(None),
        (Some(_), None) | (None, Some(_)) if is_immediate_leaf(lhs) || is_immediate_leaf(rhs) => {
            non_comparable(operation)
        }
        (Some(_), None) | (None, Some(_)) => Ok(None),
        (None, None) => Ok(None),
    }
}

fn immediate_leaf_values_equal(lhs: &Value, rhs: &Value) -> Option<bool> {
    match (lhs, rhs) {
        (Value::Missing, _) | (_, Value::Missing) => None,
        (Value::Unit, Value::Unit) => Some(true),
        (Value::Bool(lhs), Value::Bool(rhs)) => Some(lhs == rhs),
        (Value::Char(lhs), Value::Char(rhs)) => Some(lhs == rhs),
        (lhs, rhs) if lhs.is_scalar() && rhs.is_scalar() => {
            Some(lhs.as_scalar() == rhs.as_scalar())
        }
        (lhs, rhs)
            if is_immediate_comparable_leaf(lhs, rhs)
                && (is_immediate_leaf(lhs) || is_immediate_leaf(rhs)) =>
        {
            Some(false)
        }
        _ => None,
    }
}

fn immediate_leaf_values_partial_cmp(
    lhs: &Value,
    rhs: &Value,
    operation: &'static str,
) -> VmResult<Option<Option<Ordering>>> {
    if let Some(ordering) = immediate_leaf_values_total_cmp(lhs, rhs) {
        return Ok(Some(Some(ordering)));
    }
    match (lhs, rhs) {
        (Value::F32(lhs), Value::F32(rhs)) => Ok(Some(lhs.partial_cmp(rhs))),
        (Value::F64(lhs), Value::F64(rhs)) => Ok(Some(lhs.partial_cmp(rhs))),
        (lhs, rhs) if is_immediate_leaf(lhs) || is_immediate_leaf(rhs) => non_comparable(operation),
        _ => Ok(None),
    }
}

fn immediate_leaf_values_total_cmp(lhs: &Value, rhs: &Value) -> Option<Ordering> {
    match (lhs, rhs) {
        (Value::Bool(lhs), Value::Bool(rhs)) => Some(lhs.cmp(rhs)),
        (Value::Char(lhs), Value::Char(rhs)) => Some(lhs.cmp(rhs)),
        (Value::I8(lhs), Value::I8(rhs)) => Some(lhs.cmp(rhs)),
        (Value::I16(lhs), Value::I16(rhs)) => Some(lhs.cmp(rhs)),
        (Value::I32(lhs), Value::I32(rhs)) => Some(lhs.cmp(rhs)),
        (Value::I64(lhs), Value::I64(rhs)) => Some(lhs.cmp(rhs)),
        (Value::U8(lhs), Value::U8(rhs)) => Some(lhs.cmp(rhs)),
        (Value::U16(lhs), Value::U16(rhs)) => Some(lhs.cmp(rhs)),
        (Value::U32(lhs), Value::U32(rhs)) => Some(lhs.cmp(rhs)),
        (Value::U64(lhs), Value::U64(rhs)) => Some(lhs.cmp(rhs)),
        _ => None,
    }
}

fn is_immediate_comparable_leaf(lhs: &Value, rhs: &Value) -> bool {
    is_immediate_leaf(lhs) || is_immediate_leaf(rhs)
}

fn is_immediate_leaf(value: &Value) -> bool {
    matches!(
        value,
        Value::Unit
            | Value::Bool(_)
            | Value::Char(_)
            | Value::I8(_)
            | Value::I16(_)
            | Value::I32(_)
            | Value::I64(_)
            | Value::U8(_)
            | Value::U16(_)
            | Value::U32(_)
            | Value::U64(_)
            | Value::F32(_)
            | Value::F64(_)
    )
}

fn heap_leaf<'a>(
    value: &'a Value,
    heap: Option<&'a HeapExecution<'_>>,
) -> VmResult<Option<HeapLeaf<'a>>> {
    let Value::HeapRef(reference) = value else {
        return Ok(None);
    };
    let Some(heap_value) = heap.and_then(|heap| heap.heap.get(*reference)) else {
        return non_comparable("equal");
    };
    match heap_value {
        HeapValue::String(value) => Ok(Some(HeapLeaf::String(value))),
        HeapValue::Bytes(value) => Ok(Some(HeapLeaf::Bytes(value))),
        HeapValue::Range(value) => Ok(Some(HeapLeaf::Range(*value))),
        HeapValue::PathProxy(_) => non_comparable("equal"),
        HeapValue::Tuple(_)
        | HeapValue::Array(_)
        | HeapValue::Map(_)
        | HeapValue::Set(_)
        | HeapValue::Record { .. }
        | HeapValue::Enum { .. }
        | HeapValue::Closure(_)
        | HeapValue::Iterator(_) => Ok(None),
    }
}

fn identity_key(value: &Value, heap: Option<&HeapExecution<'_>>) -> VmResult<IdentityKey> {
    match value {
        Value::HeapRef(reference) => heap_identity_key(*reference, heap),
        Value::HostRef(reference) => Ok(IdentityKey::Host(*reference)),
        Value::Missing => non_comparable("identity equal"),
        Value::Unit
        | Value::Bool(_)
        | Value::Char(_)
        | Value::I8(_)
        | Value::I16(_)
        | Value::I32(_)
        | Value::I64(_)
        | Value::U8(_)
        | Value::U16(_)
        | Value::U32(_)
        | Value::U64(_)
        | Value::F32(_)
        | Value::F64(_) => non_comparable("identity equal"),
    }
}

fn heap_identity_key(reference: GcRef, heap: Option<&HeapExecution<'_>>) -> VmResult<IdentityKey> {
    let Some(heap_value) = heap.and_then(|heap| heap.heap.get(reference)) else {
        return non_comparable("identity equal");
    };
    match heap_value {
        HeapValue::Array(_)
        | HeapValue::Map(_)
        | HeapValue::Set(_)
        | HeapValue::Record { .. }
        | HeapValue::Enum { .. }
        | HeapValue::Closure(_)
        | HeapValue::Iterator(_) => Ok(IdentityKey::Heap(reference)),
        HeapValue::String(_)
        | HeapValue::Bytes(_)
        | HeapValue::Range(_)
        | HeapValue::Tuple(_)
        | HeapValue::PathProxy(_) => non_comparable("identity equal"),
    }
}

fn non_comparable<T>(operation: &'static str) -> VmResult<T> {
    Err(comparable_error(operation))
}

fn comparable_error(operation: &'static str) -> VmError {
    VmError::new(VmErrorKind::TypeMismatch { operation })
}

enum HeapLeaf<'a> {
    String(&'a str),
    Bytes(&'a [u8]),
    Range(crate::ranges::RangeValue),
}

enum IdentityKey {
    Heap(GcRef),
    Host(vela_host::path::HostSlotRef),
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use vela_common::{HostObjectId, HostTypeId, ShapeId};
    use vela_def::TypeId;
    use vela_host::path::{HostRef, HostSlotRef};
    use vela_host::proxy::PathProxy;
    use vela_host::target::HostTargetPlan;

    use crate::heap::{RecordIdentity, ScriptHeap};
    use crate::ranges::RangeValue;
    use crate::script_object::ScriptFields;

    use super::*;

    #[test]
    fn semantic_equality_is_tag_exact_for_leaf_values() {
        assert_eq!(equal(Value::Unit, Value::Unit), Ok(true));
        assert_eq!(equal(Value::Bool(true), Value::Bool(false)), Ok(false));
        assert_eq!(equal(Value::Char('v'), Value::Char('v')), Ok(true));
        assert_eq!(equal(Value::I64(1), Value::I64(1)), Ok(true));
        assert_eq!(equal(Value::I64(1), Value::U64(1)), Ok(false));
        assert_eq!(equal(Value::F64(f64::NAN), Value::F64(f64::NAN)), Ok(false));
        assert_eq!(equal(Value::F64(-0.0), Value::F64(0.0)), Ok(true));
        let mut heap = crate::heap::ScriptHeap::new();
        let first = heap.allocate(HeapValue::Range(RangeValue::new(0, 10, false)));
        let second = heap.allocate(HeapValue::Range(RangeValue::new(0, 10, false)));
        let third = heap.allocate(HeapValue::Range(RangeValue::new(0, 11, false)));
        let execution = HeapExecution::new(&mut heap);
        assert_eq!(
            values_equal(
                &Value::HeapRef(first),
                &Value::HeapRef(second),
                Some(&execution)
            ),
            Ok(true),
            "distinct heap ranges with equal bounds compare equal"
        );
        assert_eq!(
            values_equal(
                &Value::HeapRef(first),
                &Value::HeapRef(third),
                Some(&execution)
            ),
            Ok(false)
        );
    }

    #[test]
    fn semantic_equality_compares_string_and_bytes_payloads() {
        let mut heap = ScriptHeap::new();
        let left = Value::HeapRef(heap.allocate(HeapValue::String("gold".to_owned())));
        let right = Value::HeapRef(heap.allocate(HeapValue::String("gold".to_owned())));
        let bytes = Value::HeapRef(heap.allocate(HeapValue::Bytes(vec![1, 2, 3])));
        let same_bytes = Value::HeapRef(heap.allocate(HeapValue::Bytes(vec![1, 2, 3])));
        let heap = HeapExecution::new(&mut heap);

        assert_eq!(values_equal(&left, &right, Some(&heap)), Ok(true));
        assert_eq!(values_equal(&bytes, &same_bytes, Some(&heap)), Ok(true));
        assert_eq!(values_equal(&left, &bytes, Some(&heap)), Ok(false));
        assert_eq!(values_equal(&left, &Value::I64(1), Some(&heap)), Ok(false));
    }

    #[test]
    fn semantic_equality_rejects_objects_without_partial_eq() {
        let mut heap = ScriptHeap::new();
        let array = Value::HeapRef(heap.allocate(HeapValue::Array(Vec::new())));
        let record = Value::HeapRef(heap.allocate(record("Reward")));
        let heap = HeapExecution::new(&mut heap);

        assert_type_mismatch(values_equal(&array, &array, Some(&heap)), "equal");
        assert_type_mismatch(values_equal(&record, &record, Some(&heap)), "equal");
    }

    #[test]
    fn semantic_equality_rejects_missing_and_path_proxy() {
        assert_type_mismatch(
            values_equal(&Value::Missing, &Value::Missing, None),
            "equal",
        );

        let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(7), 1);
        let plan = HostTargetPlan::new(host_ref.type_id);
        let mut heap = ScriptHeap::new();
        let proxy =
            Value::HeapRef(heap.allocate(HeapValue::PathProxy(PathProxy::new(host_ref, plan))));
        let heap = HeapExecution::new(&mut heap);

        assert_type_mismatch(values_equal(&proxy, &proxy, Some(&heap)), "equal");
    }

    #[test]
    fn identity_equality_accepts_only_identity_values() {
        let mut heap = ScriptHeap::new();
        let first = Value::HeapRef(heap.allocate(record("Reward")));
        let second = Value::HeapRef(heap.allocate(record("Reward")));
        let string = Value::HeapRef(heap.allocate(HeapValue::String("Reward".to_owned())));
        let heap = HeapExecution::new(&mut heap);

        assert_eq!(identity_equal(&first, &first, Some(&heap)), Ok(true));
        assert_eq!(identity_equal(&first, &second, Some(&heap)), Ok(false));
        assert_type_mismatch(
            identity_equal(&string, &string, Some(&heap)),
            "identity equal",
        );
        assert_type_mismatch(
            identity_equal(&Value::I64(1), &Value::I64(1), Some(&heap)),
            "identity equal",
        );
    }

    #[test]
    fn identity_equality_compares_host_refs_without_host_reads() {
        let first = HostSlotRef::new(7, 1);
        let same = HostSlotRef::new(7, 1);
        let stale = HostSlotRef::new(7, 2);

        assert_eq!(
            identity_equal(&Value::HostRef(first), &Value::HostRef(same), None),
            Ok(true)
        );
        assert_eq!(
            identity_equal(&Value::HostRef(first), &Value::HostRef(stale), None),
            Ok(false)
        );
    }

    fn equal(lhs: Value, rhs: Value) -> VmResult<bool> {
        values_equal(&lhs, &rhs, None)
    }

    fn assert_type_mismatch(result: VmResult<bool>, operation: &'static str) {
        let error = result.expect_err("operation should reject non-comparable value");
        assert_eq!(error.kind(), VmErrorKind::TypeMismatch { operation });
    }

    fn record(type_name: &str) -> HeapValue {
        HeapValue::Record {
            identity: Some(RecordIdentity::new(TypeId::new(1), ShapeId::new(1))),
            fields: ScriptFields::from_pairs(
                type_name,
                BTreeMap::from([("id".to_owned(), Value::I64(1))]),
            ),
        }
    }
}
