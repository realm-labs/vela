use vela_hir::body::{HirCall, HirExprKind};
use vela_hir::ids::HirExprId;

use crate::{
    CompileCallTarget, CompileCalleeTarget, CompileFunctionClass, CompileReflectionCall,
    MirAwaitOperation, MirBuildError, MirEffect, MirImmediate, MirOperand, MirPlace,
    MirReflectionOperation, MirSafepoint, MirSourceOrigin, MirStatement, MirStatementKind,
};

use super::calls::AwaitContext;
use super::core::{FunctionBuilder, value_type};

impl FunctionBuilder<'_> {
    pub(super) fn lower_reflection_call(
        &mut self,
        expression: HirExprId,
        call: &HirCall,
        target: &CompileCallTarget,
        origin: MirSourceOrigin,
        await_context: Option<AwaitContext>,
    ) -> Result<MirOperand, MirBuildError> {
        let CompileCalleeTarget::Reflection {
            operation,
            function,
            debug_name,
        } = &target.callee
        else {
            return Err(self.inconsistent(origin, "reflection lowering received another callee"));
        };
        let expected_name = match operation {
            CompileReflectionCall::Read => "reflect::get",
            CompileReflectionCall::Write => "reflect::set",
            CompileReflectionCall::Call => "reflect::call",
        };
        if debug_name != expected_name
            || self.static_callee_name(call.callee, origin)? != *debug_name
        {
            return Err(self.inconsistent(
                origin,
                "reflection operation or debug name disagrees with the HIR callee",
            ));
        }
        let descriptor = self
            .input
            .targets()
            .function_descriptor(*function)
            .cloned()
            .ok_or_else(|| {
                self.inconsistent(origin, "reflection call target has no function descriptor")
            })?;
        if descriptor.id != *function
            || !matches!(
                descriptor.class,
                CompileFunctionClass::Native | CompileFunctionClass::Registry
            )
            || descriptor.canonical_symbol != *debug_name
            || descriptor.debug_name != *debug_name
        {
            return Err(self.inconsistent(
                origin,
                "reflection function descriptor disagrees with the placed target",
            ));
        }
        let arguments =
            self.lower_external_arguments(&target.arguments, &descriptor.signature, origin)?;
        if self.current_is_terminated()? {
            return Ok(MirOperand::Immediate(MirImmediate::Unit));
        }
        let operation = match operation {
            CompileReflectionCall::Read => {
                let [target, member] = arguments.as_slice() else {
                    return Err(self.inconsistent(
                        origin,
                        "reflection read requires exactly two evaluated arguments",
                    ));
                };
                MirReflectionOperation::Read {
                    function: *function,
                    target: target.clone(),
                    member: member.clone(),
                }
            }
            CompileReflectionCall::Write => {
                let [target, member, value] = arguments.as_slice() else {
                    return Err(self.inconsistent(
                        origin,
                        "reflection write requires exactly three evaluated arguments",
                    ));
                };
                MirReflectionOperation::Write {
                    function: *function,
                    target: target.clone(),
                    member: member.clone(),
                    value: value.clone(),
                }
            }
            CompileReflectionCall::Call => {
                let Some((target, tail)) = arguments.split_first() else {
                    return Err(self.inconsistent(
                        origin,
                        "reflection call requires at least one evaluated argument",
                    ));
                };
                MirReflectionOperation::Call {
                    function: *function,
                    target: target.clone(),
                    tail: tail.to_vec(),
                }
            }
        };
        let intrinsic_effect = match operation {
            MirReflectionOperation::Read { .. } => MirEffect::reflection_read(),
            MirReflectionOperation::Write { .. } => MirEffect::reflection_write(),
            MirReflectionOperation::Call { .. } => MirEffect::reflection_call(),
        };
        let effect = intrinsic_effect.union(descriptor.signature.effect);
        if let Some(context) = await_context {
            let analysis = self.input.analysis();
            let result = analysis.expression(expression).ok_or_else(|| {
                self.inconsistent(origin, "reflection expression has no analysis type fact")
            })?;
            self.append_await_operation(
                context,
                MirAwaitOperation::Reflect(operation),
                value_type(Some(result)),
                effect,
            )
        } else {
            self.append_reflection(expression, operation, effect, origin)
        }
    }

    fn static_callee_name(
        &self,
        expression: HirExprId,
        origin: MirSourceOrigin,
    ) -> Result<String, MirBuildError> {
        let record = self.body.expression(expression).ok_or_else(|| {
            self.inconsistent(origin, "reflection call references a missing HIR callee")
        })?;
        match &record.kind {
            HirExprKind::Path(path) => self
                .body
                .paths
                .get(path)
                .map(|path| path.path.join("::"))
                .ok_or_else(|| self.inconsistent(origin, "reflection callee has no HIR path")),
            HirExprKind::Paren {
                expression: Some(inner),
            } => self.static_callee_name(*inner, origin),
            _ => Err(self.inconsistent(origin, "reflection callee is not a static HIR path")),
        }
    }

    fn append_reflection(
        &mut self,
        expression: HirExprId,
        operation: MirReflectionOperation,
        effect: MirEffect,
        origin: MirSourceOrigin,
    ) -> Result<MirOperand, MirBuildError> {
        let analysis = self.input.analysis();
        let result = analysis.expression(expression).ok_or_else(|| {
            self.inconsistent(origin, "reflection expression has no analysis type fact")
        })?;
        let destination = self.function.add_temp(value_type(Some(result)), origin);
        let safepoint = effect
            .requires_safepoint()
            .then(|| self.function.add_safepoint(MirSafepoint::new(origin)));
        self.function.append_statement(
            self.current_block,
            MirStatement::new(
                origin,
                Some(MirPlace::temp(destination)),
                MirStatementKind::Reflect(operation),
                effect,
                safepoint,
            ),
        )?;
        Ok(MirOperand::Temp(destination))
    }
}

#[cfg(test)]
#[path = "tests/reflection.rs"]
mod tests;
