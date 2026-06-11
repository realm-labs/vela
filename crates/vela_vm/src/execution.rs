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
        if let Some(inline_caches) = call.inline_caches {
            debug_assert!(inline_caches.len() >= code.cache_sites.len());
        }
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
                UnlinkedInstructionKind::LoadConst { dst, constant } => {
                    let constant_value = code.constants.get(constant.0).ok_or_else(|| {
                        VmError::new(VmErrorKind::ConstantOutOfBounds {
                            constant: constant.0,
                        })
                        .with_source_span(instruction.span)
                    })?;
                    let value = match constant_value {
                        Constant::Null => Value::Null,
                        Constant::Bool(value) => Value::Bool(*value),
                        Constant::Scalar(value) => Value::Scalar(*value),
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
                UnlinkedInstructionKind::Move { dst, src } => {
                    let value = *frame.read(*src)?;
                    frame.write(*dst, value)?;
                }
                UnlinkedInstructionKind::Not { dst, src } => {
                    let value = Value::Bool(!is_truthy(frame.read(*src)?));
                    frame.write(*dst, value)?;
                }
                UnlinkedInstructionKind::Truthy { dst, src } => {
                    let value = Value::Bool(is_truthy(frame.read(*src)?));
                    frame.write(*dst, value)?;
                }
                UnlinkedInstructionKind::Negate { dst, src } => {
                    let value = negate_numeric(frame.read(*src)?)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write(*dst, value)?;
                }
                UnlinkedInstructionKind::Add { dst, lhs, rhs } => {
                    let value = add_numeric(frame.read(*lhs)?, frame.read(*rhs)?)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write(*dst, value)?;
                }
                UnlinkedInstructionKind::Sub { dst, lhs, rhs } => {
                    let value = sub_numeric(frame.read(*lhs)?, frame.read(*rhs)?)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write(*dst, value)?;
                }
                UnlinkedInstructionKind::Mul { dst, lhs, rhs } => {
                    let value = mul_numeric(frame.read(*lhs)?, frame.read(*rhs)?)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write(*dst, value)?;
                }
                UnlinkedInstructionKind::Div { dst, lhs, rhs } => {
                    let value = div_numeric(frame.read(*lhs)?, frame.read(*rhs)?)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write(*dst, value)?;
                }
                UnlinkedInstructionKind::Rem { dst, lhs, rhs } => {
                    let value = rem_numeric(frame.read(*lhs)?, frame.read(*rhs)?)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write(*dst, value)?;
                }
                UnlinkedInstructionKind::Equal { dst, lhs, rhs } => {
                    let value = Value::Bool(values_equal(
                        frame.read(*lhs)?,
                        frame.read(*rhs)?,
                        heap.as_deref(),
                    )?);
                    frame.write(*dst, value)?;
                }
                UnlinkedInstructionKind::NotEqual { dst, lhs, rhs } => {
                    let value = Value::Bool(!values_equal(
                        frame.read(*lhs)?,
                        frame.read(*rhs)?,
                        heap.as_deref(),
                    )?);
                    frame.write(*dst, value)?;
                }
                UnlinkedInstructionKind::Less { dst, lhs, rhs } => {
                    let value = less_numeric(frame.read(*lhs)?, frame.read(*rhs)?)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write(*dst, Value::Bool(value))?;
                }
                UnlinkedInstructionKind::LessEqual { dst, lhs, rhs } => {
                    let value = less_equal_numeric(frame.read(*lhs)?, frame.read(*rhs)?)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write(*dst, Value::Bool(value))?;
                }
                UnlinkedInstructionKind::Greater { dst, lhs, rhs } => {
                    let value = greater_numeric(frame.read(*lhs)?, frame.read(*rhs)?)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write(*dst, Value::Bool(value))?;
                }
                UnlinkedInstructionKind::GreaterEqual { dst, lhs, rhs } => {
                    let value = greater_equal_numeric(frame.read(*lhs)?, frame.read(*rhs)?)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    frame.write(*dst, Value::Bool(value))?;
                }
                UnlinkedInstructionKind::JumpIfFalse { condition, target } => {
                    if !is_truthy(frame.read(*condition)?) {
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
                UnlinkedInstructionKind::CallMethod {
                    dst,
                    receiver,
                    method,
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
                                values: values.as_slice(),
                            },
                        )?;
                    }
                }
                UnlinkedInstructionKind::CallMethodId {
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
                UnlinkedInstructionKind::TryPropagate { dst, src } => {
                    match try_propagate_value(frame.read(*src)?, heap.as_deref())? {
                        TryPropagation::Continue(value) => frame.write(*dst, value)?,
                        TryPropagation::Return(value) => return Ok(value),
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
                    let value = indexing::get_index(
                        frame.read(*base)?,
                        frame.read(*index)?,
                        heap.as_deref(),
                    )?;
                    frame.write(*dst, value)?;
                }
                UnlinkedInstructionKind::SetIndex { base, index, src } => {
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
                UnlinkedInstructionKind::EnumTagEqual {
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
                UnlinkedInstructionKind::LoadGlobal {
                    dst,
                    global,
                    slot,
                    cache_site,
                } => {
                    let cached_slot = cache_site
                        .and_then(|site| {
                            call.inline_caches
                                .and_then(|caches| caches.global_read_slot(site))
                        })
                        .or(*slot);
                    let value = host_access::load_host_global(
                        host_access::HostAccessRuntime {
                            frame: &frame,
                            heap: heap.as_deref_mut(),
                            budget: budget.as_deref_mut(),
                            host: host.as_deref_mut(),
                            inline_caches: call.inline_caches,
                            source_span: instruction.span,
                        },
                        global,
                        cached_slot,
                    )?;
                    if let (Some(caches), Some(cache_site), Some(slot)) =
                        (call.inline_caches, *cache_site, *slot)
                        && caches.global_read_slot(cache_site).is_none()
                    {
                        caches.set_global_read_slot(cache_site, slot);
                    }
                    frame.write(*dst, value)?;
                }
                UnlinkedInstructionKind::HostRead {
                    dst,
                    root,
                    target,
                    dynamic_args,
                    cache_site,
                } => {
                    let plan = host_access::code_host_target(
                        &code.host_targets,
                        *target,
                        instruction.span,
                    )?;
                    let value = host_access::execute_host_read(
                        host_access::HostAccessRuntime {
                            frame: &frame,
                            heap: heap.as_deref_mut(),
                            budget: budget.as_deref_mut(),
                            host: host.as_deref_mut(),
                            inline_caches: call.inline_caches,
                            source_span: instruction.span,
                        },
                        *root,
                        *target,
                        plan,
                        dynamic_args,
                        *cache_site,
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
                    let plan = host_access::code_host_target(
                        &code.host_targets,
                        *target,
                        instruction.span,
                    )?;
                    host_access::execute_host_write(
                        host_access::HostAccessRuntime {
                            frame: &frame,
                            heap: heap.as_deref_mut(),
                            budget: budget.as_deref_mut(),
                            host: host.as_deref_mut(),
                            inline_caches: call.inline_caches,
                            source_span: instruction.span,
                        },
                        *root,
                        *target,
                        plan,
                        dynamic_args,
                        *src,
                        *cache_site,
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
                    let plan = host_access::code_host_target(
                        &code.host_targets,
                        *target,
                        instruction.span,
                    )?;
                    host_access::execute_host_mutate(
                        host_access::HostAccessRuntime {
                            frame: &frame,
                            heap: heap.as_deref_mut(),
                            budget: budget.as_deref_mut(),
                            host: host.as_deref_mut(),
                            inline_caches: call.inline_caches,
                            source_span: instruction.span,
                        },
                        *root,
                        host_access::HostMutationPlan {
                            target_id: *target,
                            target: plan,
                            dynamic_args,
                            op: *op,
                            rhs: *rhs,
                            cache_site: *cache_site,
                        },
                    )?;
                }
                UnlinkedInstructionKind::HostRemove {
                    root,
                    target,
                    dynamic_args,
                    cache_site,
                } => {
                    let plan = host_access::code_host_target(
                        &code.host_targets,
                        *target,
                        instruction.span,
                    )?;
                    host_access::execute_host_remove(
                        host_access::HostAccessRuntime {
                            frame: &frame,
                            heap: heap.as_deref_mut(),
                            budget: budget.as_deref_mut(),
                            host: host.as_deref_mut(),
                            inline_caches: call.inline_caches,
                            source_span: instruction.span,
                        },
                        *root,
                        *target,
                        plan,
                        dynamic_args,
                        *cache_site,
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
                    let plan = host_access::code_host_target(
                        &code.host_targets,
                        *target,
                        instruction.span,
                    )?;
                    let return_value = host_access::execute_host_call(
                        host_access::HostAccessRuntime {
                            frame: &frame,
                            heap: heap.as_deref_mut(),
                            budget: budget.as_deref_mut(),
                            host: host.as_deref_mut(),
                            inline_caches: call.inline_caches,
                            source_span: instruction.span,
                        },
                        *root,
                        host_access::HostCallPlan {
                            target_id: *target,
                            target: plan,
                            dynamic_args,
                            method: *method,
                            args,
                            wants_return: dst.is_some(),
                            cache_site: *cache_site,
                        },
                    )?;
                    if let (Some(dst), Some(return_value)) = (dst, return_value) {
                        frame.write(*dst, return_value)?;
                    }
                }
                UnlinkedInstructionKind::Return { src } => return Ok(*frame.read(*src)?),
            }

            if let Some(heap) = heap.as_deref_mut() {
                heap.collect_frame_at_safe_point(&frame, budget.as_deref_mut());
            }
        }

        Err(VmError::new(VmErrorKind::MissingReturn))
    }
}
