#![cfg_attr(not(test), deny(clippy::wildcard_imports))]

//! Backend-neutral mid-level representation for Vela executable bodies.
//!
//! MIR is generation-local compiler state. It preserves stable HIR and
//! definition identities, but its arena IDs are deliberately not runtime or
//! serialization identities.

mod arena;
mod budget;
mod builder;
mod cfg;
mod contract;
mod dump;
mod effects;
mod facts;
mod function;
mod ids;
mod input;
mod jit;
mod liveness;
mod operations;
mod origin;
mod targets;
mod value;
mod verifier;

#[cfg(test)]
mod tests;

pub use budget::{MirBudgetClass, MirBudgetPoint, MirBudgetSchedule, MirBudgetSite};
pub use builder::build_mir;
pub use cfg::{
    MirBasicBlock, MirRangeStepMode, MirSwitchCase, MirSwitchValue, MirTerminator,
    MirTerminatorKind, MirTryContinue,
};
pub use contract::{HostTypeTarget, MirCallableKind, MirCallableKindSet, MirTypeContract};
pub use effects::{
    MirEffect, MirGuard, MirGuardAssumption, MirGuardContext, MirGuardKind, MirGuardLocation,
    MirLiveValue, MirSafepoint,
};
pub use facts::{
    MirFamilyFact, MirProgramPointFacts, MirShapeFact, MirShapeFieldIdentity, MirValueFact,
};
pub use function::{
    DebugLocalKind, MirDebugLocal, MirFunction, MirFunctionCapture, MirFunctionOwner,
    MirFunctionParameter, MirFunctionReservation, MirFunctionReturn, MirLiveRegion, MirLiveness,
    MirLocal, MirLocalKind, MirParameterKind, MirParameterSpec, MirProgram, MirTemp,
};
pub use ids::{
    MirBlockId, MirDebugLocalId, MirFunctionId, MirGuardId, MirLocalId, MirSafepointId,
    MirStatementId, MirTempId,
};
pub use input::{
    CompileCallArguments, CompileCallTarget, CompileCalleeTarget, CompileConstructorField,
    CompileConstructorTarget, CompileConstructorValue, CompileDynamicCallArgument,
    CompileDynamicConstructorField, CompileFieldTarget, CompileFunctionIdentity,
    CompileFunctionTarget, CompileFunctionTargets, CompileGuardKey, CompileGuardTarget,
    CompileHostIndexCapability, CompileHostPathSegment, CompileHostPathTarget,
    CompileLambdaParameterTarget, CompileLambdaTarget, CompileMemberTarget, CompileParameter,
    CompileParameterDefault, CompilePatternConstructorTarget, CompilePlacedCallArgument,
    CompilePlacedCallValue, CompilePositionalPolicy, CompileReflectionCall, CompileSignature,
    CompileStateTarget, CompileTargetKind, CompileTargetSnapshot, CompileTargetSnapshotBuilder,
    CompileTaskContinuationTarget, CompileTaskOperation, CompileTaskTarget, CompileTryFamily,
    CompileTryLayoutTarget, CompileTryTarget, DynamicMethodTarget, HostFieldTarget,
    HostMethodTarget, MethodExecutableTarget, MirBuildError, MirLoweringConfig, MirLoweringInput,
};
pub use jit::{MirJitEligibility, MirJitIneligibility, restricted_jit_eligibility};
pub use operations::{
    MirAggregate, MirAwaitOperation, MirCall, MirContextualBinaryOp, MirContextualNumericLiteral,
    MirDynamicArgument, MirDynamicBinaryOp, MirDynamicUnaryOp, MirFieldTarget, MirFormatPart,
    MirHostMutation, MirHostOperation, MirHostPath, MirHostPathSegment, MirIdentityOp, MirIndexKey,
    MirIndexOperation, MirIteratorOperation, MirLiteralSide, MirReflectionOperation,
    MirScriptArgument, MirScriptParameterGuardMode, MirStateOperation, MirStatement,
    MirStatementKind, MirTaskContinuation, MirTaskOperation,
};
pub use origin::{MirSourceNode, MirSourceOrigin};
pub use targets::{
    CompileFieldAccess, CompileFieldDescriptor, CompileFunctionAccess, CompileFunctionClass,
    CompileFunctionDescriptor, CompileMethodAccess, CompileMethodClass, CompileMethodDescriptor,
    CompileStateDescriptor, CompileStateStorage, CompileTypeClass, CompileTypeDescriptor,
    CompileVariantDescriptor, MirTargetTable,
};
pub use value::{
    MirBinaryOp, MirComparisonOp, MirConstantProvenance, MirEvaluatedConstant, MirImmediate,
    MirNumericBinaryOp, MirOperand, MirPatternPredicate, MirPlace, MirRvalue, MirUnaryOp,
    MirValueType,
};
pub use verifier::{
    MirBackendHandoff, MirBackendHandoffError, MirDebugAvailability, MirDestinationExpectation,
    MirFunctionAnalyses, MirRootLiveness, MirVerifyError, MirVerifyErrorKind, MirVerifyTarget,
    OwnedVerifiedMirBundle, OwnedVerifiedMirProgram, VerifiedMirProgram, verify_mir,
    verify_owned_mir,
};
