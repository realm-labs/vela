use vela_analysis::semantic_facts::OperatorTargetFact;
use vela_common::PrimitiveTag;
use vela_hir::body::{HirBinaryOp, HirElseBranch, HirIf, HirStmtKind};
use vela_hir::ids::{HirBlockId, HirExprId};

use crate::{
    MirBuildError, MirEffect, MirImmediate, MirLocalId, MirOperand, MirPlace, MirRvalue,
    MirSourceOrigin, MirStatement, MirTerminator, MirTerminatorKind, MirValueType,
};

use super::core::{FunctionBuilder, value_type};

impl FunctionBuilder<'_> {
    /// Lowers a block expression through one synthetic result local.
    ///
    /// Prefix statements use the central statement lowerer. The tail is
    /// selected before lowering so it is never evaluated once for effects and
    /// then again for its value.
    pub(super) fn lower_block_expression(
        &mut self,
        expression: HirExprId,
        block: HirBlockId,
        origin: MirSourceOrigin,
    ) -> Result<MirOperand, MirBuildError> {
        let result = self.add_result_local(expression, origin)?;
        self.lower_block_value_into(block, result, origin)?;
        Ok(MirOperand::Local(result))
    }

    /// Lowers an expression-valued `if` into a mutable synthetic join local.
    pub(super) fn lower_if_expression(
        &mut self,
        expression: HirExprId,
        value: &HirIf,
        origin: MirSourceOrigin,
    ) -> Result<MirOperand, MirBuildError> {
        let result = self.add_result_local(expression, origin)?;
        self.lower_if_root(value, Some(result), origin)?;
        Ok(MirOperand::Local(result))
    }

    /// Lowers a statement-position `if` through the same explicit CFG shape
    /// without manufacturing a value local.
    pub(super) fn lower_if_statement(
        &mut self,
        value: &HirIf,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        self.lower_if_root(value, None, origin)
    }

    /// Lowers `&&` and `||` without evaluating the right operand eagerly.
    /// Both operators produce a boolean even when their operands are dynamic.
    pub(super) fn lower_short_circuit(
        &mut self,
        expression: HirExprId,
        operation: HirBinaryOp,
        left: HirExprId,
        right: HirExprId,
        origin: MirSourceOrigin,
    ) -> Result<MirOperand, MirBuildError> {
        if !matches!(operation, HirBinaryOp::And | HirBinaryOp::Or) {
            return Err(self.inconsistent(
                origin,
                "short-circuit lowering received a non-logical operator",
            ));
        }
        match self.input.analysis().operator_target(expression) {
            Some(OperatorTargetFact::Binary(target)) if target == operation => {}
            Some(OperatorTargetFact::Binary(_)) => {
                return Err(self.inconsistent(
                    origin,
                    "analysis logical target disagrees with the HIR operator",
                ));
            }
            Some(OperatorTargetFact::Unresolved) => {
                return Err(self.inconsistent(origin, "unresolved logical operator reached MIR"));
            }
            Some(OperatorTargetFact::Dynamic) => {}
            Some(OperatorTargetFact::Unary(_) | OperatorTargetFact::Assignment(_)) => {
                return Err(self.inconsistent(
                    origin,
                    "analysis operator target has the wrong logical family",
                ));
            }
            None => {
                return Err(
                    self.inconsistent(origin, "logical expression has no analysis operator target")
                );
            }
        }

        let result = self
            .function
            .add_synthetic_local(MirValueType::Primitive(PrimitiveTag::Bool), origin);
        let left_origin = self.expression_origin_for_control(left)?;
        let left = self.lower_expression(left)?;
        if self.current_is_terminated()? {
            return Ok(MirOperand::Local(result));
        }

        let right_block = self.function.add_block();
        let short_block = self.function.add_block();
        let join_block = self.function.add_block();
        let (then_block, else_block, short_value) = match operation {
            HirBinaryOp::And => (right_block, short_block, false),
            HirBinaryOp::Or => (short_block, right_block, true),
            _ => unreachable!("logical operator was checked above"),
        };
        self.function.set_terminator(
            self.current_block,
            MirTerminator::new(
                left_origin,
                MirTerminatorKind::Branch {
                    condition: left,
                    then_block,
                    else_block,
                },
                MirEffect::PURE,
                None,
            ),
        )?;

        self.current_block = short_block;
        self.assign_result(
            result,
            MirRvalue::Use(MirOperand::Immediate(MirImmediate::Bool(short_value))),
            origin,
        )?;
        self.jump_current_to(join_block, origin)?;

        self.current_block = right_block;
        let right_origin = self.expression_origin_for_control(right)?;
        let right = self.lower_expression(right)?;
        if !self.current_is_terminated()? {
            self.assign_result(result, MirRvalue::Truthy { value: right }, right_origin)?;
            self.jump_current_to(join_block, origin)?;
        }

        self.current_block = join_block;
        Ok(MirOperand::Local(result))
    }

    fn lower_if_root(
        &mut self,
        value: &HirIf,
        destination: Option<MirLocalId>,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        let Some((then_block, else_block)) = self.begin_if(value, origin)? else {
            return Ok(());
        };
        let join_block = self.function.add_block();
        let reaches_join = self.lower_if_arms_into(
            value,
            destination,
            then_block,
            else_block,
            join_block,
            origin,
        )?;
        self.current_block = join_block;
        if !reaches_join {
            self.function.set_terminator(
                join_block,
                MirTerminator::new(
                    origin,
                    MirTerminatorKind::Unreachable,
                    MirEffect::PURE,
                    None,
                ),
            )?;
        }
        Ok(())
    }

    fn lower_if_into_continuation(
        &mut self,
        value: &HirIf,
        destination: Option<MirLocalId>,
        continuation: crate::MirBlockId,
        origin: MirSourceOrigin,
    ) -> Result<bool, MirBuildError> {
        let Some((then_block, else_block)) = self.begin_if(value, origin)? else {
            return Ok(false);
        };
        self.lower_if_arms_into(
            value,
            destination,
            then_block,
            else_block,
            continuation,
            origin,
        )
    }

    fn begin_if(
        &mut self,
        value: &HirIf,
        origin: MirSourceOrigin,
    ) -> Result<Option<(crate::MirBlockId, crate::MirBlockId)>, MirBuildError> {
        let condition = value
            .condition
            .ok_or_else(|| self.inconsistent(origin, "if expression has no condition"))?;
        let condition_origin = self.expression_origin_for_control(condition)?;
        let condition = self.lower_expression(condition)?;
        if self.current_is_terminated()? {
            return Ok(None);
        }
        let then_block = self.function.add_block();
        let else_block = self.function.add_block();
        self.function.set_terminator(
            self.current_block,
            MirTerminator::new(
                condition_origin,
                MirTerminatorKind::Branch {
                    condition,
                    then_block,
                    else_block,
                },
                MirEffect::PURE,
                None,
            ),
        )?;
        Ok(Some((then_block, else_block)))
    }

    fn lower_if_arms_into(
        &mut self,
        value: &HirIf,
        destination: Option<MirLocalId>,
        then_block: crate::MirBlockId,
        else_block: crate::MirBlockId,
        continuation: crate::MirBlockId,
        origin: MirSourceOrigin,
    ) -> Result<bool, MirBuildError> {
        self.current_block = then_block;
        let then_reaches =
            self.lower_block_arm_into(value.then_block, destination, continuation, origin)?;

        self.current_block = else_block;
        let else_reaches = match value.else_branch.as_ref() {
            Some(HirElseBranch::If(nested)) => {
                self.lower_if_into_continuation(nested, destination, continuation, origin)?
            }
            Some(HirElseBranch::Block(block)) => {
                self.lower_block_arm_into(Some(*block), destination, continuation, origin)?
            }
            None => self.lower_block_arm_into(None, destination, continuation, origin)?,
        };
        Ok(then_reaches || else_reaches)
    }

    fn lower_block_arm_into(
        &mut self,
        block: Option<HirBlockId>,
        destination: Option<MirLocalId>,
        continuation: crate::MirBlockId,
        origin: MirSourceOrigin,
    ) -> Result<bool, MirBuildError> {
        match (block, destination) {
            (Some(block), Some(destination)) => {
                self.lower_block_value_into(block, destination, origin)?;
            }
            (Some(block), None) => self.lower_block(block)?,
            (None, Some(destination)) => self.assign_result(
                destination,
                MirRvalue::Use(MirOperand::Immediate(MirImmediate::Unit)),
                origin,
            )?,
            (None, None) => {}
        }
        if self.current_is_terminated()? {
            return Ok(false);
        }
        self.jump_current_to(continuation, origin)?;
        Ok(true)
    }

    fn lower_block_value_into(
        &mut self,
        block: HirBlockId,
        destination: MirLocalId,
        origin: MirSourceOrigin,
    ) -> Result<bool, MirBuildError> {
        let block = self.body.blocks.get(&block).ok_or_else(|| {
            self.inconsistent(
                self.body_origin_for_control(),
                format!("missing HIR block {block:?}"),
            )
        })?;
        let mut statements = block.statements.clone();
        let tail = statements.last().and_then(|statement| {
            self.body
                .statements
                .get(statement)
                .map(|statement| (statement.id, statement.origin.span, statement.kind.clone()))
                .filter(|(_, _, kind)| {
                    matches!(
                        kind,
                        HirStmtKind::Expr {
                            expression: Some(_),
                            terminated: false,
                        } | HirStmtKind::If(_)
                            | HirStmtKind::Match(_)
                    )
                })
        });
        if tail.is_some() {
            statements.pop();
        }

        for statement in statements {
            self.lower_statement(statement)?;
            if self.current_is_terminated()? {
                return Ok(false);
            }
        }

        let Some((statement, span, tail)) = tail else {
            self.assign_result(
                destination,
                MirRvalue::Use(MirOperand::Immediate(MirImmediate::Unit)),
                origin,
            )?;
            return Ok(true);
        };
        let tail_origin = MirSourceOrigin::statement(self.body.id, statement, span);
        match tail {
            HirStmtKind::Expr {
                expression: Some(expression),
                terminated: false,
            } => {
                let value_origin = self.expression_origin_for_control(expression)?;
                let value = self.lower_expression(expression)?;
                if self.current_is_terminated()? {
                    return Ok(false);
                }
                self.assign_result(destination, MirRvalue::Use(value), value_origin)?;
            }
            HirStmtKind::If(value) => {
                self.lower_if_root(&value, Some(destination), tail_origin)?;
            }
            HirStmtKind::Match(_) => {
                return Err(self.unsupported(tail_origin, "match block value tail"));
            }
            _ => unreachable!("block value tail was filtered above"),
        }
        Ok(!self.current_is_terminated()?)
    }

    fn add_result_local(
        &mut self,
        expression: HirExprId,
        origin: MirSourceOrigin,
    ) -> Result<MirLocalId, MirBuildError> {
        let analysis = self.input.analysis();
        let fact = analysis
            .expression(expression)
            .ok_or_else(|| self.inconsistent(origin, "control-flow expression has no type fact"))?;
        Ok(self
            .function
            .add_synthetic_local(value_type(Some(fact)), origin))
    }

    fn assign_result(
        &mut self,
        destination: MirLocalId,
        value: MirRvalue,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        self.function.append_statement(
            self.current_block,
            MirStatement::assign(origin, MirPlace::local(destination), value),
        )?;
        Ok(())
    }

    fn jump_current_to(
        &mut self,
        target: crate::MirBlockId,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        self.function.set_terminator(
            self.current_block,
            MirTerminator::new(
                origin,
                MirTerminatorKind::Jump(target),
                MirEffect::PURE,
                None,
            ),
        )
    }

    fn expression_origin_for_control(
        &self,
        expression: HirExprId,
    ) -> Result<MirSourceOrigin, MirBuildError> {
        let record = self.body.expression(expression).ok_or_else(|| {
            self.inconsistent(
                self.body_origin_for_control(),
                format!("missing HIR expression {expression:?}"),
            )
        })?;
        Ok(MirSourceOrigin::expression(
            self.body.id,
            expression,
            record.origin.span,
        ))
    }

    fn body_origin_for_control(&self) -> MirSourceOrigin {
        MirSourceOrigin::body(self.body.id, self.body.origin.span)
    }
}
