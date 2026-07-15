use vela_hir::ids::{HirDeclId, HirExprId};
use vela_hir::module_graph::DeclarationKind;

use crate::{
    MirBuildError, MirEffect, MirGlobalOperation, MirOperand, MirPlace, MirSourceOrigin,
    MirStatement, MirStatementKind,
};

use super::core::{FunctionBuilder, value_type};

impl FunctionBuilder<'_> {
    pub(super) fn lower_declaration_path(
        &mut self,
        expression: HirExprId,
        declaration: HirDeclId,
        origin: MirSourceOrigin,
    ) -> Result<MirOperand, MirBuildError> {
        let metadata =
            self.input.graph().declaration(declaration).ok_or_else(|| {
                self.inconsistent(origin, "declaration path has no HIR declaration")
            })?;
        let global = self.input.targets().global(declaration);
        let constant = self.input.targets().evaluated_constant(declaration);
        if global.is_some() && constant.is_some() {
            return Err(self.inconsistent(
                origin,
                "declaration path has both global and evaluated constant compile targets",
            ));
        }
        let result_type = self
            .input
            .analysis()
            .expression(expression)
            .map(|fact| value_type(Some(fact)))
            .ok_or_else(|| {
                self.inconsistent(
                    origin,
                    "declaration path has no authoritative analysis type",
                )
            })?;

        match metadata.kind {
            DeclarationKind::Const => {
                if global.is_some() {
                    return Err(self.inconsistent(
                        origin,
                        "const declaration path has a global compile target",
                    ));
                }
                let value = constant.cloned().ok_or_else(|| {
                    self.inconsistent(origin, "const declaration path has no evaluated constant")
                })?;
                self.lower_evaluated_constant(value, result_type, origin)
            }
            DeclarationKind::State => {
                if constant.is_some() {
                    return Err(self.inconsistent(
                        origin,
                        "global declaration path has an evaluated constant compile target",
                    ));
                }
                let global = global.ok_or_else(|| {
                    self.inconsistent(
                        origin,
                        "global declaration path has no global compile target",
                    )
                })?;
                let destination = self.function.add_temp(result_type, origin);
                self.function.append_statement(
                    self.current_block,
                    MirStatement::new(
                        origin,
                        Some(MirPlace::temp(destination)),
                        MirStatementKind::Global(MirGlobalOperation::Read { global: global.id }),
                        MirEffect::global_read(),
                        None,
                    ),
                )?;
                Ok(MirOperand::Temp(destination))
            }
            DeclarationKind::Function
            | DeclarationKind::Struct
            | DeclarationKind::Enum
            | DeclarationKind::Trait
            | DeclarationKind::Impl => Err(self.inconsistent(
                origin,
                "declaration value path does not name a const or global",
            )),
        }
    }
}

#[cfg(test)]
#[path = "tests/declarations.rs"]
mod tests;
