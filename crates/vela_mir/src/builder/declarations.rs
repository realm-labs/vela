use vela_hir::ids::{HirDeclId, HirExprId};
use vela_hir::module_graph::DeclarationKind;

use crate::{
    CompileStateStorage, MirBuildError, MirEffect, MirOperand, MirPlace, MirSourceOrigin,
    MirStateOperation, MirStatement, MirStatementKind,
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
        let state = self.input.targets().state(declaration);
        let constant = self.input.targets().evaluated_constant(declaration);
        if state.is_some() && constant.is_some() {
            return Err(self.inconsistent(
                origin,
                "declaration path has both state and evaluated constant compile targets",
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
                if state.is_some() {
                    return Err(self.inconsistent(
                        origin,
                        "const declaration path has a state compile target",
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
                        "state declaration path has an evaluated constant compile target",
                    ));
                }
                let state = state.ok_or_else(|| {
                    self.inconsistent(origin, "state declaration path has no state compile target")
                })?;
                let operation = match state.storage {
                    CompileStateStorage::Vm => MirStateOperation::ReadVmState { state: state.id },
                    CompileStateStorage::Extern => {
                        MirStateOperation::ReadExternState { state: state.id }
                    }
                };
                let destination = self.function.add_temp(result_type, origin);
                self.function.append_statement(
                    self.current_block,
                    MirStatement::new(
                        origin,
                        Some(MirPlace::temp(destination)),
                        MirStatementKind::State(operation),
                        MirEffect::state_read(),
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
                "declaration value path does not name a const or state",
            )),
        }
    }
}

#[cfg(test)]
#[path = "tests/declarations.rs"]
mod tests;
