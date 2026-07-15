//! Structural and data-flow verification for backend-neutral MIR.
//!
//! Verification consumes MIR and its frozen target table only. It never
//! consults HIR or analysis again. A physical backend additionally requests a
//! handoff that requires computed liveness/debug/safepoint metadata.

pub(crate) mod cfg;
pub(crate) mod dataflow;
mod liveness;
mod operations;
mod try_regions;

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::sync::Arc;

use vela_def::{FieldId, FunctionId, MethodId, StateId, TypeId, VariantId};

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

/// Immutable, generation-retainable MIR together with the analyses sealed by
/// verification. Physical backends borrow this owner; they never receive the
/// mutable program used during construction.
#[derive(Clone, Debug, PartialEq)]
pub struct OwnedVerifiedMirProgram {
    program: Arc<MirProgram>,
    analyses: Arc<BTreeMap<MirFunctionId, MirFunctionAnalyses>>,
}

/// One compile generation's stable semantic roots mapped to their sealed,
/// generation-local MIR programs (including nested lambdas).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OwnedVerifiedMirBundle {
    roots: BTreeMap<FunctionId, Arc<OwnedVerifiedMirProgram>>,
}

impl OwnedVerifiedMirBundle {
    #[must_use]
    pub fn new(programs: impl IntoIterator<Item = OwnedVerifiedMirProgram>) -> Self {
        let mut roots = BTreeMap::new();
        for program in programs {
            let root = program
                .program()
                .functions()
                .find_map(|(_, function)| match function.owner() {
                    crate::MirFunctionOwner::Function(id) => Some(*id),
                    crate::MirFunctionOwner::Method(target) => Some(target.function),
                    crate::MirFunctionOwner::Lambda { .. } => None,
                })
                .expect("verified production MIR has one stable executable root");
            assert!(
                roots.insert(root, Arc::new(program)).is_none(),
                "verified MIR bundle contains duplicate stable root {root:?}"
            );
        }
        Self { roots }
    }

    pub fn roots(&self) -> impl Iterator<Item = (FunctionId, &Arc<OwnedVerifiedMirProgram>)> {
        self.roots.iter().map(|(id, program)| (*id, program))
    }

    #[must_use]
    pub fn root(&self, id: FunctionId) -> Option<&Arc<OwnedVerifiedMirProgram>> {
        self.roots.get(&id)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirRootLiveness {
    pub live_before_safepoint: BTreeMap<MirSafepointId, BTreeSet<crate::MirLiveValue>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirDebugAvailability {
    pub locals: BTreeMap<crate::MirDebugLocalId, crate::MirLiveRegion>,
    pub block_entry: BTreeMap<MirBlockId, BTreeSet<crate::MirDebugLocalId>>,
    pub statement_before: BTreeMap<MirStatementId, BTreeSet<crate::MirDebugLocalId>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MirFunctionAnalyses {
    pub value_liveness: crate::MirLiveness,
    pub root_liveness: MirRootLiveness,
    pub debug_availability: MirDebugAvailability,
    pub facts: crate::MirProgramPointFacts,
    pub budget: crate::MirBudgetSchedule,
}

impl OwnedVerifiedMirProgram {
    #[must_use]
    pub fn program(&self) -> &MirProgram {
        &self.program
    }

    #[must_use]
    pub fn analyses(&self, function: MirFunctionId) -> Option<&MirFunctionAnalyses> {
        self.analyses.get(&function)
    }

    pub fn backend_handoff(&self) -> Result<MirBackendHandoff<'_>, MirBackendHandoffError> {
        require_computed_liveness(&self.program)?;
        Ok(MirBackendHandoff {
            program: &self.program,
            analyses: &self.analyses,
        })
    }
}

impl<'a> VerifiedMirProgram<'a> {
    #[must_use]
    pub const fn program(self) -> &'a MirProgram {
        self.program
    }
}

fn require_computed_liveness(program: &MirProgram) -> Result<(), MirBackendHandoffError> {
    for (function, body) in program.functions() {
        if !body.liveness().is_computed() {
            return Err(MirBackendHandoffError::MissingLiveness {
                function,
                origin: body.origin(),
            });
        }
    }
    Ok(())
}

/// Complete verifier-proven MIR input for a physical backend.
///
/// The contained program owns backend-neutral targets, logical values, CFG,
/// effects, guards, source/debug records, and computed live metadata. The
/// wrapper cannot be constructed without first passing [`verify_mir`].
#[derive(Clone, Copy, Debug)]
pub struct MirBackendHandoff<'a> {
    program: &'a MirProgram,
    analyses: &'a BTreeMap<MirFunctionId, MirFunctionAnalyses>,
}

impl<'a> MirBackendHandoff<'a> {
    #[must_use]
    pub const fn program(self) -> &'a MirProgram {
        self.program
    }

    #[must_use]
    pub fn analyses(self, function: MirFunctionId) -> Option<&'a MirFunctionAnalyses> {
        self.analyses.get(&function)
    }

    #[must_use]
    pub fn all_analyses(self) -> &'a BTreeMap<MirFunctionId, MirFunctionAnalyses> {
        self.analyses
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirBackendHandoffError {
    MissingLiveness {
        function: MirFunctionId,
        origin: MirSourceOrigin,
    },
}

impl fmt::Display for MirBackendHandoffError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingLiveness { function, .. } => {
                write!(
                    formatter,
                    "MIR backend handoff requires computed liveness for {function}"
                )
            }
        }
    }
}

impl Error for MirBackendHandoffError {}

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
    Global(StateId),
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
    InvalidLivenessMetadata(String),
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
    DuplicateSafepointUse {
        safepoint: MirSafepointId,
    },
    OrphanSafepoint(MirSafepointId),
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
    facts: Option<&'a crate::MirProgramPointFacts>,
}

impl<'a> FunctionVerifier<'a> {
    fn new(program: &'a MirProgram, function_id: MirFunctionId, function: &'a MirFunction) -> Self {
        Self {
            program,
            function_id,
            function,
            facts: None,
        }
    }

    fn with_facts(
        program: &'a MirProgram,
        function_id: MirFunctionId,
        function: &'a MirFunction,
        facts: &'a crate::MirProgramPointFacts,
    ) -> Self {
        Self {
            program,
            function_id,
            function,
            facts: Some(facts),
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
        if let (Some(statement), Some(facts)) = (statement, self.facts)
            && let Some(fact) = facts.operand_before(statement, operand)
        {
            return Ok(fact.value_type);
        }
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
        let structural = FunctionVerifier::new(program, function_id, function);
        operations::verify_function_metadata(&structural)?;
        let graph = cfg::analyze(&structural)?;
        let facts = crate::facts::analyze(program, function);
        let verifier = FunctionVerifier::with_facts(program, function_id, function, &facts);
        operations::verify_operations(&verifier, &graph)?;
        try_regions::verify(&verifier, &graph)?;
        dataflow::verify(&verifier, &graph)?;
        liveness::verify(&verifier)?;
    }

    Ok(VerifiedMirProgram { program })
}

/// Consumes a completed MIR program and seals the exact verified generation.
/// This is the production backend boundary; the borrowed verifier remains
/// useful to corruption tests that need to retain their fixture.
pub fn verify_owned_mir(program: MirProgram) -> Result<OwnedVerifiedMirProgram, MirVerifyError> {
    verify_mir(&program)?;
    let analyses = program
        .functions()
        .map(|(id, function)| {
            let mut analyses = crate::liveness::sealed_analyses(function);
            analyses.facts = crate::facts::analyze(&program, function);
            let verifier = FunctionVerifier::new(&program, id, function);
            let graph = cfg::analyze(&verifier)
                .expect("owned MIR was structurally verified before analyses were sealed");
            analyses.budget = crate::budget::analyze(function, &graph);
            (id, analyses)
        })
        .collect();
    Ok(OwnedVerifiedMirProgram {
        program: Arc::new(program),
        analyses: Arc::new(analyses),
    })
}
