//! Backend-neutral mid-level representation for Vela executable bodies.
//!
//! MIR is generation-local compiler state. It preserves stable HIR and
//! definition identities, but its arena IDs are deliberately not runtime or
//! serialization identities.

mod arena;
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

#[cfg(test)]
mod tests;

pub use cfg::{
    MirBasicBlock, MirRangeStepMode, MirSwitchCase, MirSwitchValue, MirTerminator,
    MirTerminatorKind,
};
pub use contract::{HostTypeTarget, MirCallableKind, MirTypeContract};
pub use effects::{MirEffect, MirGuard, MirGuardAssumption, MirLiveValue, MirSafepoint};
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
    CompileMemberTarget, CompileParameter, CompileParameterDefault,
    CompilePatternConstructorTarget, CompilePositionalPolicy, CompileReflectionCall,
    CompileScriptCallArgument, CompileSignature, CompileTargetKind, CompileTargetSnapshot,
    CompileTargetSnapshotBuilder, CompileTryFamily, CompileTryLayoutTarget, CompileTryTarget,
    DynamicMethodTarget, HostFieldTarget, HostMethodTarget, MethodExecutableTarget, MirBuildError,
    MirLoweringConfig, MirLoweringInput,
};
pub use operations::{
    MirAggregate, MirCall, MirContextualBinaryOp, MirContextualNumericLiteral, MirDynamicArgument,
    MirDynamicBinaryOp, MirDynamicUnaryOp, MirFieldTarget, MirFormatPart, MirGlobalOperation,
    MirHostMutation, MirHostOperation, MirHostPath, MirHostPathSegment, MirIdentityOp, MirIndexKey,
    MirIndexOperation, MirIteratorOperation, MirLiteralSide, MirReflectionOperation,
    MirScriptArgument, MirScriptParameterGuardMode, MirStatement, MirStatementKind, MirTrapKind,
};
pub use origin::{MirSourceNode, MirSourceOrigin};
pub use targets::{
    CompileFieldAccess, CompileFieldDescriptor, CompileFunctionAccess, CompileFunctionClass,
    CompileFunctionDescriptor, CompileGlobalDescriptor, CompileMethodAccess, CompileMethodClass,
    CompileMethodDescriptor, CompileTypeClass, CompileTypeDescriptor, CompileVariantDescriptor,
    MirTargetTable,
};
pub use value::{
    MirBinaryOp, MirComparisonOp, MirEvaluatedConstant, MirImmediate, MirNumericBinaryOp,
    MirOperand, MirPlace, MirRvalue, MirUnaryOp, MirValueType,
};
