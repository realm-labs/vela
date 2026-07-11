impl<'a> FunctionBackend<'a> {
    fn operand(
        &mut self,
        operand: &MirOperand,
        span: vela_common::Span,
    ) -> Result<Register, MirBackendError> {
        Ok(match operand {
            MirOperand::Local(local) => self.locals[local],
            MirOperand::Temp(temp) => {
                if let Some(register) = self.temp_aliases.get(temp).copied() {
                    return Ok(register);
                }
                let register = self.temps[temp];
                if self
                    .current_block
                    .is_some_and(|block| self.loop_blocks.contains(&block))
                {
                    if self.temp_is_dead_after_current(*temp)
                        && let Some((_, Some(MirPlace::Temp(previous)), _, end)) =
                            self.last_statement
                        && previous == *temp
                        && end == self.code.instructions.len()
                        && let Some(UnlinkedInstruction {
                            kind: UnlinkedInstructionKind::Move { dst, src },
                            ..
                        }) = self.code.instructions.last()
                        && *dst == register
                    {
                        let src = *src;
                        self.code.instructions.pop();
                        self.shapes.remove(&register);
                        self.immediates.remove(&register);
                        src
                    } else {
                        register
                    }
                } else {
                    register
                }
            }
            MirOperand::Immediate(value) => {
                let register = self.alloc_register()?;
                self.load_immediate(register, *value, span);
                register
            }
        })
    }

    fn operands(
        &mut self,
        operands: &[MirOperand],
        span: vela_common::Span,
    ) -> Result<Vec<Register>, MirBackendError> {
        operands
            .iter()
            .map(|operand| self.operand(operand, span))
            .collect()
    }

    fn place(&self, place: MirPlace) -> Register {
        match place {
            MirPlace::Local(local) => self.locals[&local],
            MirPlace::Temp(temp) => self.temps[&temp],
        }
    }

    fn temp_is_dead_after_current(&self, temp: vela_mir::MirTempId) -> bool {
        self.current_statement.is_some_and(|statement| {
            self.function
                .liveness()
                .statement_live_after
                .get(&statement)
                .is_some_and(|live| !live.contains(&MirLiveValue::Temp(temp)))
        })
    }

    fn remove_dead_temp_move(&mut self, operand: &MirOperand) -> bool {
        let MirOperand::Temp(temp) = operand else {
            return false;
        };
        let register = self.temps[temp];
        if !matches!(
                self.code.instructions.last().map(|instruction| &instruction.kind),
                Some(UnlinkedInstructionKind::Move { dst, .. }) if *dst == register
            )
        {
            return false;
        }
        self.code.instructions.pop();
        self.shapes.remove(&register);
        self.immediates.remove(&register);
        true
    }

    fn local_is_dead_after_current(&self, local: vela_mir::MirLocalId) -> bool {
        self.current_statement.is_some_and(|statement| {
            self.function
                .liveness()
                .statement_live_after
                .get(&statement)
                .is_some_and(|live| !live.contains(&MirLiveValue::Local(local)))
        })
    }

    fn temp_for_register(&self, register: Register) -> Option<vela_mir::MirTempId> {
        self.temps
            .iter()
            .find_map(|(temp, candidate)| (*candidate == register).then_some(*temp))
    }

    fn materialize_aliases_before_write(&mut self, register: Register, span: vela_common::Span) {
        let aliases = self
            .temp_aliases
            .iter()
            .filter_map(|(temp, source)| (*source == register).then_some(*temp))
            .collect::<Vec<_>>();
        for temp in aliases {
            self.temp_aliases.remove(&temp);
            if !self.current_statement.is_some_and(|statement| {
                self.function
                    .liveness()
                    .statement_live_before
                    .get(&statement)
                    .is_some_and(|live| live.contains(&MirLiveValue::Temp(temp)))
            }) {
                continue;
            }
            let dst = self.temps[&temp];
            self.emit(UnlinkedInstructionKind::Move { dst, src: register }, span);
            self.copy_shape(dst, register);
            self.copy_immediate(dst, register);
        }
    }

    fn try_retarget_try_result(&mut self, local: vela_mir::MirLocalId, dst: Register) -> bool {
        if !self.local_is_dead_after_current(local) {
            return false;
        }
        let src = self.locals[&local];
        let Some(instruction) = self.code.instructions.last_mut() else {
            return false;
        };
        let UnlinkedInstructionKind::TryPropagate {
            dst: instruction_dst,
            ..
        } = &mut instruction.kind
        else {
            return false;
        };
        if *instruction_dst != src {
            return false;
        }
        *instruction_dst = dst;
        true
    }

    fn try_retarget_dead_temp(&mut self, temp: vela_mir::MirTempId, dst: Register) -> bool {
        if !self.temp_is_dead_after_current(temp) {
            return false;
        }
        let src = self.temps[&temp];
        let Some((_, Some(MirPlace::Temp(previous)), _, end)) = self.last_statement else {
            return false;
        };
        if previous != temp || end != self.code.instructions.len() {
            return false;
        }
        let Some(instruction) = self.code.instructions.last_mut() else {
            return false;
        };
        if !retarget_destination(&mut instruction.kind, src, dst) {
            return false;
        }
        self.copy_shape(dst, src);
        self.copy_immediate(dst, src);
        self.shapes.remove(&src);
        self.immediates.remove(&src);
        true
    }

    fn alloc_register(&mut self) -> Result<Register, MirBackendError> {
        let register = Register(self.next_register);
        self.next_register = self
            .next_register
            .checked_add(1)
            .ok_or(MirBackendError::RegisterOverflow)?;
        Ok(register)
    }

    fn copy_shape(&mut self, dst: Register, src: Register) {
        match self.shapes.get(&src).cloned() {
            Some(shape) => {
                self.shapes.insert(dst, shape);
            }
            None => {
                self.shapes.remove(&dst);
            }
        }
    }

    fn global_slot(&self, global: vela_def::GlobalId) -> Option<vela_common::GlobalSlot> {
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
            .map(vela_common::GlobalSlot::new)
    }

    fn stable_field_slot(&self, field: vela_def::FieldId) -> Result<usize, MirBackendError> {
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

    fn copy_immediate(&mut self, dst: Register, src: Register) {
        match self.immediates.get(&src).copied() {
            Some(value) => {
                self.immediates.insert(dst, value);
            }
            None => {
                self.immediates.remove(&dst);
            }
        }
    }

    fn install_record_shape(
        &mut self,
        dst: Register,
        fields: &[(String, Register)],
        variant: bool,
    ) {
        let mut names = fields
            .iter()
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();
        names.sort();
        names.dedup();
        let layout = names
            .into_iter()
            .enumerate()
            .map(|(slot, name)| {
                let shape = fields
                    .iter()
                    .find(|(candidate, _)| candidate == &name)
                    .and_then(|(_, register)| self.shapes.get(register).cloned())
                    .map(Box::new);
                (name, (slot, shape))
            })
            .collect();
        self.shapes.insert(
            dst,
            if variant {
                PhysicalShape::Variant(layout)
            } else {
                PhysicalShape::Record(layout)
            },
        );
    }

    fn shape_field(
        &self,
        receiver: Register,
        name: &str,
        variant: bool,
    ) -> Option<(usize, Option<PhysicalShape>)> {
        let fields = match (self.shapes.get(&receiver)?, variant) {
            (PhysicalShape::Record(fields), false) | (PhysicalShape::Variant(fields), true) => {
                fields
            }
            _ => return None,
        };
        let (slot, shape) = fields.get(name)?;
        Some((*slot, shape.as_deref().cloned()))
    }

    fn load_immediate(&mut self, dst: Register, value: MirImmediate, span: vela_common::Span) {
        self.immediates.insert(dst, value);
        let constant = self.code.push_constant(match value {
            MirImmediate::Unit => Constant::Unit,
            MirImmediate::Bool(value) => Constant::Bool(value),
            MirImmediate::Char(value) => Constant::Char(value),
            MirImmediate::Scalar(value) => Constant::Scalar(value),
        });
        self.emit(UnlinkedInstructionKind::LoadConst { dst, constant }, span);
    }

    fn select_binary(
        &mut self,
        operation: MirBinaryOp,
        dst: Register,
        lhs: Register,
        rhs: Register,
    ) -> UnlinkedInstructionKind {
        if self
            .current_block
            .is_some_and(|block| {
                self.loop_blocks.contains(&block) || self.try_join_blocks.contains(&block)
            })
            && let Some(MirImmediate::Scalar(vela_common::ScalarValue::I64(imm))) =
                self.immediates.get(&rhs).copied()
        {
            match operation {
                MirBinaryOp::Numeric {
                    operation: MirNumericBinaryOp::Add,
                    ..
                } => {
                    self.remove_dead_immediate_definition(rhs);
                    return UnlinkedInstructionKind::I64AddImm { dst, lhs, imm };
                }
                MirBinaryOp::Numeric {
                    operation: MirNumericBinaryOp::Subtract,
                    ..
                } => {
                    self.remove_dead_immediate_definition(rhs);
                    return UnlinkedInstructionKind::I64SubImm { dst, lhs, imm };
                }
                MirBinaryOp::Numeric {
                    operation: MirNumericBinaryOp::Multiply,
                    ..
                } => {
                    self.remove_dead_immediate_definition(rhs);
                    return UnlinkedInstructionKind::I64MulImm { dst, lhs, imm };
                }
                MirBinaryOp::Numeric {
                    operation: MirNumericBinaryOp::Remainder,
                    ..
                } if imm != 0 => {
                    self.remove_dead_immediate_definition(rhs);
                    return UnlinkedInstructionKind::I64RemImm { dst, lhs, imm };
                }
                MirBinaryOp::Compare { operation, .. } => {
                    self.remove_dead_immediate_definition(rhs);
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

    fn remove_dead_immediate_definition(&mut self, register: Register) {
        let Some((_, Some(MirPlace::Temp(temp)), _, end)) = self.last_statement else {
            return;
        };
        if self.temps[&temp] != register
            || !self.temp_is_dead_after_current(temp)
            || end != self.code.instructions.len()
            || !matches!(
                self.code.instructions.last().map(|instruction| &instruction.kind),
                Some(UnlinkedInstructionKind::LoadConst { dst, .. }) if *dst == register
            )
        {
            return;
        }
        self.code.instructions.pop();
        self.immediates.remove(&register);
    }

    fn load_switch(
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

    fn emit(&mut self, kind: UnlinkedInstructionKind, span: vela_common::Span) {
        let unspanned = self.unspanned_spans.contains(&span)
            || (!self.unspanned_spans.is_empty()
                && matches!(kind, UnlinkedInstructionKind::LoadConst { .. }));
        let instruction = UnlinkedInstruction::new(kind);
        self.code.push_instruction(if unspanned {
            instruction
        } else {
            instruction.with_span(span)
        });
    }

    fn invert_last_comparison(&mut self, src: Register, dst: Register) -> bool {
        let Some(last) = self.code.instructions.last_mut() else {
            return false;
        };
        last.kind = match last.kind.clone() {
            UnlinkedInstructionKind::Equal {
                dst: previous,
                lhs,
                rhs,
            } if previous == src => UnlinkedInstructionKind::NotEqual { dst, lhs, rhs },
            UnlinkedInstructionKind::NotEqual {
                dst: previous,
                lhs,
                rhs,
            } if previous == src => UnlinkedInstructionKind::Equal { dst, lhs, rhs },
            UnlinkedInstructionKind::IdentityEqual {
                dst: previous,
                lhs,
                rhs,
            } if previous == src => UnlinkedInstructionKind::IdentityNotEqual { dst, lhs, rhs },
            UnlinkedInstructionKind::IdentityNotEqual {
                dst: previous,
                lhs,
                rhs,
            } if previous == src => UnlinkedInstructionKind::IdentityEqual { dst, lhs, rhs },
            kind => {
                last.kind = kind;
                return false;
            }
        };
        true
    }

    fn fuse_i64_compare_branch(&mut self, condition: Register, target: MirBlockId) -> bool {
        let index = self.code.instructions.len().saturating_sub(1);
        let Some(last) = self.code.instructions.last_mut() else {
            return false;
        };
        let UnlinkedInstructionKind::I64CmpImm { dst, op, lhs, imm } = last.kind.clone() else {
            return false;
        };
        if dst != condition {
            return false;
        }
        last.kind = UnlinkedInstructionKind::I64CmpImmJumpIfFalse {
            op,
            lhs,
            imm,
            target: InstructionOffset(0),
        };
        self.patches.push((index, target));
        true
    }

    fn emit_patch(
        &mut self,
        kind: UnlinkedInstructionKind,
        target: MirBlockId,
        span: vela_common::Span,
    ) {
        let index = self.code.instructions.len();
        self.emit(kind, span);
        self.patches.push((index, target));
    }

    fn patch_targets(&mut self) -> Result<(), MirBackendError> {
        for (index, block) in &self.patches {
            let target = *self
                .blocks
                .get(block)
                .ok_or(MirBackendError::MissingBlock(*block))?;
            match &mut self.code.instructions[*index].kind {
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
                } => *slot = target,
                _ => return Err(MirBackendError::MissingTarget("CFG patch instruction")),
            }
        }
        Ok(())
    }

    fn attach_cache_sites(&mut self) {
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
