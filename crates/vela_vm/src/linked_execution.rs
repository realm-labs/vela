use vela_bytecode::linked::InstructionKind;
use vela_bytecode::{InstructionOffset, LinkedCodeObject, LinkedProgram, Register};
use vela_common::Span;

use super::*;

pub(crate) struct LinkedExecutionCall<'a> {
    pub(crate) code: &'a LinkedCodeObject,
    pub(crate) program: &'a LinkedProgram,
    pub(crate) captures: &'a [Value],
    pub(crate) args: &'a [Value],
    pub(crate) check_param_guards: bool,
    pub(crate) call_site: Option<Span>,
    pub(crate) call_site_offset: Option<InstructionOffset>,
    pub(crate) inline_caches: Option<&'a dyn VmInlineCaches>,
    pub(crate) bytecode_profiler: Option<&'a dyn VmBytecodeProfiler>,
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
        let charges_instructions = budget
            .as_deref()
            .is_some_and(ExecutionBudget::charges_instructions);
        let has_profiler = call.bytecode_profiler.is_some();
        let result = match (charges_instructions, has_profiler) {
            (false, false) => {
                self.execute_linked_body::<false, false>(call, host, heap, budget.as_deref_mut())
            }
            (true, false) => {
                self.execute_linked_body::<true, false>(call, host, heap, budget.as_deref_mut())
            }
            (false, true) => {
                self.execute_linked_body::<false, true>(call, host, heap, budget.as_deref_mut())
            }
            (true, true) => {
                self.execute_linked_body::<true, true>(call, host, heap, budget.as_deref_mut())
            }
        }
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

    fn execute_linked_body<const CHARGE_BUDGET: bool, const PROFILE: bool>(
        &self,
        call: LinkedExecutionCall<'_>,
        mut host: Option<&mut HostExecution<'_>>,
        mut heap: Option<&mut HeapExecution<'_>>,
        mut budget: Option<&mut ExecutionBudget>,
    ) -> VmResult<Value> {
        let code = call.code;
        validate_inline_cache_layout(call.inline_caches, code.cache_sites.len())?;
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
        if call.check_param_guards {
            runtime_type_guards::execute_linked_param_guards(
                code,
                call.program,
                &frame,
                heap.as_deref(),
            )?;
        }

        let mut ip = 0_usize;
        while ip < code.instructions.len() {
            let instruction_offset = InstructionOffset(ip);
            let instruction = &code.instructions[ip];
            if CHARGE_BUDGET {
                budget
                    .as_deref_mut()
                    .expect("budget execution mode requires a budget")
                    .charge_instruction()?;
            }
            if PROFILE {
                call.bytecode_profiler
                    .expect("profile execution mode requires a profiler")
                    .record_instruction(code.debug_name, instruction_offset);
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
                    constant_loads::dispatch_load_const(
                        &mut frame,
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                        *dst,
                        constant_value,
                    )?;
                }
                InstructionKind::Move { dst, src } => {
                    let value = frame.read(*src)?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::Not { dst, src } => {
                    let value = Value::Bool(!is_truthy(&frame.read(*src)?));
                    frame.write(*dst, value)?;
                }
                InstructionKind::Truthy { dst, src } => {
                    let value = Value::Bool(is_truthy(&frame.read(*src)?));
                    frame.write(*dst, value)?;
                }
                InstructionKind::Negate { dst, src } => {
                    let value = negate_numeric(&frame.read(*src)?)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::Add { dst, lhs, rhs } => {
                    let value = add_numeric(&frame.read(*lhs)?, &frame.read(*rhs)?)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::Sub { dst, lhs, rhs } => {
                    let value = sub_numeric(&frame.read(*lhs)?, &frame.read(*rhs)?)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::Mul { dst, lhs, rhs } => {
                    let value = mul_numeric(&frame.read(*lhs)?, &frame.read(*rhs)?)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::Div { dst, lhs, rhs } => {
                    let value = div_numeric(&frame.read(*lhs)?, &frame.read(*rhs)?)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::Rem { dst, lhs, rhs } => {
                    let value = rem_numeric(&frame.read(*lhs)?, &frame.read(*rhs)?)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::BinaryIntLiteral {
                    dst,
                    op,
                    value,
                    literal,
                    side,
                } => {
                    let value = binary_int_literal_numeric(
                        *op,
                        &frame.read(*value)?,
                        literal.as_str(),
                        *side,
                    )
                    .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::BinaryFloatLiteral {
                    dst,
                    op,
                    value,
                    literal,
                    side,
                } => {
                    let value = binary_float_literal_numeric(
                        *op,
                        &frame.read(*value)?,
                        literal.as_str(),
                        *side,
                    )
                    .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::Equal { dst, lhs, rhs } => {
                    let value = Value::Bool(values_equal(
                        &frame.read(*lhs)?,
                        &frame.read(*rhs)?,
                        heap.as_deref(),
                    )?);
                    frame.write(*dst, value)?;
                }
                InstructionKind::NotEqual { dst, lhs, rhs } => {
                    let value = Value::Bool(!values_equal(
                        &frame.read(*lhs)?,
                        &frame.read(*rhs)?,
                        heap.as_deref(),
                    )?);
                    frame.write(*dst, value)?;
                }
                InstructionKind::Less { dst, lhs, rhs } => {
                    let value = less_numeric(&frame.read(*lhs)?, &frame.read(*rhs)?)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write(*dst, Value::Bool(value))?;
                }
                InstructionKind::LessEqual { dst, lhs, rhs } => {
                    let value = less_equal_numeric(&frame.read(*lhs)?, &frame.read(*rhs)?)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write(*dst, Value::Bool(value))?;
                }
                InstructionKind::Greater { dst, lhs, rhs } => {
                    let value = greater_numeric(&frame.read(*lhs)?, &frame.read(*rhs)?)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write(*dst, Value::Bool(value))?;
                }
                InstructionKind::GreaterEqual { dst, lhs, rhs } => {
                    let value = greater_equal_numeric(&frame.read(*lhs)?, &frame.read(*rhs)?)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write(*dst, Value::Bool(value))?;
                }
                InstructionKind::I64Add { dst, lhs, rhs } => {
                    let lhs = frame
                        .read_i64(*lhs, "add")
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    let rhs = frame
                        .read_i64(*rhs, "add")
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    let value = i64_ops::add_raw(lhs, rhs)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write_i64(*dst, value)?;
                }
                InstructionKind::I64Sub { dst, lhs, rhs } => {
                    let lhs = frame
                        .read_i64(*lhs, "sub")
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    let rhs = frame
                        .read_i64(*rhs, "sub")
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    let value = i64_ops::sub_raw(lhs, rhs)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write_i64(*dst, value)?;
                }
                InstructionKind::I64Mul { dst, lhs, rhs } => {
                    let lhs = frame
                        .read_i64(*lhs, "mul")
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    let rhs = frame
                        .read_i64(*rhs, "mul")
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    let value = i64_ops::mul_raw(lhs, rhs)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write_i64(*dst, value)?;
                }
                InstructionKind::I64Rem { dst, lhs, rhs } => {
                    let lhs = frame
                        .read_i64(*lhs, "rem")
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    let rhs = frame
                        .read_i64(*rhs, "rem")
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    let value = i64_ops::rem_raw(lhs, rhs)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write_i64(*dst, value)?;
                }
                InstructionKind::I64AddImm { dst, lhs, imm } => {
                    let lhs = frame
                        .read_i64(*lhs, "add")
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    let value = i64_ops::add_raw(lhs, *imm)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write_i64(*dst, value)?;
                }
                InstructionKind::I64SubImm { dst, lhs, imm } => {
                    let lhs = frame
                        .read_i64(*lhs, "sub")
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    let value = i64_ops::sub_raw(lhs, *imm)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write_i64(*dst, value)?;
                }
                InstructionKind::I64MulImm { dst, lhs, imm } => {
                    let lhs = frame
                        .read_i64(*lhs, "mul")
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    let value = i64_ops::mul_raw(lhs, *imm)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write_i64(*dst, value)?;
                }
                InstructionKind::I64RemImm { dst, lhs, imm } => {
                    let lhs = frame
                        .read_i64(*lhs, "rem")
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    let value = i64_ops::rem_raw(lhs, *imm)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write_i64(*dst, value)?;
                }
                InstructionKind::I64CmpImm { dst, op, lhs, imm } => {
                    let lhs = frame
                        .read_i64(*lhs, "compare")
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write_bool(*dst, i64_ops::compare(lhs, *op, *imm))?;
                }
                InstructionKind::I64CmpImmJumpIfFalse {
                    op,
                    lhs,
                    imm,
                    target,
                } => {
                    let lhs = frame
                        .read_i64(*lhs, "compare")
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    if !i64_ops::compare(lhs, *op, *imm) {
                        debug_assert!(target.0 <= code.instructions.len());
                        ip = target.0;
                    }
                }
                InstructionKind::GuardType { src, guard } => {
                    runtime_type_guards::execute_linked_register_guard(
                        code,
                        call.program,
                        &frame,
                        *src,
                        *guard,
                        heap.as_deref(),
                    )
                    .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                }
                InstructionKind::JumpIfFalse { condition, target } => {
                    let jump = match frame.read_bool_lane(*condition)? {
                        Some(condition) => !condition,
                        None => !is_truthy(&frame.read(*condition)?),
                    };
                    if jump {
                        debug_assert!(target.0 <= code.instructions.len());
                        ip = target.0;
                    }
                }
                InstructionKind::JumpIfNotMissing { value, target } => {
                    if !matches!(frame.read(*value)?, Value::Missing) {
                        debug_assert!(target.0 <= code.instructions.len());
                        ip = target.0;
                    }
                }
                InstructionKind::Jump { target } => {
                    debug_assert!(target.0 <= code.instructions.len());
                    ip = target.0;
                }
                InstructionKind::CallNative {
                    dst,
                    native,
                    debug_name,
                    cache_site,
                    args,
                } => {
                    native_function_calls::dispatch_linked_native_function_call(
                        self,
                        &mut host,
                        &mut heap,
                        &mut budget,
                        &mut frame,
                        native_function_calls::LinkedNativeFunctionCall {
                            dst: *dst,
                            program: call.program,
                            native: *native,
                            debug_name: *debug_name,
                            cache_site: *cache_site,
                            inline_caches: call.inline_caches,
                            args,
                            call_site: instruction.span,
                        },
                    )?;
                }
                InstructionKind::CallFunction {
                    dst,
                    function,
                    debug_name,
                    mode,
                    args,
                } => {
                    script_function_calls::dispatch_linked_script_function_call(
                        self,
                        script_function_calls::LinkedScriptFunctionCallContext {
                            program: call.program,
                            inline_caches: call.inline_caches,
                            call_site: instruction.span,
                            call_site_offset: Some(instruction_offset),
                            bytecode_profiler: call.bytecode_profiler,
                        },
                        &mut host,
                        &mut heap,
                        &mut budget,
                        &mut frame,
                        script_function_calls::LinkedScriptFunctionCall {
                            dst: *dst,
                            function: *function,
                            debug_name: *debug_name,
                            mode: *mode,
                            args,
                        },
                    )?;
                }
                InstructionKind::MakeClosure {
                    dst,
                    function,
                    captures,
                } => {
                    closure_calls::make_linked_closure(
                        &mut heap,
                        &mut budget,
                        &mut frame,
                        closure_calls::LinkedMakeClosure {
                            dst: *dst,
                            function: *function,
                            captures,
                            call_site: instruction.span,
                        },
                    )?;
                }
                InstructionKind::CallClosure { dst, callee, args } => {
                    closure_calls::dispatch_linked_closure_call(
                        self,
                        closure_calls::LinkedClosureCallContext {
                            program: call.program,
                            inline_caches: call.inline_caches,
                            call_site: instruction.span,
                            call_site_offset: instruction_offset,
                            bytecode_profiler: call.bytecode_profiler,
                        },
                        &mut host,
                        &mut heap,
                        &mut budget,
                        &mut frame,
                        closure_calls::LinkedClosureCall {
                            dst: *dst,
                            callee: *callee,
                            args,
                        },
                    )?;
                }
                InstructionKind::CallMethod {
                    dst,
                    receiver,
                    dispatch,
                    debug_name,
                    cache_site,
                    args,
                } => {
                    script_method_calls::dispatch_linked_method_call(
                        self,
                        script_method_calls::LinkedScriptMethodCallContext {
                            program: call.program,
                            inline_caches: call.inline_caches,
                            cache_site: *cache_site,
                            call_site: instruction.span,
                            call_site_offset: Some(instruction_offset),
                            bytecode_profiler: call.bytecode_profiler,
                        },
                        &mut host,
                        &mut heap,
                        &mut budget,
                        &mut frame,
                        script_method_calls::LinkedScriptMethodCall {
                            dst: *dst,
                            receiver: *receiver,
                            dispatch: *dispatch,
                            debug_name: *debug_name,
                            args,
                        },
                    )?;
                }
                InstructionKind::CallDynamicMethod {
                    dst,
                    receiver,
                    method_name,
                    cache_site,
                    args,
                } => {
                    script_method_calls::dispatch_linked_dynamic_method_call(
                        self,
                        script_method_calls::LinkedScriptMethodCallContext {
                            program: call.program,
                            inline_caches: call.inline_caches,
                            cache_site: *cache_site,
                            call_site: instruction.span,
                            call_site_offset: Some(instruction_offset),
                            bytecode_profiler: call.bytecode_profiler,
                        },
                        &mut host,
                        &mut heap,
                        &mut budget,
                        &mut frame,
                        script_method_calls::LinkedDynamicMethodCall {
                            dst: *dst,
                            receiver: *receiver,
                            method_name: *method_name,
                            args,
                        },
                    )?;
                }
                InstructionKind::TryPropagate { dst, src } => {
                    if let Some(value) = try_propagation::dispatch_try_propagate(
                        &mut frame,
                        heap.as_deref(),
                        *dst,
                        *src,
                    )? {
                        return runtime_type_guards::execute_linked_return_guard(
                            code,
                            call.program,
                            value,
                            heap.as_deref(),
                        );
                    }
                }
                InstructionKind::MakeArray { dst, elements } => {
                    script_aggregate_construction::make_array(
                        &mut frame,
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                        *dst,
                        elements,
                    )?;
                }
                InstructionKind::MakeMap { dst, entries } => {
                    script_aggregate_construction::make_linked_map(
                        &mut frame,
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                        *dst,
                        code,
                        entries,
                        instruction.span,
                    )?;
                }
                InstructionKind::MakeRange {
                    dst,
                    start,
                    end,
                    inclusive,
                } => {
                    script_aggregate_construction::make_range(
                        &mut frame, *dst, *start, *end, *inclusive,
                    )?;
                }
                InstructionKind::MakeRecord { dst, ty, fields } => {
                    script_object_construction::make_linked_record(
                        &mut frame,
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                        *dst,
                        call.program,
                        *ty,
                        fields,
                    )?;
                }
                InstructionKind::GetRecordSlot {
                    dst,
                    record,
                    field,
                    debug_name,
                    cache_site,
                } => {
                    field_access::dispatch_linked_get_record_slot(
                        &mut frame,
                        heap.as_deref_mut(),
                        call.program,
                        field_access::LinkedRecordSlotRead {
                            dst: *dst,
                            record: *record,
                            field: *field,
                            debug_name: *debug_name,
                        },
                        call.inline_caches,
                        *cache_site,
                    )?;
                }
                InstructionKind::SetRecordSlot {
                    record,
                    field,
                    debug_name,
                    cache_site,
                    src,
                } => {
                    field_access::dispatch_linked_set_record_slot(
                        &mut frame,
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                        call.program,
                        field_access::LinkedRecordSlotWrite {
                            record: *record,
                            field: *field,
                            debug_name: *debug_name,
                            src: *src,
                        },
                        call.inline_caches,
                        *cache_site,
                    )?;
                }
                InstructionKind::MakeEnum {
                    dst,
                    enum_ty,
                    variant,
                    fields,
                } => {
                    script_object_construction::make_linked_enum(
                        &mut frame,
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                        *dst,
                        call.program,
                        script_object_construction::LinkedEnumConstruction {
                            enum_ty: *enum_ty,
                            variant: *variant,
                            fields,
                        },
                    )?;
                }
                InstructionKind::GetEnumSlot {
                    dst,
                    value,
                    field,
                    debug_name,
                } => {
                    field_access::dispatch_linked_get_enum_slot(
                        &mut frame,
                        heap.as_deref_mut(),
                        call.program,
                        *dst,
                        *value,
                        *field,
                        *debug_name,
                    )?;
                }
                InstructionKind::GetIndex { dst, base, index } => {
                    indexing::dispatch_get_index(&mut frame, heap.as_deref(), *dst, *base, *index)?;
                }
                InstructionKind::GetStringKeyIndex { dst, base, key } => {
                    let key =
                        string_key_constant(code.constants.get(key.0), key.0, instruction.span)?;
                    indexing::dispatch_get_string_key_index(
                        &mut frame,
                        heap.as_deref(),
                        *dst,
                        *base,
                        key,
                    )?;
                }
                InstructionKind::SetIndex { base, index, src } => {
                    indexing::dispatch_set_index(
                        &mut frame,
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                        *base,
                        *index,
                        *src,
                    )?;
                }
                InstructionKind::SetStringKeyIndex { base, key, src } => {
                    let key =
                        string_key_constant(code.constants.get(key.0), key.0, instruction.span)?;
                    indexing::dispatch_set_string_key_index(
                        &mut frame,
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                        *base,
                        key,
                        *src,
                    )?;
                }
                InstructionKind::IterInit { dst, iterable } => {
                    iteration::dispatch_iter_init(
                        iteration::IterRuntime {
                            frame: &mut frame,
                            heap: heap.as_deref_mut(),
                            budget: budget.as_deref_mut(),
                        },
                        *dst,
                        *iterable,
                    )?;
                }
                InstructionKind::IterNext {
                    iterator,
                    dst,
                    jump_if_done,
                } => {
                    if let Some(target) = iteration::dispatch_linked_iter_next(
                        iteration::IterRuntime {
                            frame: &mut frame,
                            heap: heap.as_deref_mut(),
                            budget: budget.as_deref_mut(),
                        },
                        code,
                        *iterator,
                        *dst,
                        *jump_if_done,
                    )? {
                        ip = target;
                    }
                }
                InstructionKind::RangeNext {
                    cursor,
                    end,
                    done,
                    inclusive,
                    dst,
                    jump_if_done,
                } => {
                    if let Some(target) = iteration::dispatch_linked_range_next(
                        iteration::IterRuntime {
                            frame: &mut frame,
                            heap: heap.as_deref_mut(),
                            budget: budget.as_deref_mut(),
                        },
                        code,
                        iteration::RangeNextStep {
                            cursor: *cursor,
                            end: *end,
                            done: *done,
                            inclusive: *inclusive,
                            dst: *dst,
                            jump_if_done: *jump_if_done,
                        },
                    )? {
                        ip = target;
                    }
                }
                InstructionKind::I64RangeNext {
                    cursor,
                    end,
                    done,
                    inclusive,
                    dst,
                    jump_if_done,
                } => {
                    if let Some(target) = iteration::dispatch_linked_i64_range_next(
                        &mut frame,
                        code,
                        iteration::RangeNextStep {
                            cursor: *cursor,
                            end: *end,
                            done: *done,
                            inclusive: *inclusive,
                            dst: *dst,
                            jump_if_done: *jump_if_done,
                        },
                    )? {
                        ip = target;
                    }
                }
                InstructionKind::EnumTagEqual {
                    dst,
                    value,
                    enum_ty,
                    variant,
                } => {
                    field_access::dispatch_linked_enum_tag_equal(
                        &mut frame,
                        heap.as_deref(),
                        call.program,
                        *dst,
                        *value,
                        *enum_ty,
                        *variant,
                    )?;
                }
                InstructionKind::LoadGlobal {
                    dst,
                    slot,
                    debug_name,
                    cache_site,
                } => {
                    let value = host_access::load_linked_cached_host_global(
                        host_access::HostAccessRuntime {
                            frame: &frame,
                            heap: heap.as_deref_mut(),
                            budget: budget.as_deref_mut(),
                            host: host.as_deref_mut(),
                            inline_caches: call.inline_caches,
                            source_span: instruction.span,
                        },
                        call.program,
                        *debug_name,
                        Some(*slot),
                        *cache_site,
                    )?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::HostRead {
                    dst,
                    root,
                    target,
                    dynamic_args,
                    cache_site,
                } => {
                    let value = host_access::execute_code_host_read(
                        host_access::HostAccessRuntime {
                            frame: &frame,
                            heap: heap.as_deref_mut(),
                            budget: budget.as_deref_mut(),
                            host: host.as_deref_mut(),
                            inline_caches: call.inline_caches,
                            source_span: instruction.span,
                        },
                        *root,
                        host_access::CodeHostTargetPlan {
                            targets: &code.host_targets,
                            target_id: *target,
                            dynamic_args,
                            cache_site: *cache_site,
                        },
                    )?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::HostWrite {
                    root,
                    target,
                    dynamic_args,
                    src,
                    cache_site,
                } => {
                    host_access::execute_code_host_write(
                        host_access::HostAccessRuntime {
                            frame: &frame,
                            heap: heap.as_deref_mut(),
                            budget: budget.as_deref_mut(),
                            host: host.as_deref_mut(),
                            inline_caches: call.inline_caches,
                            source_span: instruction.span,
                        },
                        *root,
                        host_access::CodeHostTargetPlan {
                            targets: &code.host_targets,
                            target_id: *target,
                            dynamic_args,
                            cache_site: *cache_site,
                        },
                        *src,
                    )?;
                }
                InstructionKind::HostMutate {
                    root,
                    target,
                    dynamic_args,
                    op,
                    rhs,
                    cache_site,
                } => {
                    host_access::execute_code_host_mutate(
                        host_access::HostAccessRuntime {
                            frame: &frame,
                            heap: heap.as_deref_mut(),
                            budget: budget.as_deref_mut(),
                            host: host.as_deref_mut(),
                            inline_caches: call.inline_caches,
                            source_span: instruction.span,
                        },
                        *root,
                        host_access::CodeHostMutationPlan {
                            target: host_access::CodeHostTargetPlan {
                                targets: &code.host_targets,
                                target_id: *target,
                                dynamic_args,
                                cache_site: *cache_site,
                            },
                            op: *op,
                            rhs: *rhs,
                        },
                    )?;
                }
                InstructionKind::HostRemove {
                    root,
                    target,
                    dynamic_args,
                    cache_site,
                } => {
                    host_access::execute_code_host_remove(
                        host_access::HostAccessRuntime {
                            frame: &frame,
                            heap: heap.as_deref_mut(),
                            budget: budget.as_deref_mut(),
                            host: host.as_deref_mut(),
                            inline_caches: call.inline_caches,
                            source_span: instruction.span,
                        },
                        *root,
                        host_access::CodeHostTargetPlan {
                            targets: &code.host_targets,
                            target_id: *target,
                            dynamic_args,
                            cache_site: *cache_site,
                        },
                    )?;
                }
                InstructionKind::HostCall {
                    dst,
                    root,
                    target,
                    dynamic_args,
                    method,
                    args,
                    cache_site,
                    ..
                } => {
                    let value = host_access::execute_linked_code_host_call(
                        host_access::HostAccessRuntime {
                            frame: &frame,
                            heap: heap.as_deref_mut(),
                            budget: budget.as_deref_mut(),
                            host: host.as_deref_mut(),
                            inline_caches: call.inline_caches,
                            source_span: instruction.span,
                        },
                        *root,
                        host_access::LinkedCodeHostCallPlan {
                            program: call.program,
                            target: host_access::CodeHostTargetPlan {
                                targets: &code.host_targets,
                                target_id: *target,
                                dynamic_args,
                                cache_site: *cache_site,
                            },
                            method: *method,
                            args,
                            wants_return: dst.is_some(),
                        },
                    )?;
                    if let (Some(dst), Some(value)) = (dst, value) {
                        frame.write(*dst, value)?;
                    }
                }
                InstructionKind::Return { src } => {
                    return runtime_type_guards::execute_linked_return_guard(
                        code,
                        call.program,
                        frame.read(*src)?,
                        heap.as_deref(),
                    );
                }
            }

            if let Some(heap) = heap.as_deref_mut()
                && heap.needs_safe_point()
            {
                heap.collect_frame_at_safe_point(&frame, budget.as_deref_mut());
            }
        }

        Err(VmError::new(VmErrorKind::MissingReturn))
    }
}

fn string_key_constant(
    constant: Option<&vela_bytecode::Constant>,
    constant_index: usize,
    span: Option<Span>,
) -> VmResult<&str> {
    match constant {
        Some(vela_bytecode::Constant::String(value)) => Ok(value),
        Some(_) => Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "map string key constant",
        })
        .with_source_span(span)),
        None => Err(VmError::new(VmErrorKind::ConstantOutOfBounds {
            constant: constant_index,
        })
        .with_source_span(span)),
    }
}
