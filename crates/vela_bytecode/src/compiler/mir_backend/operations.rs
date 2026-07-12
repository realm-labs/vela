use super::*;

impl<'a> FunctionBackend<'a> {
    pub(super) fn call(
        &mut self,
        dst: Register,
        call: &MirCall,
        span: vela_common::Span,
    ) -> Result<(), MirBackendError> {
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
        self.emit(kind, span);
        Ok(())
    }

    pub(super) fn host(
        &mut self,
        dst: Option<Register>,
        operation: &MirHostOperation,
        span: vela_common::Span,
    ) -> Result<(), MirBackendError> {
        let (root, path) = match operation {
            MirHostOperation::Read { root, path } | MirHostOperation::Remove { root, path } => {
                (root, path)
            }
            MirHostOperation::Write { root, path, .. }
            | MirHostOperation::Mutate { root, path, .. }
            | MirHostOperation::Call { root, path, .. } => (root, path),
        };
        let root = self.operand(root, span)?;
        let (target, dynamic_args) = self.host_target(path, span)?;
        let kind = match operation {
            MirHostOperation::Read { .. } => UnlinkedInstructionKind::HostRead {
                dst: dst.ok_or(MirBackendError::MissingDestination)?,
                root,
                target,
                dynamic_args,
                cache_site: CacheSiteId::new(0),
            },
            MirHostOperation::Write { value, .. } => UnlinkedInstructionKind::HostWrite {
                root,
                target,
                dynamic_args,
                src: self.operand(value, span)?,
                cache_site: CacheSiteId::new(0),
            },
            MirHostOperation::Mutate {
                operation, value, ..
            } => UnlinkedInstructionKind::HostMutate {
                root,
                target,
                dynamic_args,
                op: host_mutation(*operation),
                rhs: self.operand(value, span)?,
                cache_site: CacheSiteId::new(0),
            },
            MirHostOperation::Remove { .. } => UnlinkedInstructionKind::HostRemove {
                root,
                target,
                dynamic_args,
                cache_site: CacheSiteId::new(0),
            },
            MirHostOperation::Call {
                target: method,
                arguments,
                ..
            } => UnlinkedInstructionKind::HostCall {
                dst,
                root,
                target,
                dynamic_args,
                method: method.runtime,
                args: self.operands(arguments, span)?,
                cache_site: CacheSiteId::new(0),
            },
        };
        self.emit(kind, span);
        Ok(())
    }

    fn host_target(
        &mut self,
        path: &MirHostPath,
        span: vela_common::Span,
    ) -> Result<(crate::HostTargetPlanId, Vec<Register>), MirBackendError> {
        let mut plan =
            HostTargetPlan::with_part_capacity(path.root_type.runtime, path.segments.len());
        let mut dynamic_args = Vec::new();
        for segment in &path.segments {
            match segment {
                MirHostPathSegment::Field(field) => plan = plan.field(field.runtime),
                MirHostPathSegment::VariantField(field) => plan = plan.variant_field(field.runtime),
                MirHostPathSegment::ConstantIndex { value, .. } => plan = plan.const_index(*value),
                MirHostPathSegment::ConstantKey { value, .. } => {
                    plan = plan.const_key(value.clone())
                }
                MirHostPathSegment::Index { value, .. } | MirHostPathSegment::Key { value, .. } => {
                    let index = u8::try_from(dynamic_args.len())
                        .map_err(|_| MirBackendError::DynamicHostArgumentOverflow)?;
                    dynamic_args.push(self.operand(value, span)?);
                    plan = if matches!(segment, MirHostPathSegment::Index { .. }) {
                        plan.dyn_index(index)
                    } else {
                        plan.dyn_key(index)
                    };
                }
            }
        }
        Ok((self.code.intern_host_target(plan), dynamic_args))
    }

    pub(super) fn reflect(
        &mut self,
        dst: Register,
        operation: &MirReflectionOperation,
        span: vela_common::Span,
    ) -> Result<(), MirBackendError> {
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
        self.emit(
            UnlinkedInstructionKind::CallNative {
                dst: Some(dst),
                name: descriptor.debug_name.clone(),
                native: function,
                cache_site: None,
                args,
            },
            span,
        );
        Ok(())
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
                self.emit_patch(
                    UnlinkedInstructionKind::Jump {
                        target: InstructionOffset(0),
                    },
                    *next,
                    span,
                );
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
}
