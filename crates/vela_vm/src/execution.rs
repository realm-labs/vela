use super::*;

impl Vm {
    pub(super) fn execute_body(
        &self,
        call: ExecutionCall<'_>,
        mut host: Option<&mut HostExecution<'_>>,
        mut heap: Option<&mut HeapExecution<'_>>,
        mut budget: Option<&mut ExecutionBudget>,
    ) -> VmResult<Value> {
        let code = call.code;
        validate_inline_cache_layout(call.inline_caches, code.cache_sites.len())?;
        let program = call.program;
        let captures = call.captures;
        let args = call.args;
        if captures.len() != usize::from(code.capture_count) {
            return Err(VmError::new(VmErrorKind::ArityMismatch {
                name: code.name.clone(),
                expected: usize::from(code.capture_count),
                actual: captures.len(),
            }));
        }
        if args.len() > code.params.len() {
            return Err(VmError::new(VmErrorKind::ArityMismatch {
                name: code.name.clone(),
                expected: code.params.len(),
                actual: args.len(),
            }));
        }

        let mut frame = CallFrame::new(code.register_count);
        for (index, capture) in captures.iter().enumerate() {
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
        for (index, arg) in args.iter().enumerate() {
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
        for index in args.len()..code.params.len() {
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
        let actual = args
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
                    name: code.name.clone(),
                    expected: code.params.len(),
                    actual,
                }));
            }
        }
        if call.check_param_guards {
            execute_unlinked_param_guards(code, &frame, heap.as_deref())?;
        }
        let mut ip = 0_usize;

        while ip < code.instructions.len() {
            let instruction_offset = InstructionOffset(ip);
            let instruction = &code.instructions[ip];
            if let Some(budget) = budget.as_deref_mut() {
                budget.charge_instruction()?;
            }
            ip = ip.saturating_add(1);

            match &instruction.kind {
                UnlinkedInstructionKind::LoadConst { dst, constant } => {
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
                UnlinkedInstructionKind::Move { dst, src } => {
                    let value = frame.read(*src)?;
                    frame.write(*dst, value)?;
                }
                UnlinkedInstructionKind::Not { dst, src } => {
                    let value = Value::Bool(!is_truthy(&frame.read(*src)?));
                    frame.write(*dst, value)?;
                }
                UnlinkedInstructionKind::Truthy { dst, src } => {
                    let value = Value::Bool(is_truthy(&frame.read(*src)?));
                    frame.write(*dst, value)?;
                }
                UnlinkedInstructionKind::Negate { dst, src } => {
                    let value = negate_numeric(&frame.read(*src)?)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write(*dst, value)?;
                }
                UnlinkedInstructionKind::Add { dst, lhs, rhs } => {
                    let value = add_numeric(&frame.read(*lhs)?, &frame.read(*rhs)?)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write(*dst, value)?;
                }
                UnlinkedInstructionKind::Sub { dst, lhs, rhs } => {
                    let value = sub_numeric(&frame.read(*lhs)?, &frame.read(*rhs)?)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write(*dst, value)?;
                }
                UnlinkedInstructionKind::Mul { dst, lhs, rhs } => {
                    let value = mul_numeric(&frame.read(*lhs)?, &frame.read(*rhs)?)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write(*dst, value)?;
                }
                UnlinkedInstructionKind::Div { dst, lhs, rhs } => {
                    let value = div_numeric(&frame.read(*lhs)?, &frame.read(*rhs)?)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write(*dst, value)?;
                }
                UnlinkedInstructionKind::Rem { dst, lhs, rhs } => {
                    let value = rem_numeric(&frame.read(*lhs)?, &frame.read(*rhs)?)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write(*dst, value)?;
                }
                UnlinkedInstructionKind::BinaryIntLiteral {
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
                UnlinkedInstructionKind::BinaryFloatLiteral {
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
                UnlinkedInstructionKind::Equal { dst, lhs, rhs } => {
                    let value = Value::Bool(values_equal(
                        &frame.read(*lhs)?,
                        &frame.read(*rhs)?,
                        heap.as_deref(),
                    )?);
                    frame.write(*dst, value)?;
                }
                UnlinkedInstructionKind::NotEqual { dst, lhs, rhs } => {
                    let value = Value::Bool(!values_equal(
                        &frame.read(*lhs)?,
                        &frame.read(*rhs)?,
                        heap.as_deref(),
                    )?);
                    frame.write(*dst, value)?;
                }
                UnlinkedInstructionKind::Less { dst, lhs, rhs } => {
                    let value = less_numeric(&frame.read(*lhs)?, &frame.read(*rhs)?)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write(*dst, Value::Bool(value))?;
                }
                UnlinkedInstructionKind::LessEqual { dst, lhs, rhs } => {
                    let value = less_equal_numeric(&frame.read(*lhs)?, &frame.read(*rhs)?)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write(*dst, Value::Bool(value))?;
                }
                UnlinkedInstructionKind::Greater { dst, lhs, rhs } => {
                    let value = greater_numeric(&frame.read(*lhs)?, &frame.read(*rhs)?)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write(*dst, Value::Bool(value))?;
                }
                UnlinkedInstructionKind::GreaterEqual { dst, lhs, rhs } => {
                    let value = greater_equal_numeric(&frame.read(*lhs)?, &frame.read(*rhs)?)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write(*dst, Value::Bool(value))?;
                }
                UnlinkedInstructionKind::I64Add { dst, lhs, rhs } => {
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
                UnlinkedInstructionKind::I64Sub { dst, lhs, rhs } => {
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
                UnlinkedInstructionKind::I64Mul { dst, lhs, rhs } => {
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
                UnlinkedInstructionKind::I64Rem { dst, lhs, rhs } => {
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
                UnlinkedInstructionKind::I64AddImm { dst, lhs, imm } => {
                    let lhs = frame
                        .read_i64(*lhs, "add")
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    let value = i64_ops::add_raw(lhs, *imm)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write_i64(*dst, value)?;
                }
                UnlinkedInstructionKind::I64SubImm { dst, lhs, imm } => {
                    let lhs = frame
                        .read_i64(*lhs, "sub")
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    let value = i64_ops::sub_raw(lhs, *imm)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write_i64(*dst, value)?;
                }
                UnlinkedInstructionKind::I64MulImm { dst, lhs, imm } => {
                    let lhs = frame
                        .read_i64(*lhs, "mul")
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    let value = i64_ops::mul_raw(lhs, *imm)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write_i64(*dst, value)?;
                }
                UnlinkedInstructionKind::I64RemImm { dst, lhs, imm } => {
                    let lhs = frame
                        .read_i64(*lhs, "rem")
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    let value = i64_ops::rem_raw(lhs, *imm)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write_i64(*dst, value)?;
                }
                UnlinkedInstructionKind::I64EqImm { dst, lhs, imm } => {
                    let lhs = frame
                        .read_i64(*lhs, "equal")
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write_bool(*dst, lhs == *imm)?;
                }
                UnlinkedInstructionKind::I64GtImm { dst, lhs, imm } => {
                    let lhs = frame
                        .read_i64(*lhs, "greater")
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write_bool(*dst, lhs > *imm)?;
                }
                UnlinkedInstructionKind::I64EqImmJumpIfFalse { lhs, imm, target } => {
                    let lhs = frame
                        .read_i64(*lhs, "equal")
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    if lhs != *imm {
                        validate_jump(code, target.0)?;
                        ip = target.0;
                    }
                }
                UnlinkedInstructionKind::I64GtImmJumpIfFalse { lhs, imm, target } => {
                    let lhs = frame
                        .read_i64(*lhs, "greater")
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    if lhs <= *imm {
                        validate_jump(code, target.0)?;
                        ip = target.0;
                    }
                }
                UnlinkedInstructionKind::I64RemImmEqImmJumpIfFalse {
                    lhs,
                    rem_imm,
                    eq_imm,
                    target,
                } => {
                    let lhs = frame
                        .read_i64(*lhs, "rem")
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    let value = i64_ops::rem_raw(lhs, *rem_imm)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    if value != *eq_imm {
                        validate_jump(code, target.0)?;
                        ip = target.0;
                    }
                }
                UnlinkedInstructionKind::GuardType { src, guard } => {
                    runtime_type_guards::execute_unlinked_guard(
                        &frame.read(*src)?,
                        guard,
                        heap.as_deref(),
                    )
                    .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                }
                UnlinkedInstructionKind::JumpIfFalse { condition, target } => {
                    let jump = match frame.read_bool_lane(*condition)? {
                        Some(condition) => !condition,
                        None => !is_truthy(&frame.read(*condition)?),
                    };
                    if jump {
                        validate_jump(code, target.0)?;
                        ip = target.0;
                    }
                }
                UnlinkedInstructionKind::JumpIfNotMissing { value, target } => {
                    if !matches!(frame.read(*value)?, Value::Missing) {
                        validate_jump(code, target.0)?;
                        ip = target.0;
                    }
                }
                UnlinkedInstructionKind::Jump { target } => {
                    validate_jump(code, target.0)?;
                    ip = target.0;
                }
                UnlinkedInstructionKind::CallNative {
                    dst,
                    name,
                    native,
                    cache_site: _,
                    args,
                } => {
                    native_function_calls::dispatch_native_function_call(
                        self,
                        &mut host,
                        &mut heap,
                        &mut budget,
                        &mut frame,
                        native_function_calls::NativeFunctionCall {
                            dst: *dst,
                            name,
                            native: *native,
                            args,
                            call_site: instruction.span,
                        },
                    )?;
                }
                UnlinkedInstructionKind::CallFunction {
                    dst,
                    target,
                    name,
                    mode,
                    args,
                } => {
                    script_function_calls::dispatch_script_function_call(
                        self,
                        program,
                        &mut host,
                        &mut heap,
                        &mut budget,
                        &mut frame,
                        script_function_calls::ScriptFunctionCall {
                            dst: *dst,
                            target: *target,
                            name,
                            mode: *mode,
                            args,
                            call_site: instruction.span,
                            call_site_offset: instruction_offset,
                        },
                    )?;
                }
                UnlinkedInstructionKind::MakeClosure {
                    dst,
                    function,
                    captures,
                } => {
                    closure_calls::make_closure(
                        &mut heap,
                        &mut budget,
                        &mut frame,
                        closure_calls::MakeClosure {
                            dst: *dst,
                            program,
                            owner: code,
                            function: *function,
                            captures,
                        },
                    )?;
                }
                UnlinkedInstructionKind::CallClosure { dst, callee, args } => {
                    closure_calls::dispatch_closure_call(
                        self,
                        program,
                        &mut host,
                        &mut heap,
                        &mut budget,
                        &mut frame,
                        closure_calls::ClosureCall {
                            dst: *dst,
                            callee: *callee,
                            args,
                            call_site: instruction.span,
                            call_site_offset: instruction_offset,
                        },
                    )?;
                }
                UnlinkedInstructionKind::CallDynamicMethod {
                    dst,
                    receiver,
                    method,
                    args,
                } => {
                    let positional_args = args
                        .iter()
                        .map(|arg| vela_bytecode::CallArgument::Register(arg.value))
                        .collect::<Vec<_>>();
                    script_method_calls::dispatch_script_method_register_call(
                        self,
                        program,
                        &mut host,
                        &mut heap,
                        &mut budget,
                        &mut frame,
                        script_method_calls::ScriptMethodRegisterCall {
                            dst: *dst,
                            receiver: *receiver,
                            method,
                            args: &positional_args,
                        },
                    )?;
                }
                UnlinkedInstructionKind::CallMethodId {
                    dst,
                    receiver,
                    method,
                    method_id,
                    args,
                } => {
                    script_method_calls::dispatch_script_method_id_register_call(
                        self,
                        program,
                        &mut host,
                        &mut heap,
                        &mut budget,
                        &mut frame,
                        script_method_calls::ScriptMethodIdRegisterCall {
                            dst: *dst,
                            receiver: *receiver,
                            method,
                            method_id: *method_id,
                            args,
                        },
                    )?;
                }
                UnlinkedInstructionKind::TryPropagate { dst, src } => {
                    if let Some(value) = try_propagation::dispatch_try_propagate(
                        &mut frame,
                        heap.as_deref(),
                        *dst,
                        *src,
                    )? {
                        return execute_unlinked_return_guard(code, value, heap.as_deref());
                    }
                }
                UnlinkedInstructionKind::MakeArray { dst, elements } => {
                    script_aggregate_construction::make_array(
                        &mut frame,
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                        *dst,
                        elements,
                    )?;
                }
                UnlinkedInstructionKind::MakeMap { dst, entries } => {
                    script_aggregate_construction::make_map(
                        &mut frame,
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                        *dst,
                        entries,
                    )?;
                }
                UnlinkedInstructionKind::MakeRange {
                    dst,
                    start,
                    end,
                    inclusive,
                } => {
                    script_aggregate_construction::make_range(
                        &mut frame, *dst, *start, *end, *inclusive,
                    )?;
                }
                UnlinkedInstructionKind::MakeRecord {
                    dst,
                    type_name,
                    fields,
                } => {
                    script_object_construction::make_record(
                        &mut frame,
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                        *dst,
                        type_name,
                        fields,
                    )?;
                }
                UnlinkedInstructionKind::MakeEnum {
                    dst,
                    enum_name,
                    variant,
                    fields,
                } => {
                    script_object_construction::make_enum(
                        &mut frame,
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                        *dst,
                        enum_name,
                        variant,
                        fields,
                    )?;
                }
                UnlinkedInstructionKind::GetRecordField { dst, record, field } => {
                    field_access::dispatch_get_record_field(
                        &mut frame,
                        heap.as_deref_mut(),
                        *dst,
                        *record,
                        field,
                    )?;
                }
                UnlinkedInstructionKind::GetRecordSlot {
                    dst,
                    record,
                    field,
                    slot,
                } => {
                    field_access::dispatch_get_record_slot(
                        &mut frame,
                        heap.as_deref_mut(),
                        *dst,
                        *record,
                        field,
                        *slot,
                    )?;
                }
                UnlinkedInstructionKind::SetRecordField { record, field, src } => {
                    field_access::dispatch_set_record_field(
                        &mut frame,
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                        *record,
                        field,
                        *src,
                    )?;
                }
                UnlinkedInstructionKind::SetRecordSlot {
                    record,
                    field,
                    slot,
                    src,
                } => {
                    field_access::dispatch_set_record_slot(
                        &mut frame,
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                        *record,
                        field,
                        *slot,
                        *src,
                    )?;
                }
                UnlinkedInstructionKind::GetEnumField { dst, value, field } => {
                    field_access::dispatch_get_enum_field(
                        &mut frame,
                        heap.as_deref_mut(),
                        *dst,
                        *value,
                        field,
                    )?;
                }
                UnlinkedInstructionKind::GetEnumSlot {
                    dst,
                    value,
                    field,
                    slot,
                } => {
                    field_access::dispatch_get_enum_slot(
                        &mut frame,
                        heap.as_deref_mut(),
                        *dst,
                        *value,
                        field,
                        *slot,
                    )?;
                }
                UnlinkedInstructionKind::GetIndex { dst, base, index } => {
                    indexing::dispatch_get_index(&mut frame, heap.as_deref(), *dst, *base, *index)?;
                }
                UnlinkedInstructionKind::SetIndex { base, index, src } => {
                    indexing::dispatch_set_index(
                        &mut frame,
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                        *base,
                        *index,
                        *src,
                    )?;
                }
                UnlinkedInstructionKind::IterInit { dst, iterable } => {
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
                UnlinkedInstructionKind::IterNext {
                    iterator,
                    dst,
                    jump_if_done,
                } => {
                    if let Some(target) = iteration::dispatch_iter_next(
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
                UnlinkedInstructionKind::RangeNext {
                    cursor,
                    end,
                    done,
                    inclusive,
                    dst,
                    jump_if_done,
                } => {
                    if let Some(target) = iteration::dispatch_range_next(
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
                UnlinkedInstructionKind::I64RangeNext {
                    cursor,
                    end,
                    done,
                    inclusive,
                    dst,
                    jump_if_done,
                } => {
                    if let Some(target) = iteration::dispatch_i64_range_next(
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
                UnlinkedInstructionKind::EnumTagEqual {
                    dst,
                    value,
                    enum_name,
                    variant,
                } => {
                    field_access::dispatch_enum_tag_equal(
                        &mut frame,
                        heap.as_deref(),
                        *dst,
                        *value,
                        enum_name,
                        variant,
                    )?;
                }
                UnlinkedInstructionKind::LoadGlobal {
                    dst,
                    global,
                    slot,
                    cache_site,
                } => {
                    let value = host_access::load_cached_host_global(
                        host_access::HostAccessRuntime {
                            frame: &frame,
                            heap: heap.as_deref_mut(),
                            budget: budget.as_deref_mut(),
                            host: host.as_deref_mut(),
                            inline_caches: call.inline_caches,
                            source_span: instruction.span,
                        },
                        global,
                        *slot,
                        *cache_site,
                    )?;
                    frame.write(*dst, value)?;
                }
                UnlinkedInstructionKind::HostRead {
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
                UnlinkedInstructionKind::HostWrite {
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
                UnlinkedInstructionKind::HostMutate {
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
                UnlinkedInstructionKind::HostRemove {
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
                UnlinkedInstructionKind::HostCall {
                    dst,
                    root,
                    target,
                    dynamic_args,
                    method,
                    args,
                    cache_site,
                } => {
                    let return_value = host_access::execute_code_host_call(
                        host_access::HostAccessRuntime {
                            frame: &frame,
                            heap: heap.as_deref_mut(),
                            budget: budget.as_deref_mut(),
                            host: host.as_deref_mut(),
                            inline_caches: call.inline_caches,
                            source_span: instruction.span,
                        },
                        *root,
                        host_access::CodeHostCallPlan {
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
                    if let (Some(dst), Some(return_value)) = (dst, return_value) {
                        frame.write(*dst, return_value)?;
                    }
                }
                UnlinkedInstructionKind::Return { src } => {
                    return execute_unlinked_return_guard(code, frame.read(*src)?, heap.as_deref());
                }
            }

            if let Some(heap) = heap.as_deref_mut() {
                heap.collect_frame_at_safe_point(&frame, budget.as_deref_mut());
            }
        }

        Err(VmError::new(VmErrorKind::MissingReturn))
    }
}

fn execute_unlinked_param_guards(
    code: &UnlinkedCodeObject,
    frame: &CallFrame,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<()> {
    for param_guard in &code.param_guards {
        let register = Register(
            code.capture_count
                .checked_add(param_guard.parameter)
                .ok_or_else(|| {
                    VmError::new(VmErrorKind::RegisterOutOfBounds {
                        register: Register(u16::MAX),
                    })
                })?,
        );
        let value = frame.read(register)?;
        if matches!(value, Value::Missing) {
            continue;
        }
        runtime_type_guards::execute_unlinked_guard(&value, &param_guard.guard, heap)?;
    }
    Ok(())
}

fn execute_unlinked_return_guard(
    code: &UnlinkedCodeObject,
    value: Value,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    let Some(guard) = &code.return_guard else {
        return Ok(value);
    };
    runtime_type_guards::execute_unlinked_guard(&value, guard, heap)?;
    Ok(value)
}
