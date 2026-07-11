//! Backend-neutral mid-level representation for Vela executable bodies.
//!
//! MIR is generation-local compiler state. It preserves stable HIR and
//! definition identities, but its arena IDs are deliberately not runtime or
//! serialization identities.

mod arena;
mod builder;
mod cfg;
mod contract;
mod dump;
mod effects;
mod function;
mod ids;
mod input;
mod operations;
mod origin;
mod targets;
mod value;
mod verifier;

#[cfg(test)]
mod tests;

pub use builder::build_mir;
pub use cfg::{
    MirBasicBlock, MirRangeStepMode, MirSwitchCase, MirSwitchValue, MirTerminator,
    MirTerminatorKind, MirTryContinue,
};
pub use contract::{HostTypeTarget, MirCallableKind, MirTypeContract};
pub use effects::{
    MirEffect, MirGuard, MirGuardAssumption, MirGuardContext, MirGuardLocation, MirLiveValue,
    MirSafepoint,
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
    CompileFunctionTarget, CompileFunctionTargets, CompileGlobalTarget, CompileGuardKey,
    CompileGuardTarget, CompileHostIndexCapability, CompileHostPathSegment, CompileHostPathTarget,
    CompileLambdaParameterTarget, CompileLambdaTarget, CompileMemberTarget, CompileParameter,
    CompileParameterDefault, CompilePatternConstructorTarget, CompilePlacedCallArgument,
    CompilePlacedCallValue, CompilePositionalPolicy, CompileReflectionCall, CompileSignature,
    CompileTargetKind, CompileTargetSnapshot, CompileTargetSnapshotBuilder, CompileTryFamily,
    CompileTryLayoutTarget, CompileTryTarget, DynamicMethodTarget, HostFieldTarget,
    HostMethodTarget, MethodExecutableTarget, MirBuildError, MirLoweringConfig, MirLoweringInput,
};
pub use operations::{
    MirAggregate, MirCall, MirContextualBinaryOp, MirContextualNumericLiteral, MirDynamicArgument,
    MirDynamicBinaryOp, MirDynamicUnaryOp, MirFieldTarget, MirFormatPart, MirGlobalOperation,
    MirHostMutation, MirHostOperation, MirHostPath, MirHostPathSegment, MirIdentityOp, MirIndexKey,
    MirIndexOperation, MirIteratorOperation, MirLiteralSide, MirReflectionOperation,
    MirScriptArgument, MirScriptParameterGuardMode, MirStatement, MirStatementKind,
};
pub use origin::{MirSourceNode, MirSourceOrigin};
pub use targets::{
    CompileFieldAccess, CompileFieldDescriptor, CompileFunctionAccess, CompileFunctionClass,
    CompileFunctionDescriptor, CompileGlobalDescriptor, CompileMethodAccess, CompileMethodClass,
    CompileMethodDescriptor, CompileTypeClass, CompileTypeDescriptor, CompileVariantDescriptor,
    MirTargetTable,
};
pub use value::{
    MirBinaryOp, MirComparisonOp, MirConstantProvenance, MirEvaluatedConstant, MirImmediate,
    MirNumericBinaryOp, MirOperand, MirPatternPredicate, MirPlace, MirRvalue, MirUnaryOp,
    MirValueType,
};
pub use verifier::{
    MirDestinationExpectation, MirVerifyError, MirVerifyErrorKind, MirVerifyTarget,
    VerifiedMirProgram, verify_mir,
};
