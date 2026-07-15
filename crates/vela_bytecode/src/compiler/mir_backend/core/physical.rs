use super::support::{binary_instruction, i64_compare};
use super::{
    Constant, FunctionBackend, InstructionOffset, MirBackendError, MirBinaryOp, MirBlockId,
    MirImmediate, MirOperand, MirPlace, MirSwitchValue, Register, UnlinkedInstruction,
    UnlinkedInstructionKind, attach_cache_site, cache_site_kind,
};
use vela_mir::MirNumericBinaryOp;

impl<'a> FunctionBackend<'a> {
    pub(super) fn operand(
        &mut self,
        operand: &MirOperand,
        span: vela_common::Span,
    ) -> Result<Register, MirBackendError> {
        Ok(match operand {
            MirOperand::Local(local) => self.locals[local],
            MirOperand::Temp(temp) => self.temps[temp],
            MirOperand::Immediate(value) => {
                let register = self.alloc_register()?;
                self.load_immediate(register, *value, span);
                register
            }
        })
    }

    pub(super) fn operands(
        &mut self,
        operands: &[MirOperand],
        span: vela_common::Span,
    ) -> Result<Vec<Register>, MirBackendError> {
        operands
            .iter()
            .map(|operand| self.operand(operand, span))
            .collect()
    }

    pub(super) fn place(&self, place: MirPlace) -> Register {
        match place {
            MirPlace::Local(local) => self.locals[&local],
            MirPlace::Temp(temp) => self.temps[&temp],
        }
    }

    pub(super) fn missing_test_value(&self, condition: &MirOperand) -> Option<MirOperand> {
        let MirOperand::Temp(temp) = condition else {
            return None;
        };
        let definition = self.function.temp(*temp)?.definition()?;
        match &self.function.statement(definition)?.kind {
            vela_mir::MirStatementKind::Assign(vela_mir::MirRvalue::IsMissing { value }) => {
                Some(value.clone())
            }
            _ => None,
        }
    }

    pub(super) fn alloc_register(&mut self) -> Result<Register, MirBackendError> {
        let register = Register(self.next_register);
        self.next_register = self
            .next_register
            .checked_add(1)
            .ok_or(MirBackendError::RegisterOverflow)?;
        Ok(register)
    }

    pub(super) fn global_slot(&self, global: vela_def::StateId) -> Option<vela_common::StateSlot> {
        let mut globals = self
            .program
            .targets()
            .globals()
            .map(|(id, descriptor)| (id, descriptor.name.as_str()))
            .collect::<Vec<_>>();
        globals.sort_by(|left, right| left.1.cmp(right.1));
        globals
            .into_iter()
            .map(|(id, _)| id)
            .position(|id| id == global)
            .map(vela_common::StateSlot::new)
    }

    pub(super) fn stable_field_slot(
        &self,
        field: vela_def::FieldId,
    ) -> Result<usize, MirBackendError> {
        let descriptor = self
            .program
            .targets()
            .field(field)
            .ok_or(MirBackendError::MissingTarget("field"))?;
        let mut fields = self
            .program
            .targets()
            .fields()
            .filter(|(_, candidate)| {
                candidate.owner == descriptor.owner && candidate.variant == descriptor.variant
            })
            .map(|(id, candidate)| (id, candidate.name.as_str()))
            .collect::<Vec<_>>();
        fields.sort_by(|left, right| left.1.cmp(right.1));
        fields
            .iter()
            .position(|(id, _)| *id == field)
            .ok_or(MirBackendError::MissingTarget("field slot"))
    }

    pub(super) fn shape_field(
        &self,
        shape: Option<&vela_mir::MirShapeFact>,
        name: &str,
        variant: bool,
    ) -> Option<(usize, Option<vela_mir::MirShapeFact>)> {
        let fields = match (shape?, variant) {
            (vela_mir::MirShapeFact::Record(fields), false)
            | (vela_mir::MirShapeFact::Variant(fields), true) => fields,
            _ => return None,
        };
        let (identity, shape) = fields.get(name)?;
        let slot = match identity {
            vela_mir::MirShapeFieldIdentity::Stable(field) => {
                self.stable_field_slot(*field).ok()?
            }
            vela_mir::MirShapeFieldIdentity::Ordinal(ordinal) => usize::try_from(*ordinal).ok()?,
        };
        Some((slot, shape.as_deref().and_then(|fact| fact.shape.clone())))
    }

    pub(super) fn operand_shape(&self, operand: &MirOperand) -> Option<vela_mir::MirShapeFact> {
        let statement = self.current_statement?;
        self.facts.operand_before(statement, operand)?.shape
    }

    pub(super) fn load_immediate(
        &mut self,
        dst: Register,
        value: MirImmediate,
        span: vela_common::Span,
    ) {
        let constant = self.code.push_constant(match value {
            MirImmediate::Unit => Constant::Unit,
            MirImmediate::Bool(value) => Constant::Bool(value),
            MirImmediate::Char(value) => Constant::Char(value),
            MirImmediate::Scalar(value) => Constant::Scalar(value),
        });
        self.emit(UnlinkedInstructionKind::LoadConst { dst, constant }, span);
    }

    pub(super) fn select_binary(
        &mut self,
        operation: MirBinaryOp,
        dst: Register,
        lhs: Register,
        rhs: Register,
        right: &MirOperand,
    ) -> UnlinkedInstructionKind {
        if self.current_block.is_some_and(|block| {
            self.loop_blocks.contains(&block) || self.try_join_blocks.contains(&block)
        }) && let Some(MirImmediate::Scalar(vela_common::ScalarValue::I64(imm))) = self
            .current_statement
            .and_then(|statement| self.facts.operand_before(statement, right))
            .filter(|fact| {
                !matches!(
                    fact.constant_provenance,
                    Some(vela_mir::MirConstantProvenance::PatternLiteral)
                )
            })
            .and_then(|fact| fact.immediate)
        {
            match operation {
                MirBinaryOp::Numeric {
                    operation: MirNumericBinaryOp::Add,
                    ..
                } => {
                    return UnlinkedInstructionKind::I64AddImm { dst, lhs, imm };
                }
                MirBinaryOp::Numeric {
                    operation: MirNumericBinaryOp::Subtract,
                    ..
                } => {
                    return UnlinkedInstructionKind::I64SubImm { dst, lhs, imm };
                }
                MirBinaryOp::Numeric {
                    operation: MirNumericBinaryOp::Multiply,
                    ..
                } => {
                    return UnlinkedInstructionKind::I64MulImm { dst, lhs, imm };
                }
                MirBinaryOp::Numeric {
                    operation: MirNumericBinaryOp::Remainder,
                    ..
                } if imm != 0 => {
                    return UnlinkedInstructionKind::I64RemImm { dst, lhs, imm };
                }
                MirBinaryOp::Compare { operation, .. } => {
                    return UnlinkedInstructionKind::I64CmpImm {
                        dst,
                        op: i64_compare(operation),
                        lhs,
                        imm,
                    };
                }
                MirBinaryOp::Numeric { .. } => {}
            }
        }
        if self
            .current_block
            .is_some_and(|block| self.loop_blocks.contains(&block))
            && matches!(
                operation,
                MirBinaryOp::Numeric {
                    operation: MirNumericBinaryOp::Add,
                    kind: vela_common::NumericTag::I64,
                }
            )
        {
            return UnlinkedInstructionKind::I64Add { dst, lhs, rhs };
        }
        binary_instruction(operation, dst, lhs, rhs)
    }

    pub(super) fn load_switch(
        &mut self,
        dst: Register,
        value: &MirSwitchValue,
        span: vela_common::Span,
    ) -> Result<(), MirBackendError> {
        let constant = match value {
            MirSwitchValue::Bool(value) => Constant::Bool(*value),
            MirSwitchValue::Char(value) => Constant::Char(*value),
            MirSwitchValue::Signed(value) => {
                Constant::Scalar(vela_common::ScalarValue::I64(*value))
            }
            MirSwitchValue::Unsigned(value) => {
                Constant::Scalar(vela_common::ScalarValue::U64(*value))
            }
        };
        let constant = self.code.push_constant(constant);
        self.emit(UnlinkedInstructionKind::LoadConst { dst, constant }, span);
        Ok(())
    }

    pub(super) fn emit(&mut self, kind: UnlinkedInstructionKind, span: vela_common::Span) {
        let unspanned = self.unspanned_spans.contains(&span)
            || (!self.unspanned_spans.is_empty()
                && matches!(kind, UnlinkedInstructionKind::LoadConst { .. }));
        let instruction = UnlinkedInstruction::new(kind)
            .with_execution_units(std::mem::take(&mut self.pending_execution_units))
            .with_mir_metadata(
                self.current_statement
                    .map(vela_mir::MirBudgetSite::StatementBefore)
                    .or_else(|| {
                        self.current_terminator
                            .map(vela_mir::MirBudgetSite::TerminatorBefore)
                    }),
                std::mem::take(&mut self.pending_budget_charges),
            );
        self.code.push_instruction(if unspanned {
            instruction
        } else {
            instruction.with_span(span)
        });
    }

    pub(super) fn emit_patch(
        &mut self,
        kind: UnlinkedInstructionKind,
        target: MirBlockId,
        span: vela_common::Span,
    ) {
        let index = self.code.instructions.len();
        self.emit(kind, span);
        self.patches.push((
            index,
            self.current_block
                .expect("CFG patch emission occurs inside one MIR block"),
            target,
        ));
    }

    pub(super) fn patch_targets(&mut self) -> Result<(), MirBackendError> {
        let patches = std::mem::take(&mut self.patches);
        for (index, from, block) in patches {
            let mut target = *self
                .blocks
                .get(&block)
                .ok_or(MirBackendError::MissingBlock(block))?;
            if let Some(point) = self.budget.edge(from, block) {
                let site = vela_mir::MirBudgetSite::Edge { from, to: block };
                let stub = InstructionOffset(self.code.instructions.len());
                self.code.push_instruction(
                    UnlinkedInstruction::new(UnlinkedInstructionKind::ChargeExecutionUnits {
                        units: point.units,
                    })
                    .with_span(point.origin.span)
                    .with_mir_metadata(
                        Some(site),
                        vec![crate::MirBudgetCharge {
                            site,
                            class: point.class,
                            units: point.units,
                        }],
                    ),
                );
                self.code.push_instruction(
                    UnlinkedInstruction::new(UnlinkedInstructionKind::Jump { target })
                        .with_span(point.origin.span)
                        .with_mir_metadata(Some(site), Vec::new()),
                );
                target = stub;
            }
            match &mut self.code.instructions[index].kind {
                UnlinkedInstructionKind::Jump { target: slot }
                | UnlinkedInstructionKind::JumpIfFalse { target: slot, .. }
                | UnlinkedInstructionKind::JumpIfNotMissing { target: slot, .. }
                | UnlinkedInstructionKind::I64CmpImmJumpIfFalse { target: slot, .. }
                | UnlinkedInstructionKind::IterNext {
                    jump_if_done: slot, ..
                }
                | UnlinkedInstructionKind::RangeNext {
                    jump_if_done: slot, ..
                }
                | UnlinkedInstructionKind::I64RangeNext {
                    jump_if_done: slot, ..
                }
                | UnlinkedInstructionKind::AwaitCall { resume: slot, .. } => *slot = target,
                _ => return Err(MirBackendError::MissingTarget("CFG patch instruction")),
            }
        }
        Ok(())
    }

    pub(super) fn attach_cache_sites(&mut self) {
        for index in 0..self.code.instructions.len() {
            let kind = self.code.instructions[index].kind.clone();
            let Some(site_kind) = cache_site_kind(&kind) else {
                continue;
            };
            let site = self
                .code
                .push_cache_site(site_kind, InstructionOffset(index));
            self.code.instructions[index].kind = attach_cache_site(kind, site);
        }
    }
}
