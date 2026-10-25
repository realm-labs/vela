use super::{
    CallArgument, CompileTryFamily, CompileTryTarget, DynamicCallArgument, FunctionBackend,
    InstructionOffset, MirBackendError, MirBlockId, MirCall, MirImmediate, MirReflectionOperation,
    MirScriptParameterGuardMode, MirTerminatorKind, Register, ScriptCallMode, TryPropagateFamily,
    UnlinkedInstructionKind,
};

impl<'a> FunctionBackend<'a> {
    pub(super) fn call(
        &mut self,
        dst: Register,
        call: &MirCall,
        span: vela_common::Span,
    ) -> Result<(), MirBackendError> {
        let kind = self.call_instruction(dst, call, span)?;
        self.emit(kind, span);
        Ok(())
    }

    fn call_instruction(
        &mut self,
        dst: Register,
        call: &MirCall,
        span: vela_common::Span,
    ) -> Result<UnlinkedInstructionKind, MirBackendError> {
        let kind = match call {
            MirCall::ScriptFunction {
                function,
                debug_name,
                arguments,
                parameter_guards,
                ..
            } => UnlinkedInstructionKind::CallFunction {
                dst,
                target: *function,
                name: debug_name.clone(),
                mode: if matches!(
                    parameter_guards,
                    MirScriptParameterGuardMode::ProvenAtCallSite
                ) {
                    ScriptCallMode::Unchecked
                } else {
                    ScriptCallMode::Checked
                },
                args: arguments
                    .iter()
                    .map(|argument| match &argument.value {
                        Some(value) => Ok(CallArgument::Register(self.operand(value, span)?)),
                        None => Ok(CallArgument::Missing),
                    })
                    .collect::<Result<_, MirBackendError>>()?,
            },
            MirCall::ScriptMethod {
                target,
                debug_name,
                receiver,
                arguments,
                ..
            } => UnlinkedInstructionKind::CallMethodId {
                dst,
                receiver: self.operand(receiver, span)?,
                method: debug_name.clone(),
                method_id: target.method,
                args: arguments
                    .iter()
                    .map(|argument| match &argument.value {
                        Some(value) => Ok(CallArgument::Register(self.operand(value, span)?)),
                        None => Ok(CallArgument::Missing),
                    })
                    .collect::<Result<_, MirBackendError>>()?,
            },
            MirCall::CallableValue { callee, arguments } => UnlinkedInstructionKind::CallClosure {
                dst,
                callee: self.operand(callee, span)?,
                args: self.operands(arguments, span)?,
            },
            MirCall::DynamicCallable { callee, arguments } => {
                UnlinkedInstructionKind::CallClosure {
                    dst,
                    callee: self.operand(callee, span)?,
                    args: arguments
                        .iter()
                        .map(|argument| self.operand(&argument.value, span))
                        .collect::<Result<_, _>>()?,
                }
            }
            MirCall::NativeFunction {
                function,
                debug_name,
                arguments,
                ..
            }
            | MirCall::StdlibFunction {
                function,
                debug_name,
                arguments,
                ..
            } => UnlinkedInstructionKind::CallNative {
                dst: Some(dst),
                name: debug_name.clone(),
                native: *function,
                cache_site: None,
                args: self.operands(arguments, span)?,
            },
            MirCall::ValueMethod {
                method,
                debug_name,
                receiver,
                arguments,
                ..
            } => UnlinkedInstructionKind::CallMethodId {
                dst,
                receiver: self.operand(receiver, span)?,
                method: debug_name.clone(),
                method_id: *method,
                args: self
                    .operands(arguments, span)?
                    .into_iter()
                    .map(CallArgument::Register)
                    .collect(),
            },
            MirCall::DynamicMethod {
                target,
                receiver,
                arguments,
            } => UnlinkedInstructionKind::CallDynamicMethod {
                dst,
                receiver: self.operand(receiver, span)?,
                method: target.member.clone(),
                args: arguments
                    .iter()
                    .map(|argument| {
                        Ok(DynamicCallArgument {
                            name: argument.name.clone(),
                            value: self.operand(&argument.value, span)?,
                        })
                    })
                    .collect::<Result<_, MirBackendError>>()?,
            },
        };
        Ok(kind)
    }

    pub(super) fn reflect(
        &mut self,
        dst: Register,
        operation: &MirReflectionOperation,
        span: vela_common::Span,
    ) -> Result<(), MirBackendError> {
        let kind = self.reflect_instruction(dst, operation, span)?;
        self.emit(kind, span);
        Ok(())
    }

    fn reflect_instruction(
        &mut self,
        dst: Register,
        operation: &MirReflectionOperation,
        span: vela_common::Span,
    ) -> Result<UnlinkedInstructionKind, MirBackendError> {
        let (function, args) = match operation {
            MirReflectionOperation::Read {
                function,
                target,
                member,
            } => (
                *function,
                vec![self.operand(target, span)?, self.operand(member, span)?],
            ),
            MirReflectionOperation::Write {
                function,
                target,
                member,
                value,
            } => (
                *function,
                vec![
                    self.operand(target, span)?,
                    self.operand(member, span)?,
                    self.operand(value, span)?,
                ],
            ),
            MirReflectionOperation::Call {
                function,
                target,
                tail,
            } => {
                let mut args = vec![self.operand(target, span)?];
                args.extend(self.operands(tail, span)?);
                (*function, args)
            }
        };
        let descriptor = self
            .program
            .targets()
            .function(function)
            .ok_or(MirBackendError::MissingTarget("reflection function"))?;
        Ok(UnlinkedInstructionKind::CallNative {
            dst: Some(dst),
            name: descriptor.debug_name.clone(),
            native: function,
            cache_site: None,
            args,
        })
    }

    pub(super) fn terminator(
        &mut self,
        terminator: &MirTerminatorKind,
        span: vela_common::Span,
        next_block: Option<MirBlockId>,
    ) -> Result<(), MirBackendError> {
        match terminator {
            MirTerminatorKind::Jump(target) if Some(*target) == next_block => {}
            MirTerminatorKind::Jump(target) => self.emit_patch(
                UnlinkedInstructionKind::Jump {
                    target: InstructionOffset(0),
                },
                *target,
                span,
            ),
            MirTerminatorKind::Branch {
                condition,
                then_block,
                else_block,
            } => {
                let missing_value = self.missing_test_value(condition);
                let condition = self.operand(condition, span)?;
                if let Some(value) = missing_value {
                    let value = self.operand(&value, span)?;
                    self.emit_patch(
                        UnlinkedInstructionKind::JumpIfNotMissing {
                            value,
                            target: InstructionOffset(0),
                        },
                        *else_block,
                        span,
                    );
                } else {
                    self.emit_patch(
                        UnlinkedInstructionKind::JumpIfFalse {
                            condition,
                            target: InstructionOffset(0),
                        },
                        *else_block,
                        span,
                    );
                }
                if Some(*then_block) != next_block {
                    self.emit_patch(
                        UnlinkedInstructionKind::Jump {
                            target: InstructionOffset(0),
                        },
                        *then_block,
                        span,
                    );
                }
            }
            MirTerminatorKind::Switch {
                discriminant,
                cases,
                otherwise,
            } => {
                let discriminant = self.operand(discriminant, span)?;
                for case in cases {
                    let rhs = self.alloc_register()?;
                    self.load_switch(rhs, &case.value, span)?;
                    let condition = self.alloc_register()?;
                    self.emit(
                        UnlinkedInstructionKind::Equal {
                            dst: condition,
                            lhs: discriminant,
                            rhs,
                        },
                        span,
                    );
                    let skip = InstructionOffset(self.code.instructions.len() + 2);
                    self.emit(
                        UnlinkedInstructionKind::JumpIfFalse {
                            condition,
                            target: skip,
                        },
                        span,
                    );
                    self.emit_patch(
                        UnlinkedInstructionKind::Jump {
                            target: InstructionOffset(0),
                        },
                        case.target,
                        span,
                    );
                }
                self.emit_patch(
                    UnlinkedInstructionKind::Jump {
                        target: InstructionOffset(0),
                    },
                    *otherwise,
                    span,
                );
            }
            MirTerminatorKind::GuardBranch { slow, .. } => self.emit_patch(
                UnlinkedInstructionKind::Jump {
                    target: InstructionOffset(0),
                },
                *slow,
                span,
            ),
            MirTerminatorKind::TrySwitch {
                value,
                target,
                result,
                join,
                ..
            } => {
                let src = self.operand(value, span)?;
                let expected = match target {
                    CompileTryTarget::Expected(layout) => Some(match layout.family {
                        CompileTryFamily::Option => TryPropagateFamily::Option,
                        CompileTryFamily::Result => TryPropagateFamily::Result,
                    }),
                    CompileTryTarget::Dynamic { .. } => None,
                };
                self.emit(
                    UnlinkedInstructionKind::TryPropagate {
                        dst: self.locals[result],
                        src,
                        expected,
                    },
                    span,
                );
                if Some(*join) != next_block {
                    self.emit_patch(
                        UnlinkedInstructionKind::Jump {
                            target: InstructionOffset(0),
                        },
                        *join,
                        span,
                    );
                }
            }
            MirTerminatorKind::IteratorNext {
                iterator,
                item,
                next,
                done,
            } => {
                let iterator = self.operand(iterator, span)?;
                self.emit_patch(
                    UnlinkedInstructionKind::IterNext {
                        iterator,
                        dst: self.locals[item],
                        jump_if_done: InstructionOffset(0),
                    },
                    *done,
                    span,
                );
                if Some(*next) != next_block {
                    self.emit_patch(
                        UnlinkedInstructionKind::Jump {
                            target: InstructionOffset(0),
                        },
                        *next,
                        span,
                    );
                }
            }
            MirTerminatorKind::RangeNext {
                cursor,
                end,
                exhausted,
                inclusive,
                item,
                mode,
                next,
                done,
            } => {
                let end = self.operand(end, span)?;
                let common = (
                    self.locals[cursor],
                    end,
                    self.locals[exhausted],
                    *inclusive,
                    self.locals[item],
                );
                let kind = match mode {
                    vela_mir::MirRangeStepMode::I64Proven => {
                        UnlinkedInstructionKind::I64RangeNext {
                            cursor: common.0,
                            end: common.1,
                            done: common.2,
                            inclusive: common.3,
                            dst: common.4,
                            jump_if_done: InstructionOffset(0),
                        }
                    }
                    vela_mir::MirRangeStepMode::DynamicInteger => {
                        UnlinkedInstructionKind::RangeNext {
                            cursor: common.0,
                            end: common.1,
                            done: common.2,
                            inclusive: common.3,
                            dst: common.4,
                            jump_if_done: InstructionOffset(0),
                        }
                    }
                };
                self.emit_patch(kind, *done, span);
                if matches!(mode, vela_mir::MirRangeStepMode::I64Proven)
                    && let Some(instruction) = self.code.instructions.last_mut()
                {
                    instruction.span = None;
                }
                if Some(*next) != next_block
                    || self
                        .current_terminator
                        .is_none_or(|from| self.budget.edge(from, *next).is_some())
                {
                    self.emit_patch(
                        UnlinkedInstructionKind::Jump {
                            target: InstructionOffset(0),
                        },
                        *next,
                        span,
                    );
                }
                if matches!(mode, vela_mir::MirRangeStepMode::I64Proven)
                    && let Some(instruction) = self.code.instructions.last_mut()
                {
                    instruction.span = None;
                }
            }
            MirTerminatorKind::Return(value) => {
                let src = match value {
                    Some(value) => self.operand(value, span)?,
                    None => {
                        let dst = self.alloc_register()?;
                        self.load_immediate(dst, MirImmediate::Unit, span);
                        dst
                    }
                };
                self.emit(UnlinkedInstructionKind::Return { src }, span);
            }
            MirTerminatorKind::TryTypeMismatch { .. } | MirTerminatorKind::Unreachable => {
                let src = self.alloc_register()?;
                self.load_immediate(src, MirImmediate::Unit, span);
                self.emit(UnlinkedInstructionKind::Return { src }, span);
            }
        }
        Ok(())
    }

    pub(super) fn superinstruction(
        &mut self,
        selected: &super::SuperinstructionPlan,
        terminator_span: vela_common::Span,
        next_block: Option<MirBlockId>,
    ) -> Result<(), MirBackendError> {
        let statement = self
            .function
            .statement(selected.statement())
            .ok_or(MirBackendError::MissingStatement)?;
        let lhs = self.operand(selected.value(), statement.origin.span)?;
        let instruction = InstructionOffset(self.code.instructions.len());
        self.emit_patch(
            UnlinkedInstructionKind::I64CmpImmJumpIfFalse {
                op: selected.op(),
                lhs,
                imm: selected.immediate(),
                target: InstructionOffset(0),
            },
            selected.else_block(),
            statement.origin.span,
        );
        let block = self
            .current_block
            .expect("selected instruction is emitted inside one MIR block");
        self.pending_selected_units
            .push(super::PendingSelectedUnit {
                instruction,
                source_points: [statement.origin.span, terminator_span],
                budget_units: [
                    self.budget
                        .statement_before(selected.statement())
                        .map_or(0, |point| point.units),
                    self.budget
                        .terminator_before(block)
                        .map_or(0, |point| point.units),
                ],
                mir_statement: selected.statement(),
                mir_terminator: block,
            });
        if Some(selected.then_block()) != next_block {
            self.emit_patch(
                UnlinkedInstructionKind::Jump {
                    target: InstructionOffset(0),
                },
                selected.then_block(),
                terminator_span,
            );
        }
        Ok(())
    }
}
