use std::collections::BTreeSet;

use crate::verifier::{FunctionVerifier, MirVerifyError, MirVerifyErrorKind, MirVerifyTarget};
use crate::{
    MirAggregate, MirFieldTarget, MirGuardAssumption, MirOperand, MirPatternPredicate, MirRvalue,
    MirSourceNode, MirSourceOrigin, MirValueType,
};

use super::support::{
    bad_target, compatible, destination_accepts, destination_contract, error, missing_target,
    operand_value_type, require_field, require_type, require_variant, satisfies_contract,
    type_error,
};

pub(super) fn verify_aggregate(
    verifier: &FunctionVerifier<'_>,
    block: crate::MirBlockId,
    statement: crate::MirStatementId,
    origin: MirSourceOrigin,
    aggregate: &MirAggregate,
    destination: Option<MirValueType>,
) -> Result<(), MirVerifyError> {
    let result_type = match aggregate {
        MirAggregate::Record {
            type_id,
            shape,
            fields,
        } => {
            let descriptor = require_type(verifier, *type_id, origin)?;
            if descriptor.shape != Some(*shape) {
                return Err(bad_target(
                    verifier,
                    origin,
                    MirVerifyTarget::Type(*type_id),
                    "record aggregate shape disagrees with its descriptor",
                ));
            }
            let supplied = fields
                .iter()
                .map(|(field, _)| *field)
                .collect::<BTreeSet<_>>();
            let expected = descriptor.fields.iter().copied().collect::<BTreeSet<_>>();
            if supplied.len() != fields.len() || supplied != expected {
                return Err(bad_target(
                    verifier,
                    origin,
                    MirVerifyTarget::Type(*type_id),
                    "record aggregate fields are duplicate, missing, or extra",
                ));
            }
            for (field, _) in fields {
                require_field(verifier, *field, *type_id, None, origin)?;
            }
            MirValueType::ScriptType {
                type_id: *type_id,
                shape: *shape,
            }
        }
        MirAggregate::Enum {
            type_id,
            variant,
            fields,
        } => {
            require_variant(verifier, *type_id, *variant, origin)?;
            let variant_descriptor = verifier
                .program
                .targets()
                .variant(*variant)
                .expect("required variant descriptor exists");
            let supplied = fields
                .iter()
                .map(|(field, _)| *field)
                .collect::<BTreeSet<_>>();
            let expected = variant_descriptor
                .fields
                .iter()
                .copied()
                .collect::<BTreeSet<_>>();
            if supplied.len() != fields.len() || supplied != expected {
                return Err(bad_target(
                    verifier,
                    origin,
                    MirVerifyTarget::Variant(*variant),
                    "enum aggregate fields are duplicate, missing, or extra",
                ));
            }
            for (field, _) in fields {
                require_field(verifier, *field, *type_id, Some(*variant), origin)?;
            }
            MirValueType::Enum(*type_id)
        }
        MirAggregate::Closure { function, captures } => {
            let child = verifier.program.function(*function).ok_or_else(|| {
                missing_target(verifier, origin, MirVerifyTarget::MirFunction(*function))
            })?;
            let source_expression = match origin.node {
                MirSourceNode::Expression(expression) => Some(expression),
                MirSourceNode::Declaration(_)
                | MirSourceNode::Body(_)
                | MirSourceNode::Statement(_)
                | MirSourceNode::Pattern(_) => None,
            };
            if !matches!(child.owner(), crate::MirFunctionOwner::Lambda { parent, expression }
                if *parent == verifier.function_id && Some(*expression) == source_expression)
                || child.captures().len() != captures.len()
            {
                return Err(bad_target(
                    verifier,
                    origin,
                    MirVerifyTarget::MirFunction(*function),
                    "closure child owner or capture arity is invalid",
                ));
            }
            for (operand, capture) in captures.iter().zip(child.captures()) {
                let actual = operand_value_type(verifier, operand)?;
                let expected = child
                    .local(capture.storage)
                    .expect("verified child capture storage exists")
                    .value_type;
                if !compatible(actual, expected) {
                    return Err(bad_target(
                        verifier,
                        origin,
                        MirVerifyTarget::MirFunction(*function),
                        "closure capture order or operand type disagrees with child ABI",
                    ));
                }
            }
            MirValueType::Callable
        }
        MirAggregate::Tuple(values) => MirValueType::Tuple(values.len() as u32),
        MirAggregate::Array(_) | MirAggregate::Map(_) | MirAggregate::SetFromArray { .. } => {
            MirValueType::Dynamic
        }
        MirAggregate::DynamicRecord { fields, .. }
        | MirAggregate::DynamicVariant { fields, .. } => {
            let names = fields.iter().map(|(name, _)| name).collect::<BTreeSet<_>>();
            if names.len() != fields.len() {
                return Err(error(
                    verifier,
                    Some(block),
                    Some(statement),
                    origin,
                    MirVerifyErrorKind::InconsistentTarget {
                        target: MirVerifyTarget::MirFunction(verifier.function_id),
                        detail: "dynamic aggregate repeats a field name".to_owned(),
                    },
                ));
            }
            MirValueType::Dynamic
        }
    };
    destination_accepts(verifier, block, statement, origin, destination, result_type)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn verify_field_operation(
    verifier: &FunctionVerifier<'_>,
    block: crate::MirBlockId,
    statement: crate::MirStatementId,
    origin: MirSourceOrigin,
    receiver: &MirOperand,
    target: &MirFieldTarget,
    written: Option<&MirOperand>,
    destination: Option<MirValueType>,
) -> Result<(), MirVerifyError> {
    let receiver_type = verifier.operand_type(receiver, block, Some(statement), origin)?;
    let descriptor = match target {
        MirFieldTarget::RecordSlot {
            type_id,
            shape,
            field,
        } => {
            let descriptor = require_type(verifier, *type_id, origin)?;
            if descriptor.shape != Some(*shape) {
                return Err(bad_target(
                    verifier,
                    origin,
                    MirVerifyTarget::Type(*type_id),
                    "record field shape disagrees with its descriptor",
                ));
            }
            if receiver_type != MirValueType::Dynamic
                && receiver_type
                    != (MirValueType::ScriptType {
                        type_id: *type_id,
                        shape: *shape,
                    })
            {
                return Err(type_error(
                    verifier,
                    block,
                    Some(statement),
                    origin,
                    "record field receiver",
                    receiver_type,
                ));
            }
            Some(require_field(verifier, *field, *type_id, None, origin)?)
        }
        MirFieldTarget::VariantSlot {
            type_id,
            variant,
            field,
        } => {
            require_variant(verifier, *type_id, *variant, origin)?;
            let try_continuation = verifier.function.blocks().any(|(_, candidate)| {
                matches!(
                    candidate.terminator().map(|terminator| &terminator.kind),
                    Some(crate::MirTerminatorKind::TrySwitch { continuations, .. })
                        if continuations.iter().any(|continuation| continuation.block == block)
                )
            });
            if receiver_type != MirValueType::Dynamic
                && receiver_type != MirValueType::Enum(*type_id)
                && !(try_continuation && matches!(receiver_type, MirValueType::Enum(_)))
            {
                return Err(type_error(
                    verifier,
                    block,
                    Some(statement),
                    origin,
                    "variant field receiver",
                    receiver_type,
                ));
            }
            Some(require_field(
                verifier,
                *field,
                *type_id,
                Some(*variant),
                origin,
            )?)
        }
        MirFieldTarget::DynamicRecord { .. } | MirFieldTarget::DynamicVariant { .. } => {
            if matches!(receiver_type, MirValueType::Host(_)) {
                return Err(error(
                    verifier,
                    Some(block),
                    Some(statement),
                    origin,
                    MirVerifyErrorKind::InvalidHostContract(
                        "host member access cannot use an ordinary dynamic field operation"
                            .to_owned(),
                    ),
                ));
            }
            None
        }
    };
    if written.is_some()
        && matches!(
            target,
            MirFieldTarget::VariantSlot { .. } | MirFieldTarget::DynamicVariant { .. }
        )
    {
        return Err(bad_target(
            verifier,
            origin,
            MirVerifyTarget::MirFunction(verifier.function_id),
            "enum-family fields are read-only MIR targets; assignments use record-family writes",
        ));
    }
    if let Some(descriptor) = descriptor {
        if (written.is_some() && !descriptor.access.writable)
            || (written.is_none() && !descriptor.access.readable)
        {
            return Err(bad_target(
                verifier,
                origin,
                MirVerifyTarget::Field(descriptor.id),
                "field operation violates its access policy",
            ));
        }
        if let Some(value) = written
            && let Some(contract) = descriptor.contract.as_ref()
        {
            let actual = verifier.operand_type(value, block, Some(statement), origin)?;
            if !satisfies_contract(actual, contract) {
                return Err(type_error(
                    verifier,
                    block,
                    Some(statement),
                    origin,
                    "field write value",
                    actual,
                ));
            }
        }
        destination_contract(
            verifier,
            block,
            statement,
            origin,
            destination,
            descriptor.contract.as_ref(),
        )?;
    } else if written.is_none() {
        destination_contract(verifier, block, statement, origin, destination, None)?;
    }
    Ok(())
}

pub(super) fn verify_predicate_targets(
    verifier: &FunctionVerifier<'_>,
    origin: MirSourceOrigin,
    value: &MirRvalue,
) -> Result<(), MirVerifyError> {
    if let MirRvalue::PatternPredicate(predicate) = value {
        match predicate {
            MirPatternPredicate::VariantShape {
                type_id, variant, ..
            } => {
                require_variant(verifier, *type_id, *variant, origin)?;
            }
            MirPatternPredicate::TupleArity { .. }
            | MirPatternPredicate::NeverMatches { .. }
            | MirPatternPredicate::DynamicVariant { .. } => {}
        }
    }
    Ok(())
}

pub(super) fn verify_guard_use(
    verifier: &FunctionVerifier<'_>,
    block: crate::MirBlockId,
    statement: Option<crate::MirStatementId>,
    origin: MirSourceOrigin,
    guard: crate::MirGuardId,
    value: &MirOperand,
    recoverable: bool,
) -> Result<(), MirVerifyError> {
    let record = verifier.function.guard(guard).ok_or_else(|| {
        error(
            verifier,
            Some(block),
            statement,
            origin,
            MirVerifyErrorKind::MissingGuard(guard),
        )
    })?;
    if record.origin != origin {
        return Err(error(
            verifier,
            Some(block),
            statement,
            origin,
            MirVerifyErrorKind::GuardOriginMismatch { guard },
        ));
    }
    let actual = verifier.operand_type(value, block, statement, origin)?;
    let valid = match &record.assumption {
        MirGuardAssumption::Type(contract) => {
            record.kind
                == if recoverable {
                    crate::MirGuardKind::Specialization
                } else {
                    crate::MirGuardKind::Contract
                }
                && record.context.is_some() != recoverable
                && satisfies_contract(actual, contract)
        }
        MirGuardAssumption::TupleArity { arity } => {
            !recoverable
                && record.kind == crate::MirGuardKind::Contract
                && record.context.is_none()
                && (matches!(actual, MirValueType::Dynamic)
                    || matches!(actual, MirValueType::Tuple(actual) if actual == *arity))
        }
    };
    if !valid {
        return Err(error(
            verifier,
            Some(block),
            statement,
            origin,
            MirVerifyErrorKind::InvalidTerminatorContract(
                "guard operand, context, or recoverability contradicts its assumption".to_owned(),
            ),
        ));
    }
    Ok(())
}
