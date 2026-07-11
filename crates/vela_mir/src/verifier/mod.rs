//! Structural and data-flow verification for backend-neutral MIR.
//!
//! Verification consumes MIR and its frozen target table only. It never
//! consults HIR or analysis again, and it deliberately leaves liveness to the
//! separate Phase 3 liveness pass.

mod cfg;
mod dataflow;
mod operations;
mod try_regions;

#[cfg(test)]
mod tests;

use std::error::Error;
use std::fmt;

use vela_def::{FieldId, FunctionId, GlobalId, MethodId, TypeId, VariantId};

use crate::{
    MirBlockId, MirEffect, MirFunction, MirFunctionId, MirGuardId, MirLocalId, MirOperand,
    MirProgram, MirSafepointId, MirSourceOrigin, MirStatementId, MirTempId, MirValueType,
};

/// A MIR program that has passed the complete verifier available in this
/// crate generation.
///
/// The wrapper is intentionally borrowed and generation-local. It is the
/// boundary future physical backends should accept instead of unchecked MIR.
#[derive(Clone, Copy, Debug)]
pub struct VerifiedMirProgram<'a> {
    program: &'a MirProgram,
}

impl<'a> VerifiedMirProgram<'a> {
    #[must_use]
    pub const fn program(self) -> &'a MirProgram {
        self.program
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirDestinationExpectation {
    Required,
    Forbidden,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirVerifyTarget {
    Function(FunctionId),
    Method { owner: TypeId, method: MethodId },
    Type(TypeId),
    Variant(VariantId),
    Field(FieldId),
    Global(GlobalId),
    MirFunction(MirFunctionId),
}

#[derive(Clone, Debug, PartialEq)]
pub enum MirVerifyErrorKind {
    UndefinedFunctionReservation,
    MissingBlock(MirBlockId),
    MissingStatement(MirStatementId),
    DuplicateStatementPlacement(MirStatementId),
    OrphanStatement(MirStatementId),
    UnterminatedBlock,
    UnreachableBlock,
    MissingLocal(MirLocalId),
    MissingTemp(MirTempId),
    MissingGuard(MirGuardId),
    MissingSafepoint(MirSafepointId),
    MissingTarget(MirVerifyTarget),
    InconsistentTarget {
        target: MirVerifyTarget,
        detail: String,
    },
    InvalidFunctionMetadata(String),
    InvalidSourceOrigin(String),
    InvalidDebugMetadata(String),
    InvalidDestination {
        expected: MirDestinationExpectation,
    },
    InvalidConstantDefinition(String),
    IncompleteEffect {
        required: MirEffect,
        actual: MirEffect,
    },
    MissingRequiredSafepoint,
    UnexpectedSafepoint,
    SafepointOriginMismatch {
        safepoint: MirSafepointId,
    },
    GuardOriginMismatch {
        guard: MirGuardId,
    },
    InvalidCallContract(String),
    InvalidHostContract(String),
    InvalidReflectionContract(String),
    InvalidOperandType {
        role: &'static str,
        expected: String,
        actual: MirValueType,
    },
    LocalUseBeforeInitialization(MirLocalId),
    TempHasNoDefinition(MirTempId),
    TempHasMultipleDefinitions(MirTempId),
    TempDefinitionMismatch {
        temp: MirTempId,
        recorded: Option<MirStatementId>,
        actual: Option<MirStatementId>,
    },
    TempUseNotDominated {
        temp: MirTempId,
        definition: MirStatementId,
    },
    InvalidTerminatorContract(String),
}

/// A deterministic MIR verification failure.
///
/// Errors always identify the generation-local function and source origin.
/// Block and statement locations are populated when the malformed value is
/// attached to one of those locations.
#[derive(Clone, Debug, PartialEq)]
pub struct MirVerifyError {
    pub function: MirFunctionId,
    pub block: Option<MirBlockId>,
    pub statement: Option<MirStatementId>,
    pub origin: MirSourceOrigin,
    kind: Box<MirVerifyErrorKind>,
}

impl MirVerifyError {
    #[must_use]
    pub fn kind(&self) -> &MirVerifyErrorKind {
        &self.kind
    }

    #[must_use]
    pub fn into_kind(self) -> MirVerifyErrorKind {
        *self.kind
    }
}

impl fmt::Display for MirVerifyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid MIR in {}", self.function)?;
        if let Some(block) = self.block {
            write!(formatter, " {block}")?;
        }
        if let Some(statement) = self.statement {
            write!(formatter, " {statement}")?;
        }
        write!(
            formatter,
            " at {}:{}..{}: {:?}",
            self.origin.span.source.get(),
            self.origin.span.start,
            self.origin.span.end,
            self.kind()
        )
    }
}

impl Error for MirVerifyError {}

pub(crate) struct FunctionVerifier<'a> {
    program: &'a MirProgram,
    function_id: MirFunctionId,
    function: &'a MirFunction,
}

impl<'a> FunctionVerifier<'a> {
    fn new(program: &'a MirProgram, function_id: MirFunctionId, function: &'a MirFunction) -> Self {
        Self {
            program,
            function_id,
            function,
        }
    }

    pub(crate) fn error(
        &self,
        block: Option<MirBlockId>,
        statement: Option<MirStatementId>,
        origin: MirSourceOrigin,
        kind: MirVerifyErrorKind,
    ) -> MirVerifyError {
        MirVerifyError {
            function: self.function_id,
            block,
            statement,
            origin,
            kind: Box::new(kind),
        }
    }

    pub(crate) fn operand_type(
        &self,
        operand: &MirOperand,
        block: MirBlockId,
        statement: Option<MirStatementId>,
        origin: MirSourceOrigin,
    ) -> Result<MirValueType, MirVerifyError> {
        dataflow::operand_type(self, operand, block, statement, origin)
    }
}

/// Verifies one complete generation-local MIR program.
///
/// The first error is stable because reservations, functions, blocks, and
/// statements are visited in arena order and all set/map walks are ordered.
pub fn verify_mir(program: &MirProgram) -> Result<VerifiedMirProgram<'_>, MirVerifyError> {
    if let Some((function, reservation)) = program.undefined_reservations().next() {
        return Err(MirVerifyError {
            function,
            block: None,
            statement: None,
            origin: reservation.origin(),
            kind: Box::new(MirVerifyErrorKind::UndefinedFunctionReservation),
        });
    }

    for (function_id, function) in program.functions() {
        let verifier = FunctionVerifier::new(program, function_id, function);
        operations::verify_function_metadata(&verifier)?;
        let graph = cfg::analyze(&verifier)?;
        operations::verify_operations(&verifier, &graph)?;
        try_regions::verify(&verifier, &graph)?;
        dataflow::verify(&verifier, &graph)?;
    }

    Ok(VerifiedMirProgram { program })
}
