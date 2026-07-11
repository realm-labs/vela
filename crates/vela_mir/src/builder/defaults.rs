use vela_hir::body::{HirBody, HirBodyOwner, HirBodyRoot};

use crate::{
    CompileGuardTarget, MirBuildError, MirEffect, MirGuard, MirGuardAssumption, MirImmediate,
    MirOperand, MirPlace, MirRvalue, MirSourceOrigin, MirStatement, MirStatementKind,
    MirTerminator, MirTerminatorKind, MirValueType,
};

use super::core::FunctionBuilder;

impl<'a> FunctionBuilder<'a> {
    pub(super) fn lower_parameter_defaults(&mut self) -> Result<(), MirBuildError> {
        let parameters = self.function.parameters().to_vec();
        for parameter in parameters {
            let Some(default_body) = parameter.default_body else {
                continue;
            };
            let body =
                self.input
                    .graph()
                    .body(default_body)
                    .ok_or(MirBuildError::MissingHirBody {
                        body: default_body,
                        origin: parameter.origin,
                    })?;
            let crate::MirParameterKind::Explicit(parameter_id) = parameter.kind else {
                return Err(self.inconsistent(
                    parameter.origin,
                    "method receiver unexpectedly owns a parameter default body",
                ));
            };
            if !matches!(
                body.owner,
                HirBodyOwner::ParameterDefault {
                    parent,
                    parameter: owner_parameter,
                } if parent == self.function.body()
                    && parameter_id == owner_parameter
            ) {
                return Err(self.inconsistent(
                    parameter.origin,
                    "parameter default body disagrees with its owning function parameter",
                ));
            }
            self.lower_parameter_default(&parameter, body)?;
        }
        Ok(())
    }

    fn lower_parameter_default(
        &mut self,
        parameter: &crate::MirFunctionParameter,
        body: &'a HirBody,
    ) -> Result<(), MirBuildError> {
        let origin = parameter.origin;
        let missing = self.function.add_temp(
            MirValueType::Primitive(vela_common::PrimitiveTag::Bool),
            origin,
        );
        self.function.append_statement(
            self.current_block,
            MirStatement::assign(
                origin,
                MirPlace::temp(missing),
                MirRvalue::IsMissing {
                    value: MirOperand::Local(parameter.storage),
                },
            ),
        )?;
        let evaluate = self.function.add_block();
        let next = self.function.add_block();
        self.function.set_terminator(
            self.current_block,
            MirTerminator::new(
                origin,
                MirTerminatorKind::Branch {
                    condition: MirOperand::Temp(missing),
                    then_block: evaluate,
                    else_block: next,
                },
                MirEffect::PURE,
                None,
            ),
        )?;

        self.current_block = evaluate;
        let previous = std::mem::replace(&mut self.body, body);
        let lowered = self.lower_default_body_value(parameter);
        self.body = previous;
        let (value, value_origin, guard) = lowered?;
        if !self.current_is_terminated()? {
            if let Some(guard) = guard {
                self.append_default_guard(value.clone(), guard, value_origin)?;
            }
            self.function.append_statement(
                self.current_block,
                MirStatement::assign(
                    value_origin,
                    MirPlace::local(parameter.storage),
                    MirRvalue::Use(value),
                ),
            )?;
            self.function.set_terminator(
                self.current_block,
                MirTerminator::new(
                    value_origin,
                    MirTerminatorKind::Jump(next),
                    MirEffect::PURE,
                    None,
                ),
            )?;
        }
        self.current_block = next;
        Ok(())
    }

    fn lower_default_body_value(
        &mut self,
        parameter: &crate::MirFunctionParameter,
    ) -> Result<(MirOperand, MirSourceOrigin, Option<CompileGuardTarget>), MirBuildError> {
        match self.body.root {
            HirBodyRoot::Expr(expression) => {
                let origin = self.expression_origin(expression)?;
                let value = self.lower_expression(expression)?;
                let guard = self.input.targets().expression_guard(expression).cloned();
                Ok((value, origin, guard))
            }
            HirBodyRoot::Block(block) => {
                let value_type = self
                    .function
                    .local(parameter.storage)
                    .map(|local| local.value_type)
                    .ok_or(MirBuildError::MissingLocal {
                        local: parameter.storage,
                        origin: parameter.origin,
                    })?;
                let result = self
                    .function
                    .add_synthetic_local(value_type, self.body_origin());
                self.lower_block_value_into(block, result, self.body_origin())?;
                Ok((MirOperand::Local(result), self.body_origin(), None))
            }
            HirBodyRoot::Empty => Ok((
                MirOperand::Immediate(MirImmediate::Unit),
                self.body_origin(),
                None,
            )),
        }
    }

    fn append_default_guard(
        &mut self,
        value: MirOperand,
        target: CompileGuardTarget,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
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
                MirStatementKind::GuardTrap { value, guard },
                MirEffect::may_trap(),
                None,
            ),
        )?;
        Ok(())
    }

    pub(super) fn lower_owning_body(&mut self) -> Result<(), MirBuildError> {
        match self.body.root {
            HirBodyRoot::Block(block) => {
                self.lower_block(block)?;
                self.finish_open_block(None, self.body_origin())
            }
            HirBodyRoot::Expr(expression) => {
                let value = self.lower_expression(expression)?;
                self.finish_open_block(Some(value), self.expression_origin(expression)?)
            }
            HirBodyRoot::Empty => self.finish_open_block(None, self.body_origin()),
        }
    }
}
