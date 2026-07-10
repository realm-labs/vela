//! Heavy-HIR-to-MIR construction.
//!
//! This workspace-internal crate API is not routed from any production compile
//! entrypoint. Complete-function tests can therefore exercise the real
//! semantic input boundary while Phase 2 grows by execution responsibility.

mod core;
mod literals;
mod operators;

use crate::{MirBuildError, MirFunctionOwner, MirLoweringInput, MirProgram, MirSourceOrigin};

pub fn build_mir(input: MirLoweringInput<'_>) -> Result<MirProgram, MirBuildError> {
    let body = input
        .graph()
        .body(input.body())
        .ok_or(MirBuildError::MissingCompilationRoot {
            function: input.function(),
            body: input.body(),
        })?;
    let origin = MirSourceOrigin::body(body.id, body.origin.span);
    let owner = match input.identity() {
        crate::CompileFunctionIdentity::Function(function) => MirFunctionOwner::Function(function),
        crate::CompileFunctionIdentity::Method(method) => MirFunctionOwner::Method(method),
    };
    let mut program = MirProgram::new(input.targets().target_table().clone());
    let reservation = program.reserve_function(body.id, owner.clone(), origin)?;
    let function = core::FunctionBuilder::new(input, owner)?.build()?;
    program.define_function(reservation, function)?;
    Ok(program)
}
