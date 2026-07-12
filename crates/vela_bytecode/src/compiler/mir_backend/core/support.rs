use std::collections::BTreeSet;

use vela_host::resolved::HostMutationOp;
use vela_mir::{
    MirBinaryOp, MirBlockId, MirComparisonOp, MirDynamicBinaryOp, MirFunction, MirGuardLocation,
    MirHostMutation, MirNumericBinaryOp, MirProgram, MirTerminatorKind, MirTypeContract,
};

use crate::{
    GuardKind, GuardLocation, Register, StandardTypeGuard, UnlinkedGuardContext,
    UnlinkedInstructionKind, UnlinkedTypeGuard, UnlinkedTypeGuardPlan,
};

use super::MirBackendError;

pub(super) fn binary_instruction(
    operation: MirBinaryOp,
    dst: Register,
    lhs: Register,
    rhs: Register,
) -> UnlinkedInstructionKind {
    match operation {
        MirBinaryOp::Numeric {
            operation: MirNumericBinaryOp::Add,
            kind: vela_common::NumericTag::I64,
        } => UnlinkedInstructionKind::Add { dst, lhs, rhs },
        MirBinaryOp::Numeric {
            operation: MirNumericBinaryOp::Add,
            ..
        } => UnlinkedInstructionKind::Add { dst, lhs, rhs },
        MirBinaryOp::Numeric {
            operation: MirNumericBinaryOp::Subtract,
            kind: vela_common::NumericTag::I64,
        } => UnlinkedInstructionKind::I64Sub { dst, lhs, rhs },
        MirBinaryOp::Numeric {
            operation: MirNumericBinaryOp::Subtract,
            ..
        } => UnlinkedInstructionKind::Sub { dst, lhs, rhs },
        MirBinaryOp::Numeric {
            operation: MirNumericBinaryOp::Multiply,
            kind: vela_common::NumericTag::I64,
        } => UnlinkedInstructionKind::I64Mul { dst, lhs, rhs },
        MirBinaryOp::Numeric {
            operation: MirNumericBinaryOp::Multiply,
            ..
        } => UnlinkedInstructionKind::Mul { dst, lhs, rhs },
        MirBinaryOp::Numeric {
            operation: MirNumericBinaryOp::Divide,
            ..
        } => UnlinkedInstructionKind::Div { dst, lhs, rhs },
        MirBinaryOp::Numeric {
            operation: MirNumericBinaryOp::Remainder,
            kind: vela_common::NumericTag::I64,
        } => UnlinkedInstructionKind::Rem { dst, lhs, rhs },
        MirBinaryOp::Numeric {
            operation: MirNumericBinaryOp::Remainder,
            ..
        } => UnlinkedInstructionKind::Rem { dst, lhs, rhs },
        MirBinaryOp::Compare { operation, .. } => comparison_instruction(operation, dst, lhs, rhs),
    }
}

pub(super) fn mir_successors(kind: &MirTerminatorKind) -> Vec<MirBlockId> {
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
            join,
            ..
        } => continuations
            .iter()
            .map(|continuation| continuation.block)
            .chain([*propagate, *invalid, *join])
            .collect(),
        MirTerminatorKind::IteratorNext { next, done, .. }
        | MirTerminatorKind::RangeNext { next, done, .. } => vec![*next, *done],
        MirTerminatorKind::Return(_)
        | MirTerminatorKind::TryTypeMismatch { .. }
        | MirTerminatorKind::Unreachable => Vec::new(),
    }
}

pub(super) fn mir_reaches(function: &MirFunction, start: MirBlockId, target: MirBlockId) -> bool {
    let mut pending = vec![start];
    let mut visited = BTreeSet::new();
    while let Some(block) = pending.pop() {
        if block == target {
            return true;
        }
        if !visited.insert(block) {
            continue;
        }
        if let Some(terminator) = function.block(block).and_then(|block| block.terminator()) {
            pending.extend(mir_successors(&terminator.kind));
        }
    }
    false
}

pub(super) fn dynamic_binary_instruction(
    operation: MirDynamicBinaryOp,
    dst: Register,
    lhs: Register,
    rhs: Register,
) -> UnlinkedInstructionKind {
    match operation {
        MirDynamicBinaryOp::Add => UnlinkedInstructionKind::Add { dst, lhs, rhs },
        MirDynamicBinaryOp::Subtract => UnlinkedInstructionKind::Sub { dst, lhs, rhs },
        MirDynamicBinaryOp::Multiply => UnlinkedInstructionKind::Mul { dst, lhs, rhs },
        MirDynamicBinaryOp::Divide => UnlinkedInstructionKind::Div { dst, lhs, rhs },
        MirDynamicBinaryOp::Remainder => UnlinkedInstructionKind::Rem { dst, lhs, rhs },
        MirDynamicBinaryOp::Equal => UnlinkedInstructionKind::Equal { dst, lhs, rhs },
        MirDynamicBinaryOp::NotEqual => UnlinkedInstructionKind::NotEqual { dst, lhs, rhs },
        MirDynamicBinaryOp::Less => UnlinkedInstructionKind::Less { dst, lhs, rhs },
        MirDynamicBinaryOp::LessEqual => UnlinkedInstructionKind::LessEqual { dst, lhs, rhs },
        MirDynamicBinaryOp::Greater => UnlinkedInstructionKind::Greater { dst, lhs, rhs },
        MirDynamicBinaryOp::GreaterEqual => UnlinkedInstructionKind::GreaterEqual { dst, lhs, rhs },
    }
}

fn comparison_instruction(
    operation: MirComparisonOp,
    dst: Register,
    lhs: Register,
    rhs: Register,
) -> UnlinkedInstructionKind {
    match operation {
        MirComparisonOp::Equal => UnlinkedInstructionKind::Equal { dst, lhs, rhs },
        MirComparisonOp::NotEqual => UnlinkedInstructionKind::NotEqual { dst, lhs, rhs },
        MirComparisonOp::Less => UnlinkedInstructionKind::Less { dst, lhs, rhs },
        MirComparisonOp::LessEqual => UnlinkedInstructionKind::LessEqual { dst, lhs, rhs },
        MirComparisonOp::Greater => UnlinkedInstructionKind::Greater { dst, lhs, rhs },
        MirComparisonOp::GreaterEqual => UnlinkedInstructionKind::GreaterEqual { dst, lhs, rhs },
    }
}

pub(super) const fn i64_compare(operation: MirComparisonOp) -> crate::I64CompareOp {
    match operation {
        MirComparisonOp::Equal => crate::I64CompareOp::Equal,
        MirComparisonOp::NotEqual => crate::I64CompareOp::NotEqual,
        MirComparisonOp::Less => crate::I64CompareOp::Less,
        MirComparisonOp::LessEqual => crate::I64CompareOp::LessEqual,
        MirComparisonOp::Greater => crate::I64CompareOp::Greater,
        MirComparisonOp::GreaterEqual => crate::I64CompareOp::GreaterEqual,
    }
}

pub(super) fn host_mutation(operation: MirHostMutation) -> HostMutationOp {
    match operation {
        MirHostMutation::Add => HostMutationOp::Add,
        MirHostMutation::Subtract => HostMutationOp::Sub,
        MirHostMutation::Multiply => HostMutationOp::Mul,
        MirHostMutation::Divide => HostMutationOp::Div,
        MirHostMutation::Remainder => HostMutationOp::Rem,
        MirHostMutation::Push => HostMutationOp::Push,
    }
}

pub(super) fn guard_kind(location: MirGuardLocation) -> GuardKind {
    let _ = location;
    GuardKind::Contract
}

pub(super) fn guard_location(location: MirGuardLocation) -> Result<GuardLocation, MirBackendError> {
    Ok(match location {
        MirGuardLocation::Parameter { index } => GuardLocation::Parameter {
            index: u16::try_from(index).map_err(|_| MirBackendError::RegisterOverflow)?,
        },
        MirGuardLocation::Return => GuardLocation::Return,
        MirGuardLocation::Local => GuardLocation::Local,
        MirGuardLocation::Global => GuardLocation::Global,
        MirGuardLocation::Field => GuardLocation::Field,
    })
}

pub(super) fn type_guard(
    program: &MirProgram,
    contract: &MirTypeContract,
    kind: GuardKind,
    location: GuardLocation,
    debug_name: &str,
) -> Result<UnlinkedTypeGuard, MirBackendError> {
    Ok(UnlinkedTypeGuard::new(
        type_guard_plan(program, contract)?,
        UnlinkedGuardContext::new(kind, location, debug_name),
    ))
}

fn type_guard_plan(
    program: &MirProgram,
    contract: &MirTypeContract,
) -> Result<UnlinkedTypeGuardPlan, MirBackendError> {
    let nested = |value: &Option<Box<MirTypeContract>>| -> Result<Option<Box<UnlinkedTypeGuardPlan>>, MirBackendError> {
        value.as_deref().map(|value| type_guard_plan(program, value).map(Box::new)).transpose()
    };
    Ok(match contract {
        MirTypeContract::Any => UnlinkedTypeGuardPlan::Type {
            name: "Any".to_owned(),
            type_id: None,
        },
        MirTypeContract::Primitive(value) => UnlinkedTypeGuardPlan::Primitive(*value),
        MirTypeContract::Range => UnlinkedTypeGuardPlan::Standard(StandardTypeGuard::Range),
        MirTypeContract::Array(value) => UnlinkedTypeGuardPlan::Array {
            element: nested(value)?,
        },
        MirTypeContract::Map { key, value } => UnlinkedTypeGuardPlan::Map {
            key: nested(key)?,
            value: nested(value)?,
        },
        MirTypeContract::Set(value) => UnlinkedTypeGuardPlan::Set {
            element: nested(value)?,
        },
        MirTypeContract::Iterator(value) => UnlinkedTypeGuardPlan::Iterator {
            item: nested(value)?,
        },
        MirTypeContract::Tuple(values) => UnlinkedTypeGuardPlan::Tuple {
            elements: values
                .iter()
                .map(|value| {
                    value
                        .as_ref()
                        .map(|value| type_guard_plan(program, value).map(Box::new))
                        .transpose()
                })
                .collect::<Result<_, _>>()?,
        },
        MirTypeContract::Option(value) => UnlinkedTypeGuardPlan::Option {
            some: nested(value)?,
        },
        MirTypeContract::Result { ok, err } => UnlinkedTypeGuardPlan::Result {
            ok: nested(ok)?,
            err: nested(err)?,
        },
        MirTypeContract::Callable {
            accepted_kinds,
            positional_arity,
        } => UnlinkedTypeGuardPlan::Callable {
            accepts_direct_function: accepted_kinds.accepts_direct_function(),
            accepts_closure: accepted_kinds.accepts_closure(),
            positional_arity: *positional_arity,
        },
        MirTypeContract::Definition(type_id) => {
            let ty = program
                .targets()
                .type_descriptor(*type_id)
                .ok_or(MirBackendError::MissingTarget("type"))?;
            UnlinkedTypeGuardPlan::Type {
                name: ty.runtime_name.clone(),
                type_id: Some(*type_id),
            }
        }
        MirTypeContract::Shape { type_id, shape } => {
            let ty = program
                .targets()
                .type_descriptor(*type_id)
                .ok_or(MirBackendError::MissingTarget("type"))?;
            UnlinkedTypeGuardPlan::Shape {
                type_name: ty.runtime_name.clone(),
                type_id: *type_id,
                shape_id: *shape,
            }
        }
        MirTypeContract::Variant { type_id, variant } => {
            let ty = program
                .targets()
                .type_descriptor(*type_id)
                .ok_or(MirBackendError::MissingTarget("type"))?;
            let variant = program
                .targets()
                .variant(*variant)
                .ok_or(MirBackendError::MissingTarget("variant"))?;
            UnlinkedTypeGuardPlan::Variant {
                enum_name: ty.runtime_name.clone(),
                type_id: Some(*type_id),
                variant: variant.name.clone(),
                variant_id: Some(variant.id),
            }
        }
        MirTypeContract::Host(target) => {
            let ty = program
                .targets()
                .type_descriptor(target.semantic)
                .ok_or(MirBackendError::MissingTarget("host type"))?;
            UnlinkedTypeGuardPlan::HostType {
                type_name: ty.runtime_name.clone(),
                type_id: target.semantic,
                host_type_id: target.runtime,
            }
        }
    })
}
