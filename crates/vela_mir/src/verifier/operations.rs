use std::collections::BTreeSet;

mod calls;
mod host;
mod support;
mod targets;

use calls::verify_call;
use host::verify_host;
use support::{
    arity_accepts, bad_target, constant_type, destination_accepts, destination_contract, error,
    function_call_target, function_target, integer_or_dynamic, method_target, missing_target,
    operand_is, place_type, require_local, require_type, rvalue_type, satisfies_contract,
    switch_case, type_error, verify_abi, verify_contract, verify_descendant_origin, verify_origin,
    verify_value_type,
};
use targets::{
    verify_aggregate, verify_field_operation, verify_guard_use, verify_predicate_targets,
};

use vela_common::{CallableAsyncness, PrimitiveTag};

use crate::operations::MirDestinationRequirement;
use crate::{
    CompileFunctionClass, CompileMethodClass, CompileStateStorage, MirAwaitOperation, MirBinaryOp,
    MirCall, MirConstantProvenance, MirContextualBinaryOp, MirDynamicBinaryOp, MirEffect,
    MirGuardAssumption, MirIteratorOperation, MirOperand, MirPlace, MirReflectionOperation,
    MirRvalue, MirSourceNode, MirSourceOrigin, MirStateOperation, MirStatementId, MirStatementKind,
    MirTaskOperation, MirTerminatorKind, MirTypeContract, MirUnaryOp, MirValueType,
};

use super::cfg::FunctionGraph;
use super::dataflow::{visit_statement_operands, visit_terminator_operands};
use super::{
    FunctionVerifier, MirDestinationExpectation, MirVerifyError, MirVerifyErrorKind,
    MirVerifyTarget,
};

pub(crate) fn verify_function_metadata(
    verifier: &FunctionVerifier<'_>,
) -> Result<(), MirVerifyError> {
    let function = verifier.function;
    if function.origin().body != Some(function.body())
        || function.origin().node != MirSourceNode::Body(function.body())
    {
        return Err(error(
            verifier,
            None,
            None,
            function.origin(),
            MirVerifyErrorKind::InvalidSourceOrigin(
                "function origin is not its owning HIR body".to_owned(),
            ),
        ));
    }
    let reservation = verifier
        .program
        .reservation(verifier.function_id)
        .expect("a defined function retains its reservation");
    if reservation.body() != function.body()
        || reservation.owner() != function.owner()
        || reservation.origin() != function.origin()
    {
        return Err(error(
            verifier,
            None,
            None,
            function.origin(),
            MirVerifyErrorKind::InvalidFunctionMetadata(
                "definition disagrees with its reservation".to_owned(),
            ),
        ));
    }

    let descriptor = match function.owner() {
        crate::MirFunctionOwner::Function(id) => {
            let descriptor = function_target(verifier, *id, function.origin())?;
            if descriptor.class != CompileFunctionClass::Script {
                return Err(bad_target(
                    verifier,
                    function.origin(),
                    MirVerifyTarget::Function(*id),
                    "MIR owner is not a script function",
                ));
            }
            Some(descriptor)
        }
        crate::MirFunctionOwner::Method(target) => {
            let descriptor = function_target(verifier, target.function, function.origin())?;
            let method = method_target(verifier, target.owner, target.method, function.origin())?;
            require_type(verifier, target.owner, function.origin())?;
            if descriptor.class != CompileFunctionClass::Script
                || !matches!(
                    &method.class,
                    CompileMethodClass::Script { executable, code_symbol, .. }
                        if executable == target && code_symbol == function.code_symbol()
                )
            {
                return Err(bad_target(
                    verifier,
                    function.origin(),
                    MirVerifyTarget::Method {
                        owner: target.owner,
                        method: target.method,
                    },
                    "MIR method owner disagrees with its script descriptor",
                ));
            }
            Some(descriptor)
        }
        crate::MirFunctionOwner::Lambda { parent, .. } => {
            if *parent == verifier.function_id || verifier.program.function(*parent).is_none() {
                return Err(error(
                    verifier,
                    None,
                    None,
                    function.origin(),
                    MirVerifyErrorKind::MissingTarget(MirVerifyTarget::MirFunction(*parent)),
                ));
            }
            None
        }
    };
    if let Some(descriptor) = descriptor {
        verify_abi(verifier, descriptor)?;
    }

    let mut initial = BTreeSet::new();
    for parameter in function.parameters() {
        let local = function.local(parameter.storage).ok_or_else(|| {
            error(
                verifier,
                None,
                None,
                parameter.origin,
                MirVerifyErrorKind::MissingLocal(parameter.storage),
            )
        })?;
        if !matches!(local.kind, crate::MirLocalKind::Script(id) if id == parameter.hir_local)
            || !initial.insert(parameter.storage)
        {
            return Err(error(
                verifier,
                None,
                None,
                parameter.origin,
                MirVerifyErrorKind::InvalidFunctionMetadata(
                    "parameter storage is not a unique matching script local".to_owned(),
                ),
            ));
        }
        verify_origin(verifier, None, None, parameter.origin)?;
        if let Some(contract) = &parameter.contract {
            verify_contract(verifier, parameter.origin, contract)?;
        }
    }
    for capture in function.captures() {
        let local = function.local(capture.storage).ok_or_else(|| {
            error(
                verifier,
                None,
                None,
                capture.origin,
                MirVerifyErrorKind::MissingLocal(capture.storage),
            )
        })?;
        if !matches!(local.kind, crate::MirLocalKind::Script(id) if id == capture.source_local)
            || !initial.insert(capture.storage)
        {
            return Err(error(
                verifier,
                None,
                None,
                capture.origin,
                MirVerifyErrorKind::InvalidFunctionMetadata(
                    "capture storage is not a unique matching script local".to_owned(),
                ),
            ));
        }
        verify_descendant_origin(verifier, capture.origin)?;
    }
    for (local_id, local) in function.locals() {
        if function
            .captures()
            .iter()
            .any(|capture| capture.storage == local_id)
        {
            verify_descendant_origin(verifier, local.origin)?;
        } else {
            verify_origin(verifier, None, None, local.origin)?;
        }
        verify_value_type(verifier, local.origin, local.value_type)?;
    }
    for (_, temp) in function.temps() {
        verify_origin(verifier, None, None, temp.origin)?;
        verify_value_type(verifier, temp.origin, temp.value_type)?;
    }
    if let Some(value) = function.return_contract() {
        verify_origin(verifier, None, None, value.origin)?;
        verify_contract(verifier, value.origin, &value.contract)?;
    }
    for (_, guard) in function.guards() {
        verify_origin(verifier, None, None, guard.origin)?;
        if let MirGuardAssumption::Type(contract) = &guard.assumption {
            verify_contract(verifier, guard.origin, contract)?;
        }
    }
    for (_, safepoint) in function.safepoints() {
        verify_origin(verifier, None, None, safepoint.origin)?;
        for live in &safepoint.live_values {
            match live {
                crate::MirLiveValue::Local(id) if function.local(*id).is_none() => {
                    return Err(error(
                        verifier,
                        None,
                        None,
                        safepoint.origin,
                        MirVerifyErrorKind::MissingLocal(*id),
                    ));
                }
                crate::MirLiveValue::Temp(id) if function.temp(*id).is_none() => {
                    return Err(error(
                        verifier,
                        None,
                        None,
                        safepoint.origin,
                        MirVerifyErrorKind::MissingTemp(*id),
                    ));
                }
                crate::MirLiveValue::Local(_) | crate::MirLiveValue::Temp(_) => {}
            }
        }
    }
    let mut used_safepoints = BTreeSet::new();
    for (block_id, block) in function.blocks() {
        for statement_id in block.statements() {
            let statement = function
                .statement(*statement_id)
                .expect("function metadata is checked before CFG placement");
            if let Some(safepoint) = statement.safepoint
                && !used_safepoints.insert(safepoint)
            {
                return Err(error(
                    verifier,
                    Some(block_id),
                    Some(*statement_id),
                    statement.origin,
                    MirVerifyErrorKind::DuplicateSafepointUse { safepoint },
                ));
            }
        }
        if let Some(terminator) = block.terminator()
            && let Some(safepoint) = terminator.safepoint
            && !used_safepoints.insert(safepoint)
        {
            return Err(error(
                verifier,
                Some(block_id),
                None,
                terminator.origin,
                MirVerifyErrorKind::DuplicateSafepointUse { safepoint },
            ));
        }
    }
    if let Some((safepoint, record)) = function
        .safepoints()
        .find(|(safepoint, _)| !used_safepoints.contains(safepoint))
    {
        return Err(error(
            verifier,
            None,
            None,
            record.origin,
            MirVerifyErrorKind::OrphanSafepoint(safepoint),
        ));
    }
    for (_, debug) in function.debug_locals() {
        if debug.kind == crate::DebugLocalKind::Capture {
            verify_descendant_origin(verifier, debug.origin)?;
        } else {
            verify_origin(verifier, None, None, debug.origin)?;
        }
        let local = function.local(debug.storage).ok_or_else(|| {
            error(
                verifier,
                None,
                None,
                debug.origin,
                MirVerifyErrorKind::MissingLocal(debug.storage),
            )
        })?;
        let expected = match local.kind {
            crate::MirLocalKind::Script(id) => Some(id),
            crate::MirLocalKind::Synthetic => None,
        };
        if debug.hir_local != expected
            || debug
                .live_region
                .blocks
                .iter()
                .any(|block| function.block(*block).is_none())
        {
            return Err(error(
                verifier,
                None,
                None,
                debug.origin,
                MirVerifyErrorKind::InvalidDebugMetadata(
                    "debug local storage or live region is invalid".to_owned(),
                ),
            ));
        }
    }
    Ok(())
}

pub(crate) fn verify_operations(
    verifier: &FunctionVerifier<'_>,
    graph: &FunctionGraph,
) -> Result<(), MirVerifyError> {
    for block_id in graph.blocks() {
        let block = verifier
            .function
            .block(block_id)
            .expect("CFG analysis retained an existing block");
        for statement_id in block.statements().iter().copied() {
            let statement = verifier
                .function
                .statement(statement_id)
                .expect("CFG analysis retained an existing statement");
            verify_origin(
                verifier,
                Some(block_id),
                Some(statement_id),
                statement.origin,
            )?;
            verify_statement_metadata(verifier, block_id, statement_id, statement)?;
            visit_statement_operands(&statement.kind, |operand| {
                verifier
                    .operand_type(operand, block_id, Some(statement_id), statement.origin)
                    .map(|_| ())
            })?;
            verify_statement_kind(verifier, block_id, statement_id, statement)?;
        }
        let terminator = block
            .terminator()
            .expect("CFG analysis retained a terminated block");
        verify_origin(verifier, Some(block_id), None, terminator.origin)?;
        let await_boundary = matches!(terminator.kind, MirTerminatorKind::AwaitCall { .. });
        verify_effect_and_safepoint(
            verifier,
            block_id,
            None,
            terminator.origin,
            terminator.effect,
            terminator.kind.minimum_effect(),
            terminator.safepoint,
            await_boundary || terminator.effect.requires_safepoint(),
        )?;
        visit_terminator_operands(&terminator.kind, |operand| {
            verifier
                .operand_type(operand, block_id, None, terminator.origin)
                .map(|_| ())
        })?;
        verify_terminator(verifier, block_id, terminator)?;
    }
    Ok(())
}

fn verify_statement_metadata(
    verifier: &FunctionVerifier<'_>,
    block: crate::MirBlockId,
    id: crate::MirStatementId,
    statement: &crate::MirStatement,
) -> Result<(), MirVerifyError> {
    let expectation = match statement.kind.destination_requirement() {
        MirDestinationRequirement::Required => MirDestinationExpectation::Required,
        MirDestinationRequirement::Forbidden => MirDestinationExpectation::Forbidden,
    };
    if matches!(expectation, MirDestinationExpectation::Required) != statement.destination.is_some()
    {
        return Err(error(
            verifier,
            Some(block),
            Some(id),
            statement.origin,
            MirVerifyErrorKind::InvalidDestination {
                expected: expectation,
            },
        ));
    }
    if let Some(place) = statement.destination {
        place_type(verifier, block, id, statement.origin, place)?;
    }
    verify_effect_and_safepoint(
        verifier,
        block,
        Some(id),
        statement.origin,
        statement.effect,
        statement.kind.minimum_effect(),
        statement.safepoint,
        statement.effect.requires_safepoint() || statement.kind.requires_safepoint(),
    )?;
    if !statement.kind.has_valid_call_contract() {
        return Err(error(
            verifier,
            Some(block),
            Some(id),
            statement.origin,
            MirVerifyErrorKind::InvalidCallContract(
                "argument placement violates the MIR call contract".to_owned(),
            ),
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn verify_effect_and_safepoint(
    verifier: &FunctionVerifier<'_>,
    block: crate::MirBlockId,
    statement: Option<crate::MirStatementId>,
    origin: MirSourceOrigin,
    actual: MirEffect,
    required: MirEffect,
    safepoint: Option<crate::MirSafepointId>,
    requires_safepoint: bool,
) -> Result<(), MirVerifyError> {
    if !actual.contains(required) {
        return Err(error(
            verifier,
            Some(block),
            statement,
            origin,
            MirVerifyErrorKind::IncompleteEffect { required, actual },
        ));
    }
    if requires_safepoint != safepoint.is_some() {
        return Err(error(
            verifier,
            Some(block),
            statement,
            origin,
            if requires_safepoint {
                MirVerifyErrorKind::MissingRequiredSafepoint
            } else {
                MirVerifyErrorKind::UnexpectedSafepoint
            },
        ));
    }
    if let Some(id) = safepoint {
        let record = verifier.function.safepoint(id).ok_or_else(|| {
            error(
                verifier,
                Some(block),
                statement,
                origin,
                MirVerifyErrorKind::MissingSafepoint(id),
            )
        })?;
        if record.origin != origin {
            return Err(error(
                verifier,
                Some(block),
                statement,
                origin,
                MirVerifyErrorKind::SafepointOriginMismatch { safepoint: id },
            ));
        }
    }
    Ok(())
}

fn verify_statement_kind(
    verifier: &FunctionVerifier<'_>,
    block: crate::MirBlockId,
    id: crate::MirStatementId,
    statement: &crate::MirStatement,
) -> Result<(), MirVerifyError> {
    let destination = statement
        .destination
        .map(|place| place_type(verifier, block, id, statement.origin, place))
        .transpose()?;
    match &statement.kind {
        MirStatementKind::Assign(value) => {
            if let MirRvalue::Constant { provenance, .. } = value {
                let destination_is_temp = matches!(statement.destination, Some(MirPlace::Temp(_)));
                let origin_matches = matches!(
                    (provenance, statement.origin.node),
                    (
                        MirConstantProvenance::Literal
                            | MirConstantProvenance::FoldedLiteral
                            | MirConstantProvenance::EvaluatedConstant,
                        MirSourceNode::Expression(_)
                    ) | (
                        MirConstantProvenance::PatternLiteral,
                        MirSourceNode::Pattern(_)
                    )
                );
                if !destination_is_temp || !origin_matches {
                    return Err(error(
                        verifier,
                        Some(block),
                        Some(id),
                        statement.origin,
                        MirVerifyErrorKind::InvalidConstantDefinition(
                            "constant provenance requires one temp definition at its source node"
                                .to_owned(),
                        ),
                    ));
                }
            }
            let result = rvalue_type(verifier, block, id, statement.origin, value)?;
            destination_accepts(verifier, block, id, statement.origin, destination, result)?;
            verify_predicate_targets(verifier, statement.origin, value)?;
        }
        MirStatementKind::Unary { operation, operand } => {
            let expected = match operation {
                MirUnaryOp::NotBool => MirValueType::Primitive(PrimitiveTag::Bool),
                MirUnaryOp::Negate(tag) => MirValueType::Primitive(tag.primitive_tag()),
            };
            operand_is(
                verifier,
                block,
                Some(id),
                statement.origin,
                operand,
                expected,
            )?;
            destination_accepts(verifier, block, id, statement.origin, destination, expected)?;
        }
        MirStatementKind::Binary {
            operation,
            left,
            right,
        } => {
            let (input, output) = match operation {
                MirBinaryOp::Numeric { kind, .. } => {
                    let value = MirValueType::Primitive(kind.primitive_tag());
                    (value, value)
                }
                MirBinaryOp::Compare { kind, .. } => (
                    MirValueType::Primitive(*kind),
                    MirValueType::Primitive(PrimitiveTag::Bool),
                ),
            };
            operand_is(verifier, block, Some(id), statement.origin, left, input)?;
            operand_is(verifier, block, Some(id), statement.origin, right, input)?;
            destination_accepts(verifier, block, id, statement.origin, destination, output)?;
        }
        MirStatementKind::DynamicBinary { operation, .. } => {
            if matches!(
                operation,
                MirDynamicBinaryOp::Equal
                    | MirDynamicBinaryOp::NotEqual
                    | MirDynamicBinaryOp::Less
                    | MirDynamicBinaryOp::LessEqual
                    | MirDynamicBinaryOp::Greater
                    | MirDynamicBinaryOp::GreaterEqual
            ) {
                destination_accepts(
                    verifier,
                    block,
                    id,
                    statement.origin,
                    destination,
                    MirValueType::Primitive(PrimitiveTag::Bool),
                )?;
            }
        }
        MirStatementKind::ContextualNumericBinary { operation, .. } => {
            if matches!(
                operation,
                MirContextualBinaryOp::Less
                    | MirContextualBinaryOp::LessEqual
                    | MirContextualBinaryOp::Greater
                    | MirContextualBinaryOp::GreaterEqual
            ) {
                destination_accepts(
                    verifier,
                    block,
                    id,
                    statement.origin,
                    destination,
                    MirValueType::Primitive(PrimitiveTag::Bool),
                )?;
            }
        }
        MirStatementKind::IdentityCompare { .. } => destination_accepts(
            verifier,
            block,
            id,
            statement.origin,
            destination,
            MirValueType::Primitive(PrimitiveTag::Bool),
        )?,
        MirStatementKind::TupleField { tuple, index } => {
            let actual = verifier.operand_type(tuple, block, Some(id), statement.origin)?;
            if !matches!(actual, MirValueType::Dynamic | MirValueType::Tuple(_))
                || matches!(actual, MirValueType::Tuple(arity) if *index >= arity)
            {
                return Err(type_error(
                    verifier,
                    block,
                    Some(id),
                    statement.origin,
                    "tuple projection receiver/index",
                    actual,
                ));
            }
        }
        MirStatementKind::ReadField { receiver, target } => verify_field_operation(
            verifier,
            block,
            id,
            statement.origin,
            receiver,
            target,
            None,
            destination,
        )?,
        MirStatementKind::WriteField {
            receiver,
            target,
            value,
        } => verify_field_operation(
            verifier,
            block,
            id,
            statement.origin,
            receiver,
            target,
            Some(value),
            None,
        )?,
        MirStatementKind::State(operation) => {
            let (state, expected_storage) = match operation {
                MirStateOperation::ReadVmState { state }
                | MirStateOperation::WriteVmState { state, .. } => {
                    (*state, CompileStateStorage::Vm)
                }
                MirStateOperation::ReadExternState { state } => {
                    (*state, CompileStateStorage::Extern)
                }
            };
            let descriptor = verifier.program.targets().state(state).ok_or_else(|| {
                missing_target(verifier, statement.origin, MirVerifyTarget::State(state))
            })?;
            if descriptor.storage != expected_storage {
                return Err(bad_target(
                    verifier,
                    statement.origin,
                    MirVerifyTarget::State(state),
                    "state operation does not match the declaration storage class",
                ));
            }
            let contract = match operation {
                MirStateOperation::ReadVmState { .. }
                | MirStateOperation::ReadExternState { .. } => Some(&descriptor.contract),
                MirStateOperation::WriteVmState { value, .. } => {
                    let actual = verifier.operand_type(value, block, Some(id), statement.origin)?;
                    if !satisfies_contract(actual, &descriptor.contract) {
                        return Err(type_error(
                            verifier,
                            block,
                            Some(id),
                            statement.origin,
                            "VM-state write value",
                            actual,
                        ));
                    }
                    None
                }
            };
            destination_contract(verifier, block, id, statement.origin, destination, contract)?;
        }
        MirStatementKind::Allocate(value) => {
            verify_aggregate(verifier, block, id, statement.origin, value, destination)?;
        }
        MirStatementKind::Call(call) => {
            reject_known_async_call(
                verifier,
                block,
                id,
                statement.origin,
                call.known_asyncness(),
            )?;
            verify_call(verifier, block, id, statement.origin, call, destination)?
        }
        MirStatementKind::Task(task) => {
            verify_task(verifier, block, id, statement.origin, task, destination)?
        }
        MirStatementKind::Host(operation) => {
            if let crate::MirHostOperation::Call { target, .. } = operation {
                reject_known_async_call(
                    verifier,
                    block,
                    id,
                    statement.origin,
                    Some(target.signature.asyncness),
                )?;
            }
            verify_host(
                verifier,
                block,
                id,
                statement.origin,
                operation,
                destination,
            )?;
        }
        MirStatementKind::Reflect(operation) => {
            let function = match operation {
                MirReflectionOperation::Read { function, .. }
                | MirReflectionOperation::Write { function, .. }
                | MirReflectionOperation::Call { function, .. } => *function,
            };
            reject_known_async_call(
                verifier,
                block,
                id,
                statement.origin,
                Some(
                    function_target(verifier, function, statement.origin)?
                        .signature
                        .asyncness,
                ),
            )?;
            verify_reflection(
                verifier,
                block,
                id,
                statement.origin,
                statement.effect,
                operation,
                destination,
            )?;
        }
        MirStatementKind::GuardTrap { value, guard } => {
            verify_guard_use(
                verifier,
                block,
                Some(id),
                statement.origin,
                *guard,
                value,
                false,
            )?;
        }
        MirStatementKind::Iterator(MirIteratorOperation::Create { .. }) => destination_accepts(
            verifier,
            block,
            id,
            statement.origin,
            destination,
            MirValueType::Iterator,
        )?,
        MirStatementKind::FormatString { .. } => destination_accepts(
            verifier,
            block,
            id,
            statement.origin,
            destination,
            MirValueType::Primitive(PrimitiveTag::String),
        )?,
        MirStatementKind::MakeRange { start, end, .. } => {
            for bound in [start, end] {
                let actual = verifier.operand_type(bound, block, Some(id), statement.origin)?;
                if !integer_or_dynamic(actual) {
                    return Err(type_error(
                        verifier,
                        block,
                        Some(id),
                        statement.origin,
                        "range bound",
                        actual,
                    ));
                }
            }
            destination_accepts(
                verifier,
                block,
                id,
                statement.origin,
                destination,
                MirValueType::Range,
            )?;
        }
        MirStatementKind::MaterializeConstant(value) => {
            if let Some(value_type) = constant_type(value) {
                destination_accepts(
                    verifier,
                    block,
                    id,
                    statement.origin,
                    destination,
                    value_type,
                )?;
            }
        }
        MirStatementKind::DynamicUnary { .. } | MirStatementKind::Index(_) => {}
    }
    Ok(())
}

fn verify_terminator(
    verifier: &FunctionVerifier<'_>,
    block: crate::MirBlockId,
    terminator: &crate::MirTerminator,
) -> Result<(), MirVerifyError> {
    match &terminator.kind {
        MirTerminatorKind::Branch { .. } => {}
        MirTerminatorKind::Switch {
            discriminant,
            cases,
            ..
        } => {
            let actual = verifier.operand_type(discriminant, block, None, terminator.origin)?;
            let mut seen = BTreeSet::new();
            for case in cases {
                if !seen.insert(&case.value) {
                    return Err(error(
                        verifier,
                        Some(block),
                        None,
                        terminator.origin,
                        MirVerifyErrorKind::InvalidTerminatorContract(
                            "switch repeats a case value".to_owned(),
                        ),
                    ));
                }
                switch_case(verifier, terminator.origin, actual, &case.value)?;
            }
        }
        MirTerminatorKind::GuardBranch {
            value,
            guard,
            passed,
            slow,
        } => {
            if passed == slow {
                return Err(error(
                    verifier,
                    Some(block),
                    None,
                    terminator.origin,
                    MirVerifyErrorKind::InvalidTerminatorContract(
                        "guard branch passed and slow targets must differ".to_owned(),
                    ),
                ));
            }
            verify_guard_use(
                verifier,
                block,
                None,
                terminator.origin,
                *guard,
                value,
                true,
            )?;
        }
        MirTerminatorKind::TrySwitch {
            value,
            target,
            result,
            ..
        } => {
            require_local(verifier, block, terminator.origin, *result)?;
            let actual = verifier.operand_type(value, block, None, terminator.origin)?;
            let valid = match target {
                crate::CompileTryTarget::Expected(_) => {
                    matches!(actual, MirValueType::Dynamic | MirValueType::Enum(_))
                }
                crate::CompileTryTarget::Dynamic { .. } => {
                    matches!(actual, MirValueType::Dynamic)
                }
            };
            if !valid {
                return Err(type_error(
                    verifier,
                    block,
                    None,
                    terminator.origin,
                    "try operand",
                    actual,
                ));
            }
        }
        MirTerminatorKind::IteratorNext { iterator, item, .. } => {
            let actual = verifier.operand_type(iterator, block, None, terminator.origin)?;
            if !matches!(actual, MirValueType::Dynamic | MirValueType::Iterator) {
                return Err(type_error(
                    verifier,
                    block,
                    None,
                    terminator.origin,
                    "iterator-next operand",
                    actual,
                ));
            }
            require_local(verifier, block, terminator.origin, *item)?;
        }
        MirTerminatorKind::RangeNext {
            cursor,
            end,
            exhausted,
            item,
            mode,
            ..
        } => {
            let cursor = require_local(verifier, block, terminator.origin, *cursor)?.value_type;
            let exhausted =
                require_local(verifier, block, terminator.origin, *exhausted)?.value_type;
            let item = require_local(verifier, block, terminator.origin, *item)?.value_type;
            let end = verifier.operand_type(end, block, None, terminator.origin)?;
            if exhausted != MirValueType::Primitive(PrimitiveTag::Bool)
                || item != MirValueType::Primitive(PrimitiveTag::I64)
                || match mode {
                    crate::MirRangeStepMode::I64Proven => {
                        cursor != MirValueType::Primitive(PrimitiveTag::I64)
                            || end != MirValueType::Primitive(PrimitiveTag::I64)
                    }
                    crate::MirRangeStepMode::DynamicInteger => {
                        !integer_or_dynamic(cursor) || !integer_or_dynamic(end)
                    }
                }
            {
                return Err(error(
                    verifier,
                    Some(block),
                    None,
                    terminator.origin,
                    MirVerifyErrorKind::InvalidTerminatorContract(
                        "range-next local or operand types contradict its mode".to_owned(),
                    ),
                ));
            }
        }
        MirTerminatorKind::AwaitCall {
            operation,
            destination,
            resume,
        } => {
            if verifier.function.asyncness() != CallableAsyncness::Async {
                return Err(error(
                    verifier,
                    Some(block),
                    None,
                    terminator.origin,
                    MirVerifyErrorKind::InvalidTerminatorContract(
                        "await terminator is only valid in an async function".to_owned(),
                    ),
                ));
            }
            if *resume == block {
                return Err(error(
                    verifier,
                    Some(block),
                    None,
                    terminator.origin,
                    MirVerifyErrorKind::InvalidTerminatorContract(
                        "await resume block must differ from its source block".to_owned(),
                    ),
                ));
            }
            let MirPlace::Local(destination) = destination else {
                return Err(error(
                    verifier,
                    Some(block),
                    None,
                    terminator.origin,
                    MirVerifyErrorKind::InvalidTerminatorContract(
                        "await destination must be a local defined on the resume edge".to_owned(),
                    ),
                ));
            };
            let destination =
                require_local(verifier, block, terminator.origin, *destination)?.value_type;
            if !operation.has_valid_call_contract() {
                return Err(error(
                    verifier,
                    Some(block),
                    None,
                    terminator.origin,
                    MirVerifyErrorKind::InvalidCallContract(
                        "argument placement violates the MIR await call contract".to_owned(),
                    ),
                ));
            }
            verify_await_operation(
                verifier,
                block,
                terminator.origin,
                terminator.effect,
                operation,
                destination,
            )?;
        }
        MirTerminatorKind::Return(value) => {
            if let Some(value) = value {
                let _ = verifier.operand_type(value, block, None, terminator.origin)?;
            }
        }
        MirTerminatorKind::Jump(_)
        | MirTerminatorKind::TryTypeMismatch { .. }
        | MirTerminatorKind::Unreachable => {}
    }
    Ok(())
}

fn reject_known_async_call(
    verifier: &FunctionVerifier<'_>,
    block: crate::MirBlockId,
    statement: MirStatementId,
    origin: MirSourceOrigin,
    asyncness: Option<CallableAsyncness>,
) -> Result<(), MirVerifyError> {
    if asyncness == Some(CallableAsyncness::Async) {
        Err(error(
            verifier,
            Some(block),
            Some(statement),
            origin,
            MirVerifyErrorKind::InvalidCallContract(
                "known async call must use an await terminator".to_owned(),
            ),
        ))
    } else {
        Ok(())
    }
}

fn verify_await_operation(
    verifier: &FunctionVerifier<'_>,
    block: crate::MirBlockId,
    origin: MirSourceOrigin,
    effect: MirEffect,
    operation: &MirAwaitOperation,
    destination: MirValueType,
) -> Result<(), MirVerifyError> {
    // The operation verifiers use a statement id only for point facts and
    // error attribution. Await operands live at the terminator, so this id
    // deliberately misses point facts and errors are normalized below.
    let sentinel = MirStatementId::from_index(u32::MAX);
    let result = match operation {
        MirAwaitOperation::Call(call) => {
            verify_call(verifier, block, sentinel, origin, call, Some(destination))
        }
        MirAwaitOperation::Host(operation) => verify_host(
            verifier,
            block,
            sentinel,
            origin,
            operation,
            Some(destination),
        ),
        MirAwaitOperation::Reflect(operation) => verify_reflection(
            verifier,
            block,
            sentinel,
            origin,
            effect,
            operation,
            Some(destination),
        ),
    };
    result.map_err(|mut error| {
        error.statement = None;
        error
    })
}

fn verify_call_argument_contracts(
    verifier: &FunctionVerifier<'_>,
    block: crate::MirBlockId,
    statement: crate::MirStatementId,
    origin: MirSourceOrigin,
    call: &MirCall,
) -> Result<(), MirVerifyError> {
    match call {
        MirCall::ScriptFunction {
            signature,
            arguments,
            ..
        }
        | MirCall::ScriptMethod {
            signature,
            arguments,
            ..
        } => {
            for argument in arguments {
                let Some(value) = argument.value.as_ref() else {
                    continue;
                };
                if let Some(contract) = signature
                    .parameters
                    .get(argument.parameter as usize)
                    .and_then(|parameter| parameter.contract.as_ref())
                {
                    verify_call_argument(verifier, block, statement, origin, value, contract)?;
                }
            }
        }
        MirCall::NativeFunction {
            signature,
            arguments,
            ..
        }
        | MirCall::StdlibFunction {
            signature,
            arguments,
            ..
        }
        | MirCall::ValueMethod {
            signature,
            arguments,
            ..
        }
        | MirCall::Service {
            signature,
            arguments,
            ..
        } => {
            for (argument, parameter) in arguments.iter().zip(&signature.parameters) {
                if let Some(contract) = parameter.contract.as_ref() {
                    verify_call_argument(verifier, block, statement, origin, argument, contract)?;
                }
            }
        }
        MirCall::CallableValue { .. }
        | MirCall::DynamicCallable { .. }
        | MirCall::DynamicMethod { .. } => {}
    }
    Ok(())
}

fn verify_call_argument(
    verifier: &FunctionVerifier<'_>,
    block: crate::MirBlockId,
    statement: crate::MirStatementId,
    origin: MirSourceOrigin,
    value: &MirOperand,
    contract: &MirTypeContract,
) -> Result<(), MirVerifyError> {
    let actual = verifier.operand_type(value, block, Some(statement), origin)?;
    if satisfies_contract(actual, contract) {
        Ok(())
    } else {
        Err(type_error(
            verifier,
            block,
            Some(statement),
            origin,
            "call argument",
            actual,
        ))
    }
}

fn verify_reflection(
    verifier: &FunctionVerifier<'_>,
    block: crate::MirBlockId,
    statement: crate::MirStatementId,
    origin: MirSourceOrigin,
    effect: MirEffect,
    operation: &MirReflectionOperation,
    destination: Option<MirValueType>,
) -> Result<(), MirVerifyError> {
    let (function, provided) = match operation {
        MirReflectionOperation::Read { function, .. } => (*function, 2),
        MirReflectionOperation::Write { function, .. } => (*function, 3),
        MirReflectionOperation::Call { function, tail, .. } => (*function, 1 + tail.len()),
    };
    let descriptor = function_target(verifier, function, origin)?;
    if !matches!(
        descriptor.class,
        CompileFunctionClass::Native | CompileFunctionClass::Registry
    ) || !arity_accepts(&descriptor.signature, provided)
    {
        return Err(error(
            verifier,
            None,
            None,
            origin,
            MirVerifyErrorKind::InvalidReflectionContract(
                "reflection operation identity or arity is invalid".to_owned(),
            ),
        ));
    }
    if !effect.contains(descriptor.signature.effect) {
        return Err(error(
            verifier,
            Some(block),
            Some(statement),
            origin,
            MirVerifyErrorKind::IncompleteEffect {
                required: descriptor.signature.effect,
                actual: effect,
            },
        ));
    }
    destination_contract(
        verifier,
        block,
        statement,
        origin,
        destination,
        descriptor.signature.return_contract.as_ref(),
    )
}

fn verify_task(
    verifier: &FunctionVerifier<'_>,
    block: crate::MirBlockId,
    statement: crate::MirStatementId,
    origin: MirSourceOrigin,
    task: &MirTaskOperation,
    destination: Option<MirValueType>,
) -> Result<(), MirVerifyError> {
    function_call_target(
        verifier,
        origin,
        task.worker,
        &task.worker_debug_name,
        &task.worker_signature,
        CompileFunctionClass::Script,
    )?;
    if task.worker_signature.asyncness != CallableAsyncness::Async {
        return Err(bad_target(
            verifier,
            origin,
            MirVerifyTarget::Function(task.worker),
            "task worker must be asynchronous",
        ));
    }
    let worker_call = MirCall::ScriptFunction {
        function: task.worker,
        debug_name: task.worker_debug_name.clone(),
        signature: task.worker_signature.clone(),
        arguments: task.arguments.clone(),
        parameter_guards: task.parameter_guards,
    };
    verify_call_argument_contracts(verifier, block, statement, origin, &worker_call)?;
    if task.detachability.parameters.len() != task.worker_signature.parameters.len()
        || task
            .detachability
            .parameters
            .iter()
            .any(|fact| fact.rejection().is_some())
    {
        return Err(bad_target(
            verifier,
            origin,
            MirVerifyTarget::Function(task.worker),
            "task parameter detachability is invalid",
        ));
    }
    for (parameter, fact) in task
        .worker_signature
        .parameters
        .iter()
        .zip(&task.detachability.parameters)
    {
        let expected =
            crate::contract_detachability(verifier.program.targets(), parameter.contract.as_ref())
                .fact;
        if expected.rejection().is_some() || expected.union(*fact) != *fact {
            return Err(bad_target(
                verifier,
                origin,
                MirVerifyTarget::Function(task.worker),
                "task parameter detachability is weaker than the worker contract",
            ));
        }
    }
    let expected_result = crate::contract_detachability(
        verifier.program.targets(),
        task.worker_signature.return_contract.as_ref(),
    )
    .fact;
    if task.detachability.result != expected_result || expected_result.rejection().is_some() {
        return Err(bad_target(
            verifier,
            origin,
            MirVerifyTarget::Function(task.worker),
            "task result detachability disagrees with the worker contract",
        ));
    }
    if let Some(continuation) = &task.continuation {
        function_call_target(
            verifier,
            origin,
            continuation.function,
            &continuation.debug_name,
            &continuation.signature,
            CompileFunctionClass::Script,
        )?;
        if continuation.signature.asyncness != CallableAsyncness::Sync {
            return Err(bad_target(
                verifier,
                origin,
                MirVerifyTarget::Function(continuation.function),
                "task continuation must be synchronous",
            ));
        }
        let expected_outcome = crate::MirTypeContract::Result {
            ok: task.worker_signature.return_contract.clone().map(Box::new),
            err: Some(Box::new(crate::MirTypeContract::TaskError)),
        };
        if continuation.outcome_contract != expected_outcome
            || continuation
                .signature
                .parameters
                .first()
                .and_then(|parameter| parameter.contract.as_ref())
                != Some(&expected_outcome)
            || continuation.signature.parameters.get(1..)
                != Some(continuation.resume_parameters.as_slice())
        {
            return Err(bad_target(
                verifier,
                origin,
                MirVerifyTarget::Function(continuation.function),
                "task continuation ABI disagrees with the worker outcome",
            ));
        }
    }
    destination_accepts(
        verifier,
        block,
        statement,
        origin,
        destination,
        MirValueType::Unit,
    )
}
