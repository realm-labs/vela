use std::collections::BTreeSet;

use vela_hir::body::HirBodyOwner;
use vela_hir::ids::{HirBodyId, HirExprId};

use crate::{
    MirAggregate, MirBuildError, MirEffect, MirOperand, MirPlace, MirSafepoint, MirSourceOrigin,
    MirStatement, MirStatementKind,
};

use super::core::{FunctionBuilder, value_type};

impl FunctionBuilder<'_> {
    pub(super) fn lower_lambda(
        &mut self,
        expression: HirExprId,
        body: HirBodyId,
        origin: MirSourceOrigin,
    ) -> Result<MirOperand, MirBuildError> {
        let target =
            self.input.targets().lambda(body).cloned().ok_or_else(|| {
                self.inconsistent(origin, "lambda expression has no compile target")
            })?;
        if target.expression != expression || target.parent != self.function.body() {
            return Err(self.inconsistent(
                origin,
                "lambda compile target disagrees with its executable parent or expression",
            ));
        }
        let hir_body = self
            .input
            .graph()
            .body(body)
            .ok_or(MirBuildError::MissingHirBody { body, origin })?;
        if !matches!(
            hir_body.owner,
            HirBodyOwner::Lambda {
                parent,
                expression: owner_expression,
            } if parent == self.body.id && owner_expression == expression
        ) {
            return Err(self.inconsistent(
                origin,
                "lambda expression disagrees with its Heavy HIR body owner",
            ));
        }

        let function = self.nested_function(body, origin)?;
        let mut captures = Vec::with_capacity(hir_body.captures.len());
        let mut seen = BTreeSet::new();
        for capture in &hir_body.captures {
            if capture.owner != body || !seen.insert(capture.local) {
                return Err(self.inconsistent(
                    origin,
                    "lambda capture order contains a foreign or duplicate source local",
                ));
            }
            captures.push(MirOperand::Local(self.local(capture.local, origin)?));
        }

        let destination = self.function.add_temp(
            value_type(self.input.analysis().expression(expression)),
            origin,
        );
        let safepoint = self.function.add_safepoint(MirSafepoint::new(origin));
        self.function.append_statement(
            self.current_block,
            MirStatement::new(
                origin,
                Some(MirPlace::temp(destination)),
                MirStatementKind::Allocate(MirAggregate::Closure { function, captures }),
                MirEffect::allocation(),
                Some(safepoint),
            ),
        )?;
        Ok(MirOperand::Temp(destination))
    }
}

#[cfg(test)]
#[path = "tests/closures_defaults.rs"]
mod tests;
