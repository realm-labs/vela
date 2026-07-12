use std::collections::BTreeMap;

use crate::{
    MirAggregate, MirBlockId, MirFieldTarget, MirFunction, MirImmediate, MirLiveValue, MirOperand,
    MirPlace, MirProgram, MirRvalue, MirStatementId, MirStatementKind, MirTerminatorKind,
    MirTypeContract, MirValueType,
};

type MirShapeFields = BTreeMap<String, (MirShapeFieldIdentity, Option<Box<MirValueFact>>)>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirShapeFieldIdentity {
    Stable(vela_def::FieldId),
    Ordinal(u32),
}

#[derive(Clone, Debug, PartialEq)]
pub enum MirShapeFact {
    Record(MirShapeFields),
    Variant(MirShapeFields),
    Array(Option<Box<MirValueFact>>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum MirFamilyFact {
    Tuple(u32),
    Option,
    Result,
    Iterator(Option<Box<MirValueFact>>),
    Callable {
        accepted_kinds: crate::MirCallableKindSet,
        positional_arity: Option<u32>,
        return_fact: Option<Box<MirValueFact>>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct MirValueFact {
    pub value_type: MirValueType,
    pub immediate: Option<MirImmediate>,
    pub constant_provenance: Option<crate::MirConstantProvenance>,
    pub shape: Option<MirShapeFact>,
    pub family: Option<MirFamilyFact>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MirProgramPointFacts {
    pub block_entry: BTreeMap<MirBlockId, BTreeMap<MirLiveValue, MirValueFact>>,
    pub statement_before: BTreeMap<MirStatementId, BTreeMap<MirLiveValue, MirValueFact>>,
}

impl MirProgramPointFacts {
    #[must_use]
    pub fn operand_before(
        &self,
        statement: MirStatementId,
        operand: &MirOperand,
    ) -> Option<MirValueFact> {
        match operand {
            MirOperand::Immediate(value) => Some(MirValueFact {
                value_type: value.value_type(),
                immediate: Some(*value),
                constant_provenance: None,
                shape: None,
                family: None,
            }),
            MirOperand::Local(local) => self
                .statement_before
                .get(&statement)?
                .get(&MirLiveValue::Local(*local))
                .cloned(),
            MirOperand::Temp(temp) => self
                .statement_before
                .get(&statement)?
                .get(&MirLiveValue::Temp(*temp))
                .cloned(),
        }
    }
}

pub(crate) fn analyze(program: &MirProgram, function: &MirFunction) -> MirProgramPointFacts {
    type State = BTreeMap<MirLiveValue, MirValueFact>;
    let blocks = function.blocks().map(|(id, _)| id).collect::<Vec<_>>();
    let entry = function.entry_block();
    let initial = function
        .parameters()
        .iter()
        .map(|parameter| MirLiveValue::Local(parameter.storage))
        .chain(
            function
                .captures()
                .iter()
                .map(|capture| MirLiveValue::Local(capture.storage)),
        )
        .filter_map(|value| declared_fact(function, value).map(|fact| (value, fact)))
        .collect::<State>();
    let mut block_entry = blocks
        .iter()
        .map(|block| (*block, (*block == entry).then_some(initial.clone())))
        .collect::<BTreeMap<_, _>>();

    loop {
        let mut changed = false;
        for block in blocks.iter().copied() {
            if block == entry {
                continue;
            }
            let incoming = function.blocks().filter_map(|(predecessor, data)| {
                let state = block_entry.get(&predecessor)?.as_ref()?;
                let terminator = data.terminator()?;
                successors(&terminator.kind)
                    .contains(&block)
                    .then(|| transfer_block(program, function, predecessor, state, Some(block)))
            });
            let next = intersect(incoming);
            if block_entry.get(&block) != Some(&next) {
                block_entry.insert(block, next);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    let mut statement_before = BTreeMap::new();
    for block in blocks {
        let Some(mut state) = block_entry.get(&block).cloned().flatten() else {
            continue;
        };
        let data = function.block(block).expect("fact block exists");
        for statement in data.statements() {
            statement_before.insert(*statement, state.clone());
            transfer_statement(program, function, *statement, &mut state);
        }
    }
    MirProgramPointFacts {
        block_entry: block_entry
            .into_iter()
            .filter_map(|(block, state)| state.map(|state| (block, state)))
            .collect(),
        statement_before,
    }
}

fn transfer_block(
    program: &MirProgram,
    function: &MirFunction,
    block: MirBlockId,
    input: &BTreeMap<MirLiveValue, MirValueFact>,
    successor: Option<MirBlockId>,
) -> BTreeMap<MirLiveValue, MirValueFact> {
    let data = function.block(block).expect("fact block exists");
    let mut state = input.clone();
    for statement in data.statements() {
        transfer_statement(program, function, *statement, &mut state);
    }
    if let (Some(successor), Some(terminator)) = (successor, data.terminator())
        && let MirTerminatorKind::GuardBranch {
            value,
            guard,
            passed,
            ..
        } = &terminator.kind
        && *passed == successor
        && let Some(value) = operand_value(value)
        && let Some(crate::MirGuard {
            assumption: crate::MirGuardAssumption::Type(contract),
            ..
        }) = function.guard(*guard)
        && let Some((value_type, family)) = contract_fact(contract)
    {
        let fact = state.entry(value).or_insert(MirValueFact {
            value_type,
            immediate: None,
            constant_provenance: None,
            shape: None,
            family: family.clone(),
        });
        fact.value_type = value_type;
        fact.family = family;
    }
    if let (Some(successor), Some(terminator)) = (successor, data.terminator()) {
        match &terminator.kind {
            MirTerminatorKind::IteratorNext {
                iterator,
                item,
                next,
                ..
            } if *next == successor => {
                let item_fact = operand_fact(&state, iterator).and_then(|fact| match fact.family {
                    Some(MirFamilyFact::Iterator(item)) => item.as_deref().cloned(),
                    _ => None,
                });
                if let Some(fact) = item_fact {
                    state.insert(MirLiveValue::Local(*item), fact);
                } else {
                    install_declared(function, MirLiveValue::Local(*item), &mut state);
                }
            }
            MirTerminatorKind::RangeNext {
                cursor,
                exhausted,
                item,
                next,
                done,
                ..
            } if *next == successor || *done == successor => {
                install_declared(function, MirLiveValue::Local(*cursor), &mut state);
                install_declared(function, MirLiveValue::Local(*exhausted), &mut state);
                if *next == successor {
                    install_declared(function, MirLiveValue::Local(*item), &mut state);
                }
            }
            MirTerminatorKind::TrySwitch {
                result,
                continuations,
                ..
            } if continuations
                .iter()
                .any(|continuation| continuation.block == successor) =>
            {
                install_declared(function, MirLiveValue::Local(*result), &mut state);
            }
            _ => {}
        }
    }
    state
}

fn install_declared(
    function: &MirFunction,
    value: MirLiveValue,
    state: &mut BTreeMap<MirLiveValue, MirValueFact>,
) {
    if let Some(fact) = declared_fact(function, value) {
        state.insert(value, fact);
    } else {
        state.remove(&value);
    }
}

fn transfer_statement(
    program: &MirProgram,
    function: &MirFunction,
    statement_id: MirStatementId,
    state: &mut BTreeMap<MirLiveValue, MirValueFact>,
) {
    let statement = function
        .statement(statement_id)
        .expect("fact statement exists");
    if let MirStatementKind::GuardTrap { value, guard } = &statement.kind
        && let Some(value) = operand_value(value)
        && let Some(crate::MirGuard {
            assumption: crate::MirGuardAssumption::Type(contract),
            ..
        }) = function.guard(*guard)
        && let Some((value_type, family)) = contract_fact(contract)
    {
        let fact = state.entry(value).or_insert(MirValueFact {
            value_type,
            immediate: None,
            constant_provenance: None,
            shape: None,
            family: family.clone(),
        });
        fact.value_type = value_type;
        fact.family = family;
    }
    let Some(destination) = statement.destination.map(place_value) else {
        return;
    };
    let fact = match &statement.kind {
        MirStatementKind::Assign(MirRvalue::Use(operand)) => operand_fact(state, operand),
        MirStatementKind::Assign(MirRvalue::Constant { value, provenance }) => Some(MirValueFact {
            value_type: value.value_type(),
            immediate: Some(*value),
            constant_provenance: Some(*provenance),
            shape: None,
            family: None,
        }),
        MirStatementKind::Unary { operation, .. } => Some(scalar_fact(match operation {
            crate::MirUnaryOp::NotBool => vela_common::PrimitiveTag::Bool,
            crate::MirUnaryOp::Negate(kind) => kind.primitive_tag(),
        })),
        MirStatementKind::Binary { operation, .. } => Some(scalar_fact(match operation {
            crate::MirBinaryOp::Numeric { kind, .. } => kind.primitive_tag(),
            crate::MirBinaryOp::Compare { .. } => vela_common::PrimitiveTag::Bool,
        })),
        MirStatementKind::DynamicBinary {
            operation,
            left,
            right,
        } => dynamic_binary_fact(state, *operation, left, right),
        MirStatementKind::ContextualNumericBinary {
            operation, value, ..
        } => match operation {
            crate::MirContextualBinaryOp::Less
            | crate::MirContextualBinaryOp::LessEqual
            | crate::MirContextualBinaryOp::Greater
            | crate::MirContextualBinaryOp::GreaterEqual => {
                Some(scalar_fact(vela_common::PrimitiveTag::Bool))
            }
            _ => operand_fact(state, value).filter(|fact| {
                matches!(fact.value_type, MirValueType::Primitive(tag) if tag.numeric_tag().is_some())
            }),
        },
        MirStatementKind::IdentityCompare { .. }
        | MirStatementKind::Assign(MirRvalue::Truthy { .. })
        | MirStatementKind::Assign(MirRvalue::IsMissing { .. })
        | MirStatementKind::Assign(MirRvalue::PatternPredicate(_)) => {
            Some(scalar_fact(vela_common::PrimitiveTag::Bool))
        }
        MirStatementKind::Allocate(aggregate) => aggregate_fact(program, state, aggregate),
        MirStatementKind::ReadField { receiver, target } => {
            field_fact(program, state, receiver, target)
        }
        MirStatementKind::Index(crate::MirIndexOperation::Read { receiver, .. }) => {
            operand_fact(state, receiver)
                .and_then(|fact| fact.shape)
                .and_then(|shape| match shape {
                    MirShapeFact::Array(element) => element.as_deref().cloned(),
                    MirShapeFact::Record(_) | MirShapeFact::Variant(_) => None,
                })
        }
        MirStatementKind::Iterator(crate::MirIteratorOperation::Create { iterable }) => {
            let item =
                operand_fact(state, iterable).and_then(|fact| match (fact.shape, fact.family) {
                    (Some(MirShapeFact::Array(item)), _) => item,
                    (_, Some(MirFamilyFact::Iterator(item))) => item,
                    _ => None,
                });
            Some(MirValueFact {
                value_type: MirValueType::Iterator,
                immediate: None,
                constant_provenance: None,
                shape: None,
                family: Some(MirFamilyFact::Iterator(item)),
            })
        }
        MirStatementKind::Call(crate::MirCall::CallableValue { callee, .. }) => {
            let declared = declared_fact(function, destination);
            if declared
                .as_ref()
                .is_some_and(|fact| fact.value_type == MirValueType::Unit)
            {
                Some(dynamic_fact())
            } else {
                operand_fact(state, callee)
                    .and_then(|fact| match fact.family {
                        Some(MirFamilyFact::Callable { return_fact, .. }) => {
                            return_fact.map(|fact| *fact)
                        }
                        _ => None,
                    })
                    .or(declared)
                    .or_else(|| Some(dynamic_fact()))
            }
        }
        MirStatementKind::Call(
            crate::MirCall::DynamicCallable { .. } | crate::MirCall::DynamicMethod { .. },
        ) => Some(dynamic_fact()),
        _ => declared_fact(function, destination),
    }
    .or_else(|| declared_fact(function, destination));
    if let Some(fact) = fact {
        state.insert(destination, fact);
    } else {
        state.remove(&destination);
    }
}

fn dynamic_fact() -> MirValueFact {
    MirValueFact {
        value_type: MirValueType::Dynamic,
        immediate: None,
        constant_provenance: None,
        shape: None,
        family: None,
    }
}

fn scalar_fact(tag: vela_common::PrimitiveTag) -> MirValueFact {
    MirValueFact {
        value_type: MirValueType::Primitive(tag),
        immediate: None,
        constant_provenance: None,
        shape: None,
        family: None,
    }
}

fn dynamic_binary_fact(
    state: &BTreeMap<MirLiveValue, MirValueFact>,
    operation: crate::MirDynamicBinaryOp,
    left: &MirOperand,
    right: &MirOperand,
) -> Option<MirValueFact> {
    if matches!(
        operation,
        crate::MirDynamicBinaryOp::Equal
            | crate::MirDynamicBinaryOp::NotEqual
            | crate::MirDynamicBinaryOp::Less
            | crate::MirDynamicBinaryOp::LessEqual
            | crate::MirDynamicBinaryOp::Greater
            | crate::MirDynamicBinaryOp::GreaterEqual
    ) {
        return Some(scalar_fact(vela_common::PrimitiveTag::Bool));
    }
    let left = operand_fact(state, left)?;
    let right = operand_fact(state, right)?;
    (left.value_type == right.value_type
        && matches!(left.value_type, MirValueType::Primitive(tag) if tag.numeric_tag().is_some()))
    .then_some(MirValueFact {
        immediate: None,
        constant_provenance: None,
        ..left
    })
}

fn aggregate_fact(
    program: &MirProgram,
    state: &BTreeMap<MirLiveValue, MirValueFact>,
    aggregate: &MirAggregate,
) -> Option<MirValueFact> {
    if let MirAggregate::Map(entries) = aggregate {
        let values = entries
            .iter()
            .map(|(_, value)| operand_fact(state, value))
            .collect::<Vec<_>>();
        let value = meet_optional_facts(&values);
        let key = MirValueFact {
            value_type: MirValueType::Primitive(vela_common::PrimitiveTag::String),
            immediate: None,
            constant_provenance: None,
            shape: None,
            family: None,
        };
        let entry = MirValueFact {
            value_type: MirValueType::Dynamic,
            immediate: None,
            constant_provenance: None,
            shape: Some(MirShapeFact::Record(BTreeMap::from([
                (
                    "key".to_owned(),
                    (MirShapeFieldIdentity::Ordinal(0), Some(Box::new(key))),
                ),
                (
                    "value".to_owned(),
                    (MirShapeFieldIdentity::Ordinal(1), value.map(Box::new)),
                ),
            ]))),
            family: None,
        };
        return Some(MirValueFact {
            value_type: MirValueType::Dynamic,
            immediate: None,
            constant_provenance: None,
            shape: None,
            family: Some(MirFamilyFact::Iterator(Some(Box::new(entry)))),
        });
    }
    let (value_type, shape) = match aggregate {
        MirAggregate::Array(values) => {
            let facts = values
                .iter()
                .map(|value| operand_fact(state, value))
                .collect::<Vec<_>>();
            let common = meet_optional_facts(&facts);
            (
                MirValueType::Dynamic,
                MirShapeFact::Array(common.map(Box::new)),
            )
        }
        MirAggregate::Record {
            type_id, fields, ..
        } => {
            let fields = fields
                .iter()
                .filter_map(|(field, value)| {
                    let name = program.targets().field(*field)?.name.clone();
                    Some((
                        name,
                        (
                            MirShapeFieldIdentity::Stable(*field),
                            operand_fact(state, value).map(Box::new),
                        ),
                    ))
                })
                .collect();
            (
                MirValueType::ScriptType {
                    type_id: *type_id,
                    shape: program.targets().type_descriptor(*type_id)?.shape?,
                },
                MirShapeFact::Record(fields),
            )
        }
        MirAggregate::Map(_) => unreachable!("map facts return above"),
        MirAggregate::DynamicRecord { fields, .. } => (
            MirValueType::Dynamic,
            MirShapeFact::Record(named_shapes(state, fields)),
        ),
        MirAggregate::Enum {
            type_id, fields, ..
        } => {
            let fields = fields
                .iter()
                .filter_map(|(field, value)| {
                    let name = program.targets().field(*field)?.name.clone();
                    Some((
                        name,
                        (
                            MirShapeFieldIdentity::Stable(*field),
                            operand_fact(state, value).map(Box::new),
                        ),
                    ))
                })
                .collect();
            (MirValueType::Enum(*type_id), MirShapeFact::Variant(fields))
        }
        MirAggregate::DynamicVariant { fields, .. } => (
            MirValueType::Dynamic,
            MirShapeFact::Variant(named_shapes(state, fields)),
        ),
        MirAggregate::Closure { function, .. } => {
            let function = program.function(*function)?;
            let return_fact = function
                .return_contract()
                .and_then(|return_| value_fact_for_contract(&return_.contract))
                .or_else(|| infer_function_return_fact(program, function))
                .or_else(|| infer_declared_return_fact(function))
                .map(Box::new);
            return Some(MirValueFact {
                value_type: MirValueType::Callable,
                immediate: None,
                constant_provenance: None,
                shape: None,
                family: Some(MirFamilyFact::Callable {
                    accepted_kinds: crate::MirCallableKindSet::CLOSURE,
                    positional_arity: Some(function.parameters().len() as u32),
                    return_fact,
                }),
            });
        }
        _ => return None,
    };
    Some(MirValueFact {
        value_type,
        immediate: None,
        constant_provenance: None,
        shape: Some(shape),
        family: None,
    })
}

fn named_shapes(
    state: &BTreeMap<MirLiveValue, MirValueFact>,
    fields: &[(String, MirOperand)],
) -> MirShapeFields {
    let mut names = fields
        .iter()
        .map(|(name, _)| name.clone())
        .collect::<Vec<_>>();
    names.sort();
    names.dedup();
    names
        .into_iter()
        .enumerate()
        .map(|(slot, name)| {
            let fact = fields
                .iter()
                .find(|(candidate, _)| candidate == &name)
                .and_then(|(_, value)| operand_fact(state, value))
                .map(Box::new);
            (
                name,
                (
                    MirShapeFieldIdentity::Ordinal(
                        u32::try_from(slot).expect("verified MIR shape field count fits u32"),
                    ),
                    fact,
                ),
            )
        })
        .collect()
}

fn field_fact(
    program: &MirProgram,
    state: &BTreeMap<MirLiveValue, MirValueFact>,
    receiver: &MirOperand,
    target: &MirFieldTarget,
) -> Option<MirValueFact> {
    let name = match target {
        MirFieldTarget::DynamicRecord { name } | MirFieldTarget::DynamicVariant { name } => name,
        MirFieldTarget::RecordSlot { field, .. } | MirFieldTarget::VariantSlot { field, .. } => {
            let contract = program.targets().field(*field)?.contract.as_ref()?;
            let (value_type, family) = contract_fact(contract)?;
            return Some(MirValueFact {
                value_type,
                immediate: None,
                constant_provenance: None,
                shape: None,
                family,
            });
        }
    };
    let shape = operand_fact(state, receiver)?.shape?;
    let nested = match shape {
        MirShapeFact::Record(fields) | MirShapeFact::Variant(fields) => {
            fields.get(name)?.1.as_deref().cloned()
        }
        MirShapeFact::Array(_) => return None,
    };
    nested.or(Some(dynamic_fact()))
}

fn declared_fact(function: &MirFunction, value: MirLiveValue) -> Option<MirValueFact> {
    let value_type = match value {
        MirLiveValue::Local(local) => function.local(local)?.value_type,
        MirLiveValue::Temp(temp) => function.temp(temp)?.value_type,
    };
    (value_type != MirValueType::Dynamic).then_some(MirValueFact {
        value_type,
        immediate: None,
        constant_provenance: None,
        shape: None,
        family: match value_type {
            MirValueType::Tuple(arity) => Some(MirFamilyFact::Tuple(arity)),
            MirValueType::Callable => Some(MirFamilyFact::Callable {
                accepted_kinds: crate::MirCallableKindSet::FUNCTION,
                positional_arity: None,
                return_fact: None,
            }),
            MirValueType::Iterator => Some(MirFamilyFact::Iterator(None)),
            _ => None,
        },
    })
}

fn operand_fact(
    state: &BTreeMap<MirLiveValue, MirValueFact>,
    operand: &MirOperand,
) -> Option<MirValueFact> {
    match operand {
        MirOperand::Immediate(value) => Some(MirValueFact {
            value_type: value.value_type(),
            immediate: Some(*value),
            constant_provenance: None,
            shape: None,
            family: None,
        }),
        _ => state.get(&operand_value(operand)?).cloned(),
    }
}

fn intersect(
    mut incoming: impl Iterator<Item = BTreeMap<MirLiveValue, MirValueFact>>,
) -> Option<BTreeMap<MirLiveValue, MirValueFact>> {
    let mut result = incoming.next()?;
    for state in incoming {
        result = result
            .into_iter()
            .filter_map(|(value, fact)| {
                meet_fact(&fact, state.get(&value)?).map(|fact| (value, fact))
            })
            .collect();
    }
    Some(result)
}

fn meet_fact(left: &MirValueFact, right: &MirValueFact) -> Option<MirValueFact> {
    (left.value_type == right.value_type).then(|| MirValueFact {
        value_type: left.value_type,
        immediate: (left.immediate == right.immediate)
            .then_some(left.immediate)
            .flatten(),
        constant_provenance: (left.constant_provenance == right.constant_provenance)
            .then_some(left.constant_provenance)
            .flatten(),
        shape: match (&left.shape, &right.shape) {
            (Some(left), Some(right)) => meet_shape(left, right),
            _ => None,
        },
        family: (left.family == right.family)
            .then(|| left.family.clone())
            .flatten(),
    })
}

fn meet_optional_facts(values: &[Option<MirValueFact>]) -> Option<MirValueFact> {
    values
        .iter()
        .map(Option::as_ref)
        .try_fold(None, |common, fact| {
            let fact = fact?;
            match common {
                Some(common) => meet_fact(&common, fact).map(Some),
                None => Some(Some(fact.clone())),
            }
        })
        .flatten()
}

fn meet_shape(left: &MirShapeFact, right: &MirShapeFact) -> Option<MirShapeFact> {
    match (left, right) {
        (MirShapeFact::Array(left), MirShapeFact::Array(right)) => Some(MirShapeFact::Array(
            meet_nested_fact(left.as_deref(), right.as_deref()).map(Box::new),
        )),
        (MirShapeFact::Record(left), MirShapeFact::Record(right)) => {
            meet_fields(left, right).map(MirShapeFact::Record)
        }
        (MirShapeFact::Variant(left), MirShapeFact::Variant(right)) => {
            meet_fields(left, right).map(MirShapeFact::Variant)
        }
        _ => None,
    }
}

fn meet_fields(left: &MirShapeFields, right: &MirShapeFields) -> Option<MirShapeFields> {
    (left.len() == right.len()).then(|| {
        left.iter()
            .map(|(name, (slot, fact))| {
                let (right_slot, right_fact) = right.get(name)?;
                (slot == right_slot).then(|| {
                    (
                        name.clone(),
                        (
                            *slot,
                            meet_nested_fact(fact.as_deref(), right_fact.as_deref()).map(Box::new),
                        ),
                    )
                })
            })
            .collect::<Option<_>>()
    })?
}

fn meet_nested_fact(
    left: Option<&MirValueFact>,
    right: Option<&MirValueFact>,
) -> Option<MirValueFact> {
    meet_fact(left?, right?)
}

fn contract_fact(contract: &MirTypeContract) -> Option<(MirValueType, Option<MirFamilyFact>)> {
    let (value_type, family) = match contract {
        MirTypeContract::Primitive(tag) => (MirValueType::Primitive(*tag), None),
        MirTypeContract::Range => (MirValueType::Range, None),
        MirTypeContract::Array(_) | MirTypeContract::Map { .. } | MirTypeContract::Set(_) => {
            return None;
        }
        MirTypeContract::Iterator(_) => {
            (MirValueType::Iterator, Some(MirFamilyFact::Iterator(None)))
        }
        MirTypeContract::Tuple(values) => (
            MirValueType::Tuple(values.len() as u32),
            Some(MirFamilyFact::Tuple(values.len() as u32)),
        ),
        MirTypeContract::Callable {
            accepted_kinds,
            positional_arity,
        } => (
            MirValueType::Callable,
            Some(MirFamilyFact::Callable {
                accepted_kinds: *accepted_kinds,
                positional_arity: *positional_arity,
                return_fact: None,
            }),
        ),
        MirTypeContract::Shape { type_id, shape } => (
            MirValueType::ScriptType {
                type_id: *type_id,
                shape: *shape,
            },
            None,
        ),
        MirTypeContract::Variant { type_id, .. } => (MirValueType::Enum(*type_id), None),
        MirTypeContract::Host(target) => (MirValueType::Host(*target), None),
        MirTypeContract::Option(_) => (MirValueType::Dynamic, Some(MirFamilyFact::Option)),
        MirTypeContract::Result { .. } => (MirValueType::Dynamic, Some(MirFamilyFact::Result)),
        MirTypeContract::Any | MirTypeContract::Definition(_) => return None,
    };
    Some((value_type, family))
}

fn value_fact_for_contract(contract: &MirTypeContract) -> Option<MirValueFact> {
    let (value_type, family) = contract_fact(contract)?;
    Some(MirValueFact {
        value_type,
        immediate: None,
        constant_provenance: None,
        shape: None,
        family,
    })
}

fn infer_function_return_fact(
    program: &MirProgram,
    function: &MirFunction,
) -> Option<MirValueFact> {
    let facts = analyze(program, function);
    let returns = function
        .blocks()
        .filter_map(|(block, data)| {
            let MirTerminatorKind::Return(Some(value)) = &data.terminator()?.kind else {
                return None;
            };
            let mut state = facts.block_entry.get(&block)?.clone();
            for statement in data.statements() {
                transfer_statement(program, function, *statement, &mut state);
            }
            let inferred = operand_fact(&state, value);
            let declared = operand_value(value).and_then(|value| declared_fact(function, value));
            match (inferred, declared) {
                (Some(inferred), Some(declared))
                    if inferred.value_type == MirValueType::Dynamic =>
                {
                    Some(declared)
                }
                (Some(inferred), _) => Some(inferred),
                (None, declared) => declared,
            }
        })
        .collect::<Vec<_>>();
    let has_non_unit = returns
        .iter()
        .any(|fact| fact.value_type != MirValueType::Unit);
    returns
        .into_iter()
        .filter(|fact| !has_non_unit || fact.value_type != MirValueType::Unit)
        .try_fold(None, |common, fact| match common {
            Some(common) => meet_fact(&common, &fact).map(Some),
            None => Some(Some(fact)),
        })
        .flatten()
}

fn infer_declared_return_fact(function: &MirFunction) -> Option<MirValueFact> {
    function
        .blocks()
        .filter_map(|(_, data)| {
            let MirTerminatorKind::Return(Some(value)) = &data.terminator()?.kind else {
                return None;
            };
            match value {
                MirOperand::Immediate(value) => Some(MirValueFact {
                    value_type: value.value_type(),
                    immediate: Some(*value),
                    constant_provenance: None,
                    shape: None,
                    family: None,
                }),
                _ => operand_value(value).and_then(|value| declared_fact(function, value)),
            }
        })
        .find(|fact| fact.value_type != MirValueType::Unit)
}

fn successors(kind: &MirTerminatorKind) -> Vec<MirBlockId> {
    match kind {
        MirTerminatorKind::Jump(target) => vec![*target],
        MirTerminatorKind::Branch {
            then_block,
            else_block,
            ..
        } => vec![*then_block, *else_block],
        MirTerminatorKind::Switch {
            cases, otherwise, ..
        } => cases
            .iter()
            .map(|case| case.target)
            .chain(std::iter::once(*otherwise))
            .collect(),
        MirTerminatorKind::GuardBranch { passed, slow, .. } => vec![*passed, *slow],
        MirTerminatorKind::TrySwitch {
            continuations,
            propagate,
            invalid,
            ..
        } => continuations
            .iter()
            .map(|continuation| continuation.block)
            .chain([*propagate, *invalid])
            .collect(),
        MirTerminatorKind::IteratorNext { next, done, .. }
        | MirTerminatorKind::RangeNext { next, done, .. } => vec![*next, *done],
        MirTerminatorKind::Return(_)
        | MirTerminatorKind::TryTypeMismatch { .. }
        | MirTerminatorKind::Unreachable => Vec::new(),
    }
}

const fn place_value(place: MirPlace) -> MirLiveValue {
    match place {
        MirPlace::Local(local) => MirLiveValue::Local(local),
        MirPlace::Temp(temp) => MirLiveValue::Temp(temp),
    }
}

const fn operand_value(operand: &MirOperand) -> Option<MirLiveValue> {
    match operand {
        MirOperand::Immediate(_) => None,
        MirOperand::Local(local) => Some(MirLiveValue::Local(*local)),
        MirOperand::Temp(temp) => Some(MirLiveValue::Temp(*temp)),
    }
}
