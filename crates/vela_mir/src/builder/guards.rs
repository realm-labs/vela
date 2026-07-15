use vela_hir::ids::{HirDeclId, HirExprId};

use crate::{
    CompileCallArguments, CompileCalleeTarget, MirBuildError, MirEffect, MirGuard,
    MirGuardAssumption, MirOperand, MirSourceOrigin, MirStatement, MirStatementKind,
};

use super::core::FunctionBuilder;

impl FunctionBuilder<'_> {
    pub(super) fn apply_state_guard(
        &mut self,
        declaration: HirDeclId,
        value: MirOperand,
        origin: MirSourceOrigin,
    ) -> Result<MirOperand, MirBuildError> {
        let Some(target) = self.input.targets().global_guard(declaration).cloned() else {
            return Ok(value);
        };
        let guard = self.function.add_guard(MirGuard {
            kind: crate::MirGuardKind::Contract,
            assumption: MirGuardAssumption::Type(target.contract),
            context: Some(target.context),
            origin,
        });
        self.function.append_statement(
            self.current_block,
            MirStatement::new(
                origin,
                None,
                MirStatementKind::GuardTrap {
                    value: value.clone(),
                    guard,
                },
                MirEffect::may_trap(),
                None,
            ),
        )?;
        Ok(value)
    }

    /// Apply the one authoritative contract boundary attached to an expression.
    ///
    /// This runs in the central expression wrapper immediately after the value
    /// exists. Callers can therefore evaluate later source operands without
    /// moving a contract trap past their effects. Parameter and return guards
    /// remain function-entry/exit metadata and never enter this path.
    pub(super) fn apply_expression_guard(
        &mut self,
        expression: HirExprId,
        value: MirOperand,
        origin: MirSourceOrigin,
    ) -> Result<MirOperand, MirBuildError> {
        if self.current_is_terminated()? {
            return Ok(value);
        }
        let Some(target) = self.input.targets().expression_guard(expression).cloned() else {
            return Ok(value);
        };
        if self.body.expressions.values().any(|candidate| {
            self.input.targets().call(candidate.id).is_some_and(|call| {
                matches!(
                    call.callee,
                    CompileCalleeTarget::ScriptFunction { .. }
                        | CompileCalleeTarget::ScriptMethod { .. }
                ) && matches!(
                    &call.arguments,
                    CompileCallArguments::Script { evaluation_order, .. }
                        if evaluation_order.contains(&expression)
                )
            })
        }) {
            // Script callees own parameter guards in code metadata. Keeping
            // the compile-target marker lets call lowering select checked
            // dispatch without also emitting a duplicate caller-side trap.
            return Ok(value);
        }
        let guard = self.function.add_guard(MirGuard {
            kind: crate::MirGuardKind::Contract,
            assumption: MirGuardAssumption::Type(target.contract),
            context: Some(target.context),
            origin,
        });
        self.function.append_statement(
            self.current_block,
            MirStatement::new(
                origin,
                None,
                MirStatementKind::GuardTrap {
                    value: value.clone(),
                    guard,
                },
                MirEffect::may_trap(),
                None,
            ),
        )?;
        Ok(value)
    }
}

#[cfg(test)]
#[path = "tests/guards.rs"]
mod tests;
