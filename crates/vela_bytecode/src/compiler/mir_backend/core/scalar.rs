use vela_mir::{
    MirBinaryOp, MirImmediate, MirNumericBinaryOp, MirOperand, MirPlace, MirRvalue,
    MirStatementKind, MirTerminatorKind, MirUnaryOp,
};

use super::support::i64_compare;
use super::{
    FunctionBackend, InstructionOffset, MirBackendError, PendingScalarBlock, PendingScalarExit,
    Register, UnlinkedInstructionKind,
};

impl FunctionBackend<'_> {
    pub(super) fn scalar_block(
        &mut self,
        selected: &super::super::selection::ScalarBlockSelection,
        terminator: &MirTerminatorKind,
        terminator_span: vela_common::Span,
    ) -> Result<(), MirBackendError> {
        let mut operations = Vec::with_capacity(selected.statements().len());
        let mut source_points = Vec::with_capacity(selected.statements().len() + 3);
        let mut mir_budget_sites = Vec::new();
        for (operation_index, statement_id) in selected.statements().iter().enumerate() {
            let statement = self
                .function
                .statement(*statement_id)
                .ok_or(MirBackendError::MissingStatement)?;
            let source = crate::ScalarSourcePointId::new(source_points.len());
            source_points.push(statement.origin.span);
            let budget = self.budget.statement_before(*statement_id);
            if let Some(point) = budget {
                mir_budget_sites.push(crate::scalar_plan::ScalarMirBudgetSite {
                    site: vela_mir::MirBudgetSite::StatementBefore(*statement_id),
                    point,
                    location: crate::scalar_plan::ScalarBudgetLocation::Operation(operation_index),
                });
            }
            operations.push(crate::ScalarOp {
                kind: self.scalar_operation(*statement_id, statement)?,
                source,
                execution_units: budget.map_or(0, |point| point.units),
            });
        }
        let exit_source = crate::ScalarSourcePointId::new(source_points.len());
        source_points.push(terminator_span);
        let exit = match terminator {
            MirTerminatorKind::Jump(target) => PendingScalarExit::Jump(*target),
            MirTerminatorKind::Branch {
                condition,
                then_block,
                else_block,
            } => PendingScalarExit::BoolBranch {
                condition: self.scalar_operand(condition)?,
                passed: *then_block,
                failed: *else_block,
            },
            _ => return Err(MirBackendError::MissingTarget("scalar block exit")),
        };
        let instruction = InstructionOffset(self.code.instructions.len());
        let plan = crate::ScalarBlockPlanId::new(
            self.code.scalar_blocks.len() + self.pending_scalar_blocks.len(),
        );
        self.emit(
            UnlinkedInstructionKind::RunScalarBlock { plan },
            terminator_span,
        );
        let exit_budget = self.budget.terminator_before(selected.block());
        if let Some(point) = exit_budget {
            mir_budget_sites.push(crate::scalar_plan::ScalarMirBudgetSite {
                site: vela_mir::MirBudgetSite::TerminatorBefore(selected.block()),
                point,
                location: crate::scalar_plan::ScalarBudgetLocation::Exit,
            });
        }
        self.pending_scalar_blocks.push(PendingScalarBlock {
            instruction,
            block: selected.block(),
            statements: selected.statements().to_vec().into_boxed_slice(),
            operations: operations.into_boxed_slice(),
            exit,
            exit_source,
            exit_execution_units: exit_budget.map_or(0, |point| point.units),
            source_points,
            mir_budget_sites,
        });
        Ok(())
    }

    fn scalar_operation(
        &self,
        statement_id: vela_mir::MirStatementId,
        statement: &vela_mir::MirStatement,
    ) -> Result<crate::ScalarOpKind, MirBackendError> {
        let dst = self.scalar_place(
            statement
                .destination
                .ok_or(MirBackendError::MissingDestination)?,
        );
        match &statement.kind {
            MirStatementKind::Assign(MirRvalue::Use(operand)) => match operand {
                MirOperand::Immediate(value) => Ok(crate::ScalarOpKind::LoadScalar {
                    dst,
                    value: scalar_constant(*value)?,
                }),
                MirOperand::Local(_) | MirOperand::Temp(_) => Ok(crate::ScalarOpKind::Move {
                    dst,
                    src: self.scalar_operand(operand)?,
                }),
            },
            MirStatementKind::Assign(MirRvalue::Constant { value, .. }) => {
                Ok(crate::ScalarOpKind::LoadScalar {
                    dst,
                    value: scalar_constant(*value)?,
                })
            }
            MirStatementKind::Unary {
                operation: MirUnaryOp::NotBool,
                operand,
            } => Ok(crate::ScalarOpKind::BoolNot {
                dst,
                src: self.scalar_operand(operand)?,
            }),
            MirStatementKind::Binary {
                operation,
                left,
                right,
            } => {
                let lhs = self.scalar_operand(left)?;
                let rhs = self.scalar_operand(right)?;
                let immediate = self
                    .facts
                    .operand_before(statement_id, right)
                    .and_then(|fact| fact.immediate)
                    .and_then(i64_immediate);
                match operation {
                    MirBinaryOp::Numeric {
                        operation: MirNumericBinaryOp::Add,
                        kind: vela_common::NumericTag::I64,
                    } => Ok(immediate
                        .map_or(crate::ScalarOpKind::I64Add { dst, lhs, rhs }, |imm| {
                            crate::ScalarOpKind::I64AddImm { dst, lhs, imm }
                        })),
                    MirBinaryOp::Numeric {
                        operation: MirNumericBinaryOp::Subtract,
                        kind: vela_common::NumericTag::I64,
                    } => Ok(immediate
                        .map_or(crate::ScalarOpKind::I64Sub { dst, lhs, rhs }, |imm| {
                            crate::ScalarOpKind::I64SubImm { dst, lhs, imm }
                        })),
                    MirBinaryOp::Numeric {
                        operation: MirNumericBinaryOp::Multiply,
                        kind: vela_common::NumericTag::I64,
                    } => Ok(immediate
                        .map_or(crate::ScalarOpKind::I64Mul { dst, lhs, rhs }, |imm| {
                            crate::ScalarOpKind::I64MulImm { dst, lhs, imm }
                        })),
                    MirBinaryOp::Numeric {
                        operation: MirNumericBinaryOp::Remainder,
                        kind: vela_common::NumericTag::I64,
                    } => Ok(immediate
                        .map_or(crate::ScalarOpKind::I64Rem { dst, lhs, rhs }, |imm| {
                            crate::ScalarOpKind::I64RemImm { dst, lhs, imm }
                        })),
                    MirBinaryOp::Compare {
                        operation,
                        kind: vela_common::PrimitiveTag::I64,
                    } => Ok(immediate.map_or(
                        crate::ScalarOpKind::I64Compare {
                            dst,
                            op: i64_compare(*operation),
                            lhs,
                            rhs,
                        },
                        |imm| crate::ScalarOpKind::I64CompareImm {
                            dst,
                            op: i64_compare(*operation),
                            lhs,
                            imm,
                        },
                    )),
                    _ => Err(MirBackendError::MissingTarget("scalar operation")),
                }
            }
            _ => Err(MirBackendError::MissingTarget("scalar operation")),
        }
    }

    fn scalar_operand(&self, operand: &MirOperand) -> Result<Register, MirBackendError> {
        match operand {
            MirOperand::Local(local) => self
                .locals
                .get(local)
                .copied()
                .ok_or(MirBackendError::MissingTarget("scalar local register")),
            MirOperand::Temp(temp) => self
                .temps
                .get(temp)
                .copied()
                .ok_or(MirBackendError::MissingTarget("scalar temporary register")),
            MirOperand::Immediate(_) => Err(MirBackendError::MissingTarget(
                "scalar immediate register operand",
            )),
        }
    }

    fn scalar_place(&self, place: MirPlace) -> Register {
        self.place(place)
    }

    pub(super) fn finalize_scalar_blocks(&mut self) -> Result<(), MirBackendError> {
        for pending in std::mem::take(&mut self.pending_scalar_blocks) {
            debug_assert_eq!(
                self.code.instructions[pending.instruction.0].kind,
                UnlinkedInstructionKind::RunScalarBlock {
                    plan: crate::ScalarBlockPlanId::new(self.code.scalar_blocks.len())
                }
            );
            let mut source_points = pending.source_points;
            let mut mir_budget_sites = pending.mir_budget_sites;
            let exit = match pending.exit {
                PendingScalarExit::Jump(target) => {
                    crate::ScalarExitKind::Jump(self.scalar_target(
                        pending.block,
                        target,
                        crate::scalar_plan::ScalarBudgetLocation::JumpEdge,
                        &mut source_points,
                        &mut mir_budget_sites,
                    )?)
                }
                PendingScalarExit::BoolBranch {
                    condition,
                    passed,
                    failed,
                } => crate::ScalarExitKind::BoolBranch {
                    condition,
                    passed: self.scalar_target(
                        pending.block,
                        passed,
                        crate::scalar_plan::ScalarBudgetLocation::PassedEdge,
                        &mut source_points,
                        &mut mir_budget_sites,
                    )?,
                    failed: self.scalar_target(
                        pending.block,
                        failed,
                        crate::scalar_plan::ScalarBudgetLocation::FailedEdge,
                        &mut source_points,
                        &mut mir_budget_sites,
                    )?,
                },
            };
            self.code.scalar_blocks.push(
                crate::ScalarBlockPlan::new(
                    pending.operations,
                    crate::ScalarExit {
                        kind: exit,
                        source: pending.exit_source,
                        execution_units: pending.exit_execution_units,
                    },
                    source_points.into_boxed_slice(),
                )
                .with_mir_coverage(pending.statements, pending.block)
                .with_mir_budget_sites(mir_budget_sites.into_boxed_slice()),
            );
        }
        Ok(())
    }

    fn scalar_target(
        &self,
        from: vela_mir::MirBlockId,
        target: vela_mir::MirBlockId,
        location: crate::scalar_plan::ScalarBudgetLocation,
        source_points: &mut Vec<vela_common::Span>,
        mir_budget_sites: &mut Vec<crate::scalar_plan::ScalarMirBudgetSite>,
    ) -> Result<crate::ChargedScalarTarget, MirBackendError> {
        let instruction = *self
            .blocks
            .get(&target)
            .ok_or(MirBackendError::MissingBlock(target))?;
        let Some(point) = self.budget.edge(from, target) else {
            return Ok(crate::ChargedScalarTarget {
                target: instruction,
                execution_units: 0,
                budget_source: None,
            });
        };
        let source = crate::ScalarSourcePointId::new(source_points.len());
        source_points.push(point.origin.span);
        mir_budget_sites.push(crate::scalar_plan::ScalarMirBudgetSite {
            site: vela_mir::MirBudgetSite::Edge { from, to: target },
            point,
            location,
        });
        Ok(crate::ChargedScalarTarget {
            target: instruction,
            execution_units: point.units,
            budget_source: Some(source),
        })
    }
}

fn scalar_constant(value: MirImmediate) -> Result<crate::ScalarConstant, MirBackendError> {
    match value {
        MirImmediate::Bool(value) => Ok(crate::ScalarConstant::Bool(value)),
        MirImmediate::Scalar(vela_common::ScalarValue::I64(value)) => {
            Ok(crate::ScalarConstant::I64(value))
        }
        _ => Err(MirBackendError::MissingTarget("scalar constant")),
    }
}

const fn i64_immediate(value: MirImmediate) -> Option<i64> {
    match value {
        MirImmediate::Scalar(vela_common::ScalarValue::I64(value)) => Some(value),
        _ => None,
    }
}
