use vela_hir::ids::HirExprId;

use crate::{
    MirBuildError, MirEffect, MirGuard, MirGuardAssumption, MirOperand, MirSourceOrigin,
    MirStatement, MirStatementKind,
};

use super::core::FunctionBuilder;

impl FunctionBuilder<'_> {
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
        let guard = self.function.add_guard(MirGuard {
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
