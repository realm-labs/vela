use std::collections::{BTreeMap, BTreeSet};

use crate::{
    MirAggregate, MirBlockId, MirCall, MirDynamicArgument, MirFormatPart, MirHostOperation,
    MirHostPathSegment, MirImmediate, MirIndexKey, MirIndexOperation, MirIteratorOperation,
    MirLocalId, MirOperand, MirPatternPredicate, MirPlace, MirReflectionOperation, MirRvalue,
    MirScriptArgument, MirSourceOrigin, MirStatementId, MirStatementKind, MirTempId,
    MirTerminatorKind, MirValueType,
};

use super::cfg::FunctionGraph;
use super::{FunctionVerifier, MirVerifyError, MirVerifyErrorKind};

pub(crate) fn operand_type(
    verifier: &FunctionVerifier<'_>,
    operand: &MirOperand,
    block: MirBlockId,
    statement: Option<MirStatementId>,
    origin: MirSourceOrigin,
) -> Result<MirValueType, MirVerifyError> {
    match operand {
        MirOperand::Immediate(value) => Ok(match value {
            MirImmediate::Unit => MirValueType::Unit,
            MirImmediate::Bool(_) => MirValueType::Primitive(vela_common::PrimitiveTag::Bool),
            MirImmediate::Char(_) => MirValueType::Primitive(vela_common::PrimitiveTag::Char),
            MirImmediate::Scalar(value) => MirValueType::Primitive(value.primitive_tag()),
        }),
        MirOperand::Local(local) => verifier
            .function
            .local(*local)
            .map(|local| local.value_type)
            .ok_or_else(|| {
                verifier.error(
                    Some(block),
                    statement,
                    origin,
                    MirVerifyErrorKind::MissingLocal(*local),
                )
            }),
        MirOperand::Temp(temp) => verifier
            .function
            .temp(*temp)
            .map(|temp| temp.value_type)
            .ok_or_else(|| {
                verifier.error(
                    Some(block),
                    statement,
                    origin,
                    MirVerifyErrorKind::MissingTemp(*temp),
                )
            }),
    }
}

pub(crate) fn verify(
    verifier: &FunctionVerifier<'_>,
    graph: &FunctionGraph,
) -> Result<(), MirVerifyError> {
    let definitions = verify_temp_definitions(verifier, graph)?;
    let initialized = compute_initialized_locals(verifier, graph);

    for block_id in graph.blocks() {
        let block = verifier
            .function
            .block(block_id)
            .expect("CFG analysis retained only existing blocks");
        let mut state = initialized[&block_id].clone();
        for (index, statement_id) in block.statements().iter().copied().enumerate() {
            let statement = verifier
                .function
                .statement(statement_id)
                .expect("CFG analysis retained only existing statements");
            visit_statement_operands(&statement.kind, |operand| {
                verify_operand_use(
                    verifier,
                    graph,
                    &definitions,
                    &state,
                    block_id,
                    Some((statement_id, index)),
                    statement.origin,
                    operand,
                )
            })?;
            if let Some(MirPlace::Local(local)) = statement.destination {
                state.insert(local);
            }
        }

        let terminator = block
            .terminator()
            .expect("CFG analysis requires a terminator");
        visit_terminator_operands(&terminator.kind, |operand| {
            verify_operand_use(
                verifier,
                graph,
                &definitions,
                &state,
                block_id,
                None,
                terminator.origin,
                operand,
            )
        })?;
        match terminator.kind {
            MirTerminatorKind::RangeNext {
                cursor, exhausted, ..
            } => {
                verify_local_use(verifier, &state, block_id, None, terminator.origin, cursor)?;
                verify_local_use(
                    verifier,
                    &state,
                    block_id,
                    None,
                    terminator.origin,
                    exhausted,
                )?;
            }
            MirTerminatorKind::Jump(_)
            | MirTerminatorKind::Branch { .. }
            | MirTerminatorKind::Switch { .. }
            | MirTerminatorKind::GuardBranch { .. }
            | MirTerminatorKind::IteratorNext { .. }
            | MirTerminatorKind::Return(_)
            | MirTerminatorKind::Fail { .. }
            | MirTerminatorKind::Unreachable => {}
        }
    }
    Ok(())
}

fn verify_temp_definitions(
    verifier: &FunctionVerifier<'_>,
    graph: &FunctionGraph,
) -> Result<BTreeMap<MirTempId, MirStatementId>, MirVerifyError> {
    let mut actual = BTreeMap::<MirTempId, Vec<MirStatementId>>::new();
    for block_id in graph.blocks() {
        let block = verifier
            .function
            .block(block_id)
            .expect("CFG analysis retained only existing blocks");
        for statement_id in block.statements().iter().copied() {
            let statement = verifier
                .function
                .statement(statement_id)
                .expect("CFG analysis retained only existing statements");
            if let Some(MirPlace::Temp(temp)) = statement.destination {
                if verifier.function.temp(temp).is_none() {
                    return Err(verifier.error(
                        Some(block_id),
                        Some(statement_id),
                        statement.origin,
                        MirVerifyErrorKind::MissingTemp(temp),
                    ));
                }
                actual.entry(temp).or_default().push(statement_id);
            }
        }
    }

    let mut definitions = BTreeMap::new();
    for (temp, record) in verifier.function.temps() {
        let values = actual.get(&temp).map_or(&[][..], Vec::as_slice);
        match values {
            [] => {
                return Err(verifier.error(
                    None,
                    None,
                    record.origin,
                    MirVerifyErrorKind::TempHasNoDefinition(temp),
                ));
            }
            [definition] => {
                if record.definition() != Some(*definition) {
                    let location = graph.statement_location(*definition);
                    return Err(verifier.error(
                        location.map(|value| value.block),
                        Some(*definition),
                        record.origin,
                        MirVerifyErrorKind::TempDefinitionMismatch {
                            temp,
                            recorded: record.definition(),
                            actual: Some(*definition),
                        },
                    ));
                }
                definitions.insert(temp, *definition);
            }
            [first, ..] => {
                let location = graph.statement_location(*first);
                return Err(verifier.error(
                    location.map(|value| value.block),
                    Some(*first),
                    record.origin,
                    MirVerifyErrorKind::TempHasMultipleDefinitions(temp),
                ));
            }
        }
    }
    Ok(definitions)
}

fn compute_initialized_locals(
    verifier: &FunctionVerifier<'_>,
    graph: &FunctionGraph,
) -> BTreeMap<MirBlockId, BTreeSet<MirLocalId>> {
    let all_locals = verifier
        .function
        .locals()
        .map(|(local, _)| local)
        .collect::<BTreeSet<_>>();
    let entry = verifier.function.entry_block();
    let entry_initialized = verifier
        .function
        .parameters()
        .iter()
        .map(|parameter| parameter.storage)
        .chain(
            verifier
                .function
                .captures()
                .iter()
                .map(|capture| capture.storage),
        )
        .collect::<BTreeSet<_>>();
    let mut block_in = graph
        .blocks()
        .map(|block| {
            (
                block,
                if block == entry {
                    entry_initialized.clone()
                } else {
                    all_locals.clone()
                },
            )
        })
        .collect::<BTreeMap<_, _>>();

    loop {
        let edge_out = graph
            .blocks()
            .map(|block| {
                let mut state = block_in[&block].clone();
                let basic_block = verifier
                    .function
                    .block(block)
                    .expect("CFG analysis retained only existing blocks");
                for statement_id in basic_block.statements() {
                    if let Some(MirPlace::Local(local)) = verifier
                        .function
                        .statement(*statement_id)
                        .expect("CFG analysis retained only existing statements")
                        .destination
                    {
                        state.insert(local);
                    }
                }
                (block, state)
            })
            .collect::<BTreeMap<_, _>>();

        let mut changed = false;
        for block in graph.blocks().filter(|block| *block != entry) {
            let mut incoming = graph.predecessors(block).map(|predecessor| {
                let mut state = edge_out[&predecessor].clone();
                add_edge_definitions(verifier, predecessor, block, &mut state);
                state
            });
            let mut next = incoming.next().unwrap_or_default();
            for values in incoming {
                next.retain(|value| values.contains(value));
            }
            if block_in[&block] != next {
                block_in.insert(block, next);
                changed = true;
            }
        }
        if !changed {
            return block_in;
        }
    }
}

fn add_edge_definitions(
    verifier: &FunctionVerifier<'_>,
    predecessor: MirBlockId,
    successor: MirBlockId,
    state: &mut BTreeSet<MirLocalId>,
) {
    let terminator = verifier
        .function
        .block(predecessor)
        .and_then(|block| block.terminator())
        .expect("CFG analysis requires a terminator");
    match terminator.kind {
        MirTerminatorKind::IteratorNext { item, next, .. }
        | MirTerminatorKind::RangeNext { item, next, .. }
            if next == successor =>
        {
            state.insert(item);
        }
        MirTerminatorKind::Jump(_)
        | MirTerminatorKind::Branch { .. }
        | MirTerminatorKind::Switch { .. }
        | MirTerminatorKind::GuardBranch { .. }
        | MirTerminatorKind::IteratorNext { .. }
        | MirTerminatorKind::RangeNext { .. }
        | MirTerminatorKind::Return(_)
        | MirTerminatorKind::Fail { .. }
        | MirTerminatorKind::Unreachable => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn verify_operand_use(
    verifier: &FunctionVerifier<'_>,
    graph: &FunctionGraph,
    definitions: &BTreeMap<MirTempId, MirStatementId>,
    initialized: &BTreeSet<MirLocalId>,
    block: MirBlockId,
    statement: Option<(MirStatementId, usize)>,
    origin: MirSourceOrigin,
    operand: &MirOperand,
) -> Result<(), MirVerifyError> {
    match operand {
        MirOperand::Immediate(_) => Ok(()),
        MirOperand::Local(local) => verify_local_use(
            verifier,
            initialized,
            block,
            statement.map(|value| value.0),
            origin,
            *local,
        ),
        MirOperand::Temp(temp) => {
            let definition = definitions.get(temp).copied().ok_or_else(|| {
                verifier.error(
                    Some(block),
                    statement.map(|value| value.0),
                    origin,
                    MirVerifyErrorKind::TempHasNoDefinition(*temp),
                )
            })?;
            let definition_location = graph
                .statement_location(definition)
                .expect("verified temp definition has a statement location");
            let dominated = if definition_location.block == block {
                statement.is_none_or(|(_, index)| definition_location.index < index)
            } else {
                graph.dominates(definition_location.block, block)
            };
            if dominated {
                Ok(())
            } else {
                Err(verifier.error(
                    Some(block),
                    statement.map(|value| value.0),
                    origin,
                    MirVerifyErrorKind::TempUseNotDominated {
                        temp: *temp,
                        definition,
                    },
                ))
            }
        }
    }
}

fn verify_local_use(
    verifier: &FunctionVerifier<'_>,
    initialized: &BTreeSet<MirLocalId>,
    block: MirBlockId,
    statement: Option<MirStatementId>,
    origin: MirSourceOrigin,
    local: MirLocalId,
) -> Result<(), MirVerifyError> {
    if verifier.function.local(local).is_none() {
        return Err(verifier.error(
            Some(block),
            statement,
            origin,
            MirVerifyErrorKind::MissingLocal(local),
        ));
    }
    if !initialized.contains(&local) {
        return Err(verifier.error(
            Some(block),
            statement,
            origin,
            MirVerifyErrorKind::LocalUseBeforeInitialization(local),
        ));
    }
    Ok(())
}

pub(crate) fn visit_statement_operands(
    kind: &MirStatementKind,
    mut visitor: impl FnMut(&MirOperand) -> Result<(), MirVerifyError>,
) -> Result<(), MirVerifyError> {
    match kind {
        MirStatementKind::Assign(value) => visit_rvalue(value, &mut visitor)?,
        MirStatementKind::Unary { operand, .. }
        | MirStatementKind::DynamicUnary { operand, .. }
        | MirStatementKind::TupleField { tuple: operand, .. }
        | MirStatementKind::GuardTrap { value: operand, .. } => visitor(operand)?,
        MirStatementKind::Binary { left, right, .. }
        | MirStatementKind::DynamicBinary { left, right, .. }
        | MirStatementKind::IdentityCompare { left, right, .. } => {
            visitor(left)?;
            visitor(right)?;
        }
        MirStatementKind::ContextualNumericBinary { value, .. } => visitor(value)?,
        MirStatementKind::ReadField { receiver, .. } => visitor(receiver)?,
        MirStatementKind::WriteField {
            receiver, value, ..
        } => {
            visitor(receiver)?;
            visitor(value)?;
        }
        MirStatementKind::Index(operation) => match operation {
            MirIndexOperation::Read { receiver, index } => {
                visitor(receiver)?;
                visit_index_key(index, &mut visitor)?;
            }
            MirIndexOperation::Write {
                receiver,
                index,
                value,
            } => {
                visitor(receiver)?;
                visit_index_key(index, &mut visitor)?;
                visitor(value)?;
            }
        },
        MirStatementKind::Global(_) | MirStatementKind::MaterializeConstant(_) => {}
        MirStatementKind::Allocate(aggregate) => visit_aggregate(aggregate, &mut visitor)?,
        MirStatementKind::FormatString { parts } => {
            for part in parts {
                if let MirFormatPart::Value(value) = part {
                    visitor(value)?;
                }
            }
        }
        MirStatementKind::MakeRange { start, end, .. } => {
            visitor(start)?;
            visitor(end)?;
        }
        MirStatementKind::Call(call) => visit_call(call, &mut visitor)?,
        MirStatementKind::Host(operation) => visit_host(operation, &mut visitor)?,
        MirStatementKind::Reflect(operation) => match operation {
            MirReflectionOperation::Read { target, member, .. } => {
                visitor(target)?;
                visitor(member)?;
            }
            MirReflectionOperation::Write {
                target,
                member,
                value,
                ..
            } => {
                visitor(target)?;
                visitor(member)?;
                visitor(value)?;
            }
            MirReflectionOperation::Call { target, tail, .. } => {
                visitor(target)?;
                for value in tail {
                    visitor(value)?;
                }
            }
        },
        MirStatementKind::Iterator(MirIteratorOperation::Create { iterable }) => visitor(iterable)?,
    }
    Ok(())
}

pub(crate) fn visit_terminator_operands(
    kind: &MirTerminatorKind,
    mut visitor: impl FnMut(&MirOperand) -> Result<(), MirVerifyError>,
) -> Result<(), MirVerifyError> {
    match kind {
        MirTerminatorKind::Branch { condition, .. } => visitor(condition)?,
        MirTerminatorKind::Switch { discriminant, .. } => visitor(discriminant)?,
        MirTerminatorKind::GuardBranch { value, .. } => visitor(value)?,
        MirTerminatorKind::IteratorNext { iterator, .. } => visitor(iterator)?,
        MirTerminatorKind::RangeNext { end, .. } => visitor(end)?,
        MirTerminatorKind::Return(Some(value)) => visitor(value)?,
        MirTerminatorKind::Jump(_)
        | MirTerminatorKind::Return(None)
        | MirTerminatorKind::Fail { .. }
        | MirTerminatorKind::Unreachable => {}
    }
    Ok(())
}

fn visit_rvalue(
    value: &MirRvalue,
    visitor: &mut impl FnMut(&MirOperand) -> Result<(), MirVerifyError>,
) -> Result<(), MirVerifyError> {
    let operand = match value {
        MirRvalue::Use(value) | MirRvalue::Truthy { value } | MirRvalue::IsMissing { value } => {
            value
        }
        MirRvalue::PatternPredicate(
            MirPatternPredicate::TupleArity { value, .. }
            | MirPatternPredicate::RecordShape { value, .. }
            | MirPatternPredicate::VariantShape { value, .. }
            | MirPatternPredicate::DynamicRecord { value, .. }
            | MirPatternPredicate::DynamicVariant { value, .. },
        ) => value,
    };
    visitor(operand)
}

fn visit_index_key(
    key: &MirIndexKey,
    visitor: &mut impl FnMut(&MirOperand) -> Result<(), MirVerifyError>,
) -> Result<(), MirVerifyError> {
    if let MirIndexKey::Value(value) = key {
        visitor(value)?;
    }
    Ok(())
}

fn visit_aggregate(
    aggregate: &MirAggregate,
    visitor: &mut impl FnMut(&MirOperand) -> Result<(), MirVerifyError>,
) -> Result<(), MirVerifyError> {
    match aggregate {
        MirAggregate::Tuple(values) | MirAggregate::Array(values) => {
            for value in values {
                visitor(value)?;
            }
        }
        MirAggregate::Map(values) => {
            for (_, value) in values {
                visitor(value)?;
            }
        }
        MirAggregate::SetFromArray { source } => visitor(source)?,
        MirAggregate::Record { fields, .. } | MirAggregate::Enum { fields, .. } => {
            for (_, value) in fields {
                visitor(value)?;
            }
        }
        MirAggregate::DynamicRecord { fields, .. }
        | MirAggregate::DynamicVariant { fields, .. } => {
            for (_, value) in fields {
                visitor(value)?;
            }
        }
        MirAggregate::Closure { captures, .. } => {
            for value in captures {
                visitor(value)?;
            }
        }
    }
    Ok(())
}

fn visit_call(
    call: &MirCall,
    visitor: &mut impl FnMut(&MirOperand) -> Result<(), MirVerifyError>,
) -> Result<(), MirVerifyError> {
    match call {
        MirCall::ScriptFunction { arguments, .. } => visit_script_args(arguments, visitor)?,
        MirCall::ScriptMethod {
            receiver,
            arguments,
            ..
        } => {
            visitor(receiver)?;
            visit_script_args(arguments, visitor)?;
        }
        MirCall::CallableValue { callee, arguments } => {
            visitor(callee)?;
            for value in arguments {
                visitor(value)?;
            }
        }
        MirCall::DynamicCallable { callee, arguments } => {
            visitor(callee)?;
            visit_dynamic_args(arguments, visitor)?;
        }
        MirCall::NativeFunction { arguments, .. } | MirCall::StdlibFunction { arguments, .. } => {
            for value in arguments {
                visitor(value)?;
            }
        }
        MirCall::ValueMethod {
            receiver,
            arguments,
            ..
        } => {
            visitor(receiver)?;
            for value in arguments {
                visitor(value)?;
            }
        }
        MirCall::DynamicMethod {
            receiver,
            arguments,
            ..
        } => {
            visitor(receiver)?;
            visit_dynamic_args(arguments, visitor)?;
        }
    }
    Ok(())
}

fn visit_script_args(
    arguments: &[MirScriptArgument],
    visitor: &mut impl FnMut(&MirOperand) -> Result<(), MirVerifyError>,
) -> Result<(), MirVerifyError> {
    for value in arguments
        .iter()
        .filter_map(|argument| argument.value.as_ref())
    {
        visitor(value)?;
    }
    Ok(())
}

fn visit_dynamic_args(
    arguments: &[MirDynamicArgument],
    visitor: &mut impl FnMut(&MirOperand) -> Result<(), MirVerifyError>,
) -> Result<(), MirVerifyError> {
    for argument in arguments {
        visitor(&argument.value)?;
    }
    Ok(())
}

fn visit_host(
    operation: &MirHostOperation,
    visitor: &mut impl FnMut(&MirOperand) -> Result<(), MirVerifyError>,
) -> Result<(), MirVerifyError> {
    let (root, path) = match operation {
        MirHostOperation::Read { root, path }
        | MirHostOperation::Remove { root, path }
        | MirHostOperation::Call { root, path, .. } => (root, path),
        MirHostOperation::Write {
            root, path, value, ..
        }
        | MirHostOperation::Mutate {
            root, path, value, ..
        } => {
            visitor(value)?;
            (root, path)
        }
    };
    visitor(root)?;
    for segment in &path.segments {
        if let MirHostPathSegment::Index { value, .. } | MirHostPathSegment::Key { value, .. } =
            segment
        {
            visitor(value)?;
        }
    }
    if let MirHostOperation::Call { arguments, .. } = operation {
        for argument in arguments {
            visitor(argument)?;
        }
    }
    Ok(())
}
