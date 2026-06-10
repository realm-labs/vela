use vela_bytecode::linked::InstructionKind;
use vela_bytecode::{InstructionOffset, LinkedCodeObject, LinkedProgram};
use vela_common::Span;

use super::*;

pub(crate) struct LinkedExecutionCall<'a> {
    pub(crate) code: &'a LinkedCodeObject,
    pub(crate) program: &'a LinkedProgram,
    pub(crate) captures: &'a [Value],
    pub(crate) args: &'a [Value],
    pub(crate) call_site: Option<Span>,
    pub(crate) call_site_offset: Option<InstructionOffset>,
    pub(crate) inline_caches: Option<&'a dyn VmInlineCaches>,
}

impl LinkedExecutionCall<'_> {
    fn stack_frame(&self) -> VmStackFrame {
        VmStackFrame::new(
            self.program.debug_name(self.code.debug_name),
            self.call_site,
        )
        .with_bytecode_offset(self.call_site_offset)
    }
}

impl Vm {
    pub(crate) fn execute_linked_call(
        &self,
        call: LinkedExecutionCall<'_>,
        host: Option<&mut HostExecution<'_>>,
        heap: Option<&mut HeapExecution<'_>>,
        mut budget: Option<&mut ExecutionBudget>,
    ) -> VmResult<Value> {
        if let Some(budget) = &mut budget {
            budget
                .enter_call()
                .map_err(|error| error.with_call_frame(call.stack_frame()))?;
        }
        let frame = call.stack_frame();
        let fallback_span = call.call_site.or_else(|| {
            call.code
                .instructions
                .first()
                .and_then(|instruction| instruction.span)
        });
        let result = self
            .execute_linked_body(call, host, heap, budget.as_deref_mut())
            .map_err(|error| {
                error
                    .with_source_span_if_absent(fallback_span)
                    .with_call_frame(frame)
            });
        if let Some(budget) = budget {
            budget.exit_call();
        }
        result
    }

    fn execute_linked_body(
        &self,
        call: LinkedExecutionCall<'_>,
        _host: Option<&mut HostExecution<'_>>,
        mut heap: Option<&mut HeapExecution<'_>>,
        mut budget: Option<&mut ExecutionBudget>,
    ) -> VmResult<Value> {
        let code = call.code;
        if let Some(inline_caches) = call.inline_caches {
            debug_assert!(inline_caches.len() >= code.cache_sites.len());
        }
        let function_name = call.program.debug_name(code.debug_name);
        if call.captures.len() != usize::from(code.capture_count) {
            return Err(VmError::new(VmErrorKind::ArityMismatch {
                name: function_name.to_owned(),
                expected: usize::from(code.capture_count),
                actual: call.captures.len(),
            }));
        }
        if call.args.len() > code.params.len() {
            return Err(VmError::new(VmErrorKind::ArityMismatch {
                name: function_name.to_owned(),
                expected: code.params.len(),
                actual: call.args.len(),
            }));
        }

        let mut frame = CallFrame::new(code.register_count);
        for (index, capture) in call.captures.iter().enumerate() {
            frame.write(
                Register(u16::try_from(index).map_err(|_| {
                    VmError::new(VmErrorKind::RegisterOutOfBounds {
                        register: Register(u16::MAX),
                    })
                })?),
                *capture,
            )?;
        }
        let param_offset = usize::from(code.capture_count);
        for (index, arg) in call.args.iter().enumerate() {
            frame.write(
                Register(
                    u16::try_from(param_offset.saturating_add(index)).map_err(|_| {
                        VmError::new(VmErrorKind::RegisterOutOfBounds {
                            register: Register(u16::MAX),
                        })
                    })?,
                ),
                *arg,
            )?;
        }
        for index in call.args.len()..code.params.len() {
            frame.write(
                Register(
                    u16::try_from(param_offset.saturating_add(index)).map_err(|_| {
                        VmError::new(VmErrorKind::RegisterOutOfBounds {
                            register: Register(u16::MAX),
                        })
                    })?,
                ),
                Value::Missing,
            )?;
        }
        let actual = call
            .args
            .iter()
            .filter(|arg| !matches!(arg, Value::Missing))
            .count();
        for index in 0..code.params.len() {
            let register = Register(u16::try_from(param_offset.saturating_add(index)).map_err(
                |_| {
                    VmError::new(VmErrorKind::RegisterOutOfBounds {
                        register: Register(u16::MAX),
                    })
                },
            )?);
            let has_default = code.param_defaults.get(index).copied().unwrap_or(false);
            if !has_default && matches!(frame.read(register)?, Value::Missing) {
                return Err(VmError::new(VmErrorKind::ArityMismatch {
                    name: function_name.to_owned(),
                    expected: code.params.len(),
                    actual,
                }));
            }
        }

        let mut ip = 0_usize;
        while ip < code.instructions.len() {
            let instruction = &code.instructions[ip];
            if let Some(budget) = budget.as_deref_mut() {
                budget.charge_instruction()?;
            }
            ip = ip.saturating_add(1);

            match &instruction.kind {
                InstructionKind::LoadConst { dst, constant } => {
                    let constant_value = code.constants.get(constant.0).ok_or_else(|| {
                        VmError::new(VmErrorKind::ConstantOutOfBounds {
                            constant: constant.0,
                        })
                        .with_source_span(instruction.span)
                    })?;
                    let value = match constant_value {
                        Constant::Null => Value::Null,
                        Constant::Bool(value) => Value::Bool(*value),
                        Constant::Int(value) => Value::Int(*value),
                        Constant::Float(value) => Value::Float(*value),
                        Constant::String(value) => {
                            if let Some(value) = constant_loads::loaded_string_constant(
                                frame.read(*dst).ok(),
                                value,
                                heap.as_deref(),
                            ) {
                                value
                            } else {
                                value_from_constant(
                                    constant_value,
                                    heap.as_deref_mut(),
                                    budget.as_deref_mut(),
                                )?
                            }
                        }
                        Constant::Array(_) | Constant::Map(_) => value_from_constant(
                            constant_value,
                            heap.as_deref_mut(),
                            budget.as_deref_mut(),
                        )?,
                    };
                    frame.write(*dst, value)?;
                }
                InstructionKind::Move { dst, src } => {
                    let value = *frame.read(*src)?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::Not { dst, src } => {
                    let value = Value::Bool(!is_truthy(frame.read(*src)?));
                    frame.write(*dst, value)?;
                }
                InstructionKind::Truthy { dst, src } => {
                    let value = Value::Bool(is_truthy(frame.read(*src)?));
                    frame.write(*dst, value)?;
                }
                InstructionKind::Negate { dst, src } => {
                    let value = negate_numeric(frame.read(*src)?)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::Add { dst, lhs, rhs } => {
                    let value = add_numeric(frame.read(*lhs)?, frame.read(*rhs)?)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::Sub { dst, lhs, rhs } => {
                    let value = sub_numeric(frame.read(*lhs)?, frame.read(*rhs)?)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::Mul { dst, lhs, rhs } => {
                    let value = mul_numeric(frame.read(*lhs)?, frame.read(*rhs)?)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::Div { dst, lhs, rhs } => {
                    let value = div_numeric(frame.read(*lhs)?, frame.read(*rhs)?)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::Rem { dst, lhs, rhs } => {
                    let value = rem_numeric(frame.read(*lhs)?, frame.read(*rhs)?)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::Equal { dst, lhs, rhs } => {
                    let value = Value::Bool(values_equal(
                        frame.read(*lhs)?,
                        frame.read(*rhs)?,
                        heap.as_deref(),
                    )?);
                    frame.write(*dst, value)?;
                }
                InstructionKind::NotEqual { dst, lhs, rhs } => {
                    let value = Value::Bool(!values_equal(
                        frame.read(*lhs)?,
                        frame.read(*rhs)?,
                        heap.as_deref(),
                    )?);
                    frame.write(*dst, value)?;
                }
                InstructionKind::Less { dst, lhs, rhs } => {
                    let value = less_numeric(frame.read(*lhs)?, frame.read(*rhs)?)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write(*dst, Value::Bool(value))?;
                }
                InstructionKind::LessEqual { dst, lhs, rhs } => {
                    let value = less_equal_numeric(frame.read(*lhs)?, frame.read(*rhs)?)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write(*dst, Value::Bool(value))?;
                }
                InstructionKind::Greater { dst, lhs, rhs } => {
                    let value = greater_numeric(frame.read(*lhs)?, frame.read(*rhs)?)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write(*dst, Value::Bool(value))?;
                }
                InstructionKind::GreaterEqual { dst, lhs, rhs } => {
                    let value = greater_equal_numeric(frame.read(*lhs)?, frame.read(*rhs)?)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write(*dst, Value::Bool(value))?;
                }
                InstructionKind::JumpIfFalse { condition, target } => {
                    if !is_truthy(frame.read(*condition)?) {
                        validate_linked_jump(code, target.0)?;
                        ip = target.0;
                    }
                }
                InstructionKind::JumpIfNotMissing { value, target } => {
                    if !matches!(frame.read(*value)?, Value::Missing) {
                        validate_linked_jump(code, target.0)?;
                        ip = target.0;
                    }
                }
                InstructionKind::Jump { target } => {
                    validate_linked_jump(code, target.0)?;
                    ip = target.0;
                }
                InstructionKind::TryPropagate { dst, src } => {
                    match try_propagate_value(frame.read(*src)?, heap.as_deref())? {
                        TryPropagation::Continue(value) => frame.write(*dst, value)?,
                        TryPropagation::Return(value) => return Ok(value),
                    }
                }
                InstructionKind::Return { src } => {
                    return Ok(*frame.read(*src)?);
                }
                unsupported => {
                    return Err(VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
                        opcode: linked_opcode_name(unsupported),
                    })
                    .with_source_span_if_absent(instruction.span));
                }
            }
        }

        Err(VmError::new(VmErrorKind::MissingReturn))
    }
}

fn validate_linked_jump(code: &LinkedCodeObject, offset: usize) -> VmResult<()> {
    if offset <= code.instructions.len() {
        Ok(())
    } else {
        Err(VmError::new(VmErrorKind::InstructionOutOfBounds { offset }))
    }
}

fn linked_opcode_name(kind: &InstructionKind) -> &'static str {
    match kind {
        InstructionKind::LoadConst { .. } => "LoadConst",
        InstructionKind::Move { .. } => "Move",
        InstructionKind::Not { .. } => "Not",
        InstructionKind::Truthy { .. } => "Truthy",
        InstructionKind::Negate { .. } => "Negate",
        InstructionKind::Add { .. } => "Add",
        InstructionKind::Sub { .. } => "Sub",
        InstructionKind::Mul { .. } => "Mul",
        InstructionKind::Div { .. } => "Div",
        InstructionKind::Rem { .. } => "Rem",
        InstructionKind::Equal { .. } => "Equal",
        InstructionKind::NotEqual { .. } => "NotEqual",
        InstructionKind::Less { .. } => "Less",
        InstructionKind::LessEqual { .. } => "LessEqual",
        InstructionKind::Greater { .. } => "Greater",
        InstructionKind::GreaterEqual { .. } => "GreaterEqual",
        InstructionKind::JumpIfFalse { .. } => "JumpIfFalse",
        InstructionKind::JumpIfNotMissing { .. } => "JumpIfNotMissing",
        InstructionKind::Jump { .. } => "Jump",
        InstructionKind::CallNative { .. } => "CallNative",
        InstructionKind::CallFunction { .. } => "CallFunction",
        InstructionKind::MakeClosure { .. } => "MakeClosure",
        InstructionKind::CallClosure { .. } => "CallClosure",
        InstructionKind::CallMethod { .. } => "CallMethod",
        InstructionKind::TryPropagate { .. } => "TryPropagate",
        InstructionKind::MakeArray { .. } => "MakeArray",
        InstructionKind::MakeMap { .. } => "MakeMap",
        InstructionKind::MakeRange { .. } => "MakeRange",
        InstructionKind::MakeRecord { .. } => "MakeRecord",
        InstructionKind::MakeEnum { .. } => "MakeEnum",
        InstructionKind::GetRecordSlot { .. } => "GetRecordSlot",
        InstructionKind::SetRecordSlot { .. } => "SetRecordSlot",
        InstructionKind::GetEnumSlot { .. } => "GetEnumSlot",
        InstructionKind::GetIndex { .. } => "GetIndex",
        InstructionKind::SetIndex { .. } => "SetIndex",
        InstructionKind::IterInit { .. } => "IterInit",
        InstructionKind::IterNext { .. } => "IterNext",
        InstructionKind::RangeNext { .. } => "RangeNext",
        InstructionKind::EnumTagEqual { .. } => "EnumTagEqual",
        InstructionKind::LoadGlobal { .. } => "LoadGlobal",
        InstructionKind::HostRead { .. } => "HostRead",
        InstructionKind::HostWrite { .. } => "HostWrite",
        InstructionKind::HostMutate { .. } => "HostMutate",
        InstructionKind::HostRemove { .. } => "HostRemove",
        InstructionKind::HostCall { .. } => "HostCall",
        InstructionKind::Return { .. } => "Return",
    }
}
