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
        let mut ip = 0_usize;

        while ip < code.instructions.len() {
            let instruction_offset = InstructionOffset(ip);
            let instruction = &code.instructions[ip];
            if let Some(budget) = budget.as_deref_mut() {
                budget.charge_instruction()?;
            }
            ip = ip.saturating_add(1);

            match &instruction.kind {
                InstructionKind::LoadConst { dst, constant } => {
                    let constant_value = code.constants.get(constant.0).ok_or(VmError {
                        kind: VmErrorKind::ConstantOutOfBounds {
                            constant: constant.0,
                        },
                        source_span: instruction.span,
                        call_stack: Default::default(),
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
                        validate_jump(code, target.0)?;
                        ip = target.0;
                    }
                }
                InstructionKind::JumpIfNotMissing { value, target } => {
                    if !matches!(frame.read(*value)?, Value::Missing) {
                        validate_jump(code, target.0)?;
                        ip = target.0;
                    }
                }
                InstructionKind::Jump { target } => {
                    validate_jump(code, target.0)?;
                    ip = target.0;
                }
                InstructionKind::CallNative {
                    dst,
                    name,
                    native,
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
                InstructionKind::CallFunction { dst, name, args } => {
                    script_function_calls::dispatch_script_function_call(
                        self,
                        program,
                        &mut host,
                        &mut heap,
                        &mut budget,
                        &mut frame,
                        script_function_calls::ScriptFunctionCall {
                            dst: *dst,
                            name,
                            args,
                            call_site: instruction.span,
                            call_site_offset: instruction_offset,
                        },
                    )?;
                }
                InstructionKind::MakeClosure {
                    dst,
                    code,
                    captures,
                } => {
                    closure_calls::make_closure(
                        &mut heap,
                        &mut budget,
                        &mut frame,
                        closure_calls::MakeClosure {
                            dst: *dst,
                            code,
                            captures,
                        },
                    )?;
                }
                InstructionKind::CallClosure { dst, callee, args } => {
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
                InstructionKind::CallMethod {
                    dst,
                    receiver,
                    method,
                    value_method_id,
                    args,
                } => {
                    if args.is_empty() {
                        script_method_calls::dispatch_script_method_call(
                            self,
                            program,
                            &mut host,
                            &mut heap,
                            &mut budget,
                            &mut frame,
                            script_method_calls::ScriptMethodCall {
                                dst: *dst,
                                receiver: *receiver,
                                method,
                                value_method_id: *value_method_id,
                                values: &[],
                            },
                        )?;
                    } else {
                        let values = script_function_calls::script_call_args_from_call_arguments(
                            &frame, args,
                        )?;
                        script_method_calls::dispatch_script_method_call(
                            self,
                            program,
                            &mut host,
                            &mut heap,
                            &mut budget,
                            &mut frame,
                            script_method_calls::ScriptMethodCall {
                                dst: *dst,
                                receiver: *receiver,
                                method,
                                value_method_id: *value_method_id,
                                values: values.as_slice(),
                            },
                        )?;
                    }
                }
                InstructionKind::CallMethodId {
                    dst,
                    receiver,
                    method,
                    method_id,
                    args,
                } => {
                    if args.is_empty() {
                        script_method_calls::dispatch_script_method_id_call(
                            self,
                            program,
                            &mut host,
                            &mut heap,
                            &mut budget,
                            &mut frame,
                            script_method_calls::ScriptMethodIdCall {
                                dst: *dst,
                                receiver: *receiver,
                                method,
                                method_id: *method_id,
                                values: &[],
                            },
                        )?;
                    } else {
                        let values = script_function_calls::script_call_args_from_call_arguments(
                            &frame, args,
                        )?;
                        script_method_calls::dispatch_script_method_id_call(
                            self,
                            program,
                            &mut host,
                            &mut heap,
                            &mut budget,
                            &mut frame,
                            script_method_calls::ScriptMethodIdCall {
                                dst: *dst,
                                receiver: *receiver,
                                method,
                                method_id: *method_id,
                                values: values.as_slice(),
                            },
                        )?;
                    }
                }
                InstructionKind::TryPropagate { dst, src } => {
                    match try_propagate_value(frame.read(*src)?, heap.as_deref())? {
                        TryPropagation::Continue(value) => frame.write(*dst, value)?,
                        TryPropagation::Return(value) => return Ok(value),
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
                    script_aggregate_construction::make_map(
                        &mut frame,
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                        *dst,
                        entries,
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
                InstructionKind::MakeRecord {
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
                InstructionKind::MakeEnum {
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
                InstructionKind::GetRecordField { dst, record, field } => {
                    field_access::dispatch_get_record_field(
                        &mut frame,
                        heap.as_deref_mut(),
                        *dst,
                        *record,
                        field,
                    )?;
                }
                InstructionKind::GetRecordSlot {
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
                InstructionKind::SetRecordField { record, field, src } => {
                    field_access::dispatch_set_record_field(
                        &mut frame,
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                        *record,
                        field,
                        *src,
                    )?;
                }
                InstructionKind::SetRecordSlot {
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
                InstructionKind::GetEnumField { dst, value, field } => {
                    field_access::dispatch_get_enum_field(
                        &mut frame,
                        heap.as_deref_mut(),
                        *dst,
                        *value,
                        field,
                    )?;
                }
                InstructionKind::GetEnumSlot {
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
                InstructionKind::GetIndex { dst, base, index } => {
                    let value = indexing::get_index(
                        frame.read(*base)?,
                        frame.read(*index)?,
                        heap.as_deref(),
                    )?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::SetIndex { base, index, src } => {
                    let mut base_value = *frame.read(*base)?;
                    indexing::set_index(
                        &mut base_value,
                        frame.read(*index)?,
                        frame.read(*src)?,
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                    )?;
                    frame.write(*base, base_value)?;
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
                InstructionKind::RangeNext {
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
                InstructionKind::EnumTagEqual {
                    dst,
                    value,
                    enum_name,
                    variant,
                } => {
                    let matches = field_access::enum_tag_equal(
                        frame.read(*value)?,
                        enum_name,
                        variant,
                        heap.as_deref(),
                    );
                    frame.write(*dst, Value::Bool(matches))?;
                }
                InstructionKind::GetHostField { dst, root, field } => {
                    let value = host_access::read_host_field(
                        host_access::HostAccessRuntime {
                            frame: &frame,
                            heap: heap.as_deref_mut(),
                            budget: budget.as_deref_mut(),
                            host: host.as_deref_mut(),
                            source_span: instruction.span,
                        },
                        *root,
                        *field,
                    )?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::GetHostPath {
                    dst,
                    root,
                    segments,
                } => {
                    let mut symbols = self.host_path_symbols.borrow_mut();
                    let value = host_access::read_host_path(
                        host_access::HostAccessRuntime {
                            frame: &frame,
                            heap: heap.as_deref_mut(),
                            budget: budget.as_deref_mut(),
                            host: host.as_deref_mut(),
                            source_span: instruction.span,
                        },
                        *root,
                        segments,
                        &mut symbols,
                    )?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::SetHostField { root, field, src } => {
                    host_access::set_host_field(
                        host_access::HostAccessRuntime {
                            frame: &frame,
                            heap: heap.as_deref_mut(),
                            budget: budget.as_deref_mut(),
                            host: host.as_deref_mut(),
                            source_span: instruction.span,
                        },
                        *root,
                        *field,
                        *src,
                    )?;
                }
                InstructionKind::SetHostPath {
                    root,
                    segments,
                    src,
                } => {
                    let mut symbols = self.host_path_symbols.borrow_mut();
                    host_access::set_host_path(
                        host_access::HostAccessRuntime {
                            frame: &frame,
                            heap: heap.as_deref_mut(),
                            budget: budget.as_deref_mut(),
                            host: host.as_deref_mut(),
                            source_span: instruction.span,
                        },
                        *root,
                        segments,
                        *src,
                        &mut symbols,
                    )?;
                }
                InstructionKind::AddHostField { root, field, rhs } => {
                    host_access::write_host_field_numeric_patch(
                        host_access::HostAccessRuntime {
                            frame: &frame,
                            heap: heap.as_deref_mut(),
                            budget: budget.as_deref_mut(),
                            host: host.as_deref_mut(),
                            source_span: instruction.span,
                        },
                        *root,
                        *field,
                        *rhs,
                        host_access::HostNumericPatch::Add,
                    )?;
                }
                InstructionKind::SubHostField { root, field, rhs } => {
                    host_access::write_host_field_numeric_patch(
                        host_access::HostAccessRuntime {
                            frame: &frame,
                            heap: heap.as_deref_mut(),
                            budget: budget.as_deref_mut(),
                            host: host.as_deref_mut(),
                            source_span: instruction.span,
                        },
                        *root,
                        *field,
                        *rhs,
                        host_access::HostNumericPatch::Sub,
                    )?;
                }
                InstructionKind::MulHostField { root, field, rhs } => {
                    host_access::write_host_field_numeric_patch(
                        host_access::HostAccessRuntime {
                            frame: &frame,
                            heap: heap.as_deref_mut(),
                            budget: budget.as_deref_mut(),
                            host: host.as_deref_mut(),
                            source_span: instruction.span,
                        },
                        *root,
                        *field,
                        *rhs,
                        host_access::HostNumericPatch::Mul,
                    )?;
                }
                InstructionKind::DivHostField { root, field, rhs } => {
                    host_access::write_host_field_numeric_patch(
                        host_access::HostAccessRuntime {
                            frame: &frame,
                            heap: heap.as_deref_mut(),
                            budget: budget.as_deref_mut(),
                            host: host.as_deref_mut(),
                            source_span: instruction.span,
                        },
                        *root,
                        *field,
                        *rhs,
                        host_access::HostNumericPatch::Div,
                    )?;
                }
                InstructionKind::RemHostField { root, field, rhs } => {
                    host_access::write_host_field_numeric_patch(
                        host_access::HostAccessRuntime {
                            frame: &frame,
                            heap: heap.as_deref_mut(),
                            budget: budget.as_deref_mut(),
                            host: host.as_deref_mut(),
                            source_span: instruction.span,
                        },
                        *root,
                        *field,
                        *rhs,
                        host_access::HostNumericPatch::Rem,
                    )?;
                }
                InstructionKind::AddHostPath {
                    root,
                    segments,
                    rhs,
                } => {
                    let mut symbols = self.host_path_symbols.borrow_mut();
                    host_access::write_host_path_numeric_patch(
                        host_access::HostAccessRuntime {
                            frame: &frame,
                            heap: heap.as_deref_mut(),
                            budget: budget.as_deref_mut(),
                            host: host.as_deref_mut(),
                            source_span: instruction.span,
                        },
                        *root,
                        segments,
                        *rhs,
                        host_access::HostNumericPatch::Add,
                        &mut symbols,
                    )?;
                }
                InstructionKind::SubHostPath {
                    root,
                    segments,
                    rhs,
                } => {
                    let mut symbols = self.host_path_symbols.borrow_mut();
                    host_access::write_host_path_numeric_patch(
                        host_access::HostAccessRuntime {
                            frame: &frame,
                            heap: heap.as_deref_mut(),
                            budget: budget.as_deref_mut(),
                            host: host.as_deref_mut(),
                            source_span: instruction.span,
                        },
                        *root,
                        segments,
                        *rhs,
                        host_access::HostNumericPatch::Sub,
                        &mut symbols,
                    )?;
                }
                InstructionKind::MulHostPath {
                    root,
                    segments,
                    rhs,
                } => {
                    let mut symbols = self.host_path_symbols.borrow_mut();
                    host_access::write_host_path_numeric_patch(
                        host_access::HostAccessRuntime {
                            frame: &frame,
                            heap: heap.as_deref_mut(),
                            budget: budget.as_deref_mut(),
                            host: host.as_deref_mut(),
                            source_span: instruction.span,
                        },
                        *root,
                        segments,
                        *rhs,
                        host_access::HostNumericPatch::Mul,
                        &mut symbols,
                    )?;
                }
                InstructionKind::DivHostPath {
                    root,
                    segments,
                    rhs,
                } => {
                    let mut symbols = self.host_path_symbols.borrow_mut();
                    host_access::write_host_path_numeric_patch(
                        host_access::HostAccessRuntime {
                            frame: &frame,
                            heap: heap.as_deref_mut(),
                            budget: budget.as_deref_mut(),
                            host: host.as_deref_mut(),
                            source_span: instruction.span,
                        },
                        *root,
                        segments,
                        *rhs,
                        host_access::HostNumericPatch::Div,
                        &mut symbols,
                    )?;
                }
                InstructionKind::RemHostPath {
                    root,
                    segments,
                    rhs,
                } => {
                    let mut symbols = self.host_path_symbols.borrow_mut();
                    host_access::write_host_path_numeric_patch(
                        host_access::HostAccessRuntime {
                            frame: &frame,
                            heap: heap.as_deref_mut(),
                            budget: budget.as_deref_mut(),
                            host: host.as_deref_mut(),
                            source_span: instruction.span,
                        },
                        *root,
                        segments,
                        *rhs,
                        host_access::HostNumericPatch::Rem,
                        &mut symbols,
                    )?;
                }
                InstructionKind::PushHostPath {
                    root,
                    segments,
                    value,
                } => {
                    let mut symbols = self.host_path_symbols.borrow_mut();
                    host_access::push_host_path(
                        host_access::HostAccessRuntime {
                            frame: &frame,
                            heap: heap.as_deref_mut(),
                            budget: budget.as_deref_mut(),
                            host: host.as_deref_mut(),
                            source_span: instruction.span,
                        },
                        *root,
                        segments,
                        *value,
                        &mut symbols,
                    )?;
                }
                InstructionKind::RemoveHostPath { root, segments } => {
                    let mut symbols = self.host_path_symbols.borrow_mut();
                    host_access::remove_host_path(
                        host_access::HostAccessRuntime {
                            frame: &frame,
                            heap: heap.as_deref_mut(),
                            budget: budget.as_deref_mut(),
                            host: host.as_deref_mut(),
                            source_span: instruction.span,
                        },
                        *root,
                        segments,
                        &mut symbols,
                    )?;
                }
                InstructionKind::CallHostMethod {
                    dst,
                    root,
                    segments,
                    method,
                    args,
                } => {
                    let mut symbols = self.host_path_symbols.borrow_mut();
                    let return_value = host_access::call_host_method(
                        host_access::HostAccessRuntime {
                            frame: &frame,
                            heap: heap.as_deref_mut(),
                            budget: budget.as_deref_mut(),
                            host: host.as_deref_mut(),
                            source_span: instruction.span,
                        },
                        *root,
                        segments,
                        *method,
                        args,
                        dst.is_some(),
                        &mut symbols,
                    )?;
                    if let (Some(dst), Some(return_value)) = (dst, return_value) {
                        frame.write(*dst, return_value)?;
                    }
                }
                InstructionKind::Return { src } => return Ok(*frame.read(*src)?),
            }

            if let Some(heap) = heap.as_deref_mut() {
                heap.collect_frame_at_safe_point(&frame, budget.as_deref_mut());
            }
        }

        Err(VmError::new(VmErrorKind::MissingReturn))
    }
}
