use super::*;
use vela_common::{HostMethodId, MethodId};

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
                            if let Some(value) = loaded_string_constant(
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
                    enum NativeCallTarget<'a> {
                        Pure(&'a NativeFunction),
                        Host(&'a HostNativeFunction),
                    }

                    let values = native_call_args_from_registers(&frame, args, heap.as_deref())?;
                    let target = native
                        .and_then(|id| {
                            self.native_ids
                                .get(&id)
                                .map(NativeCallTarget::Pure)
                                .or_else(|| {
                                    self.host_native_ids.get(&id).map(NativeCallTarget::Host)
                                })
                        })
                        .or_else(|| self.natives.get(name).map(NativeCallTarget::Pure))
                        .or_else(|| self.host_natives.get(name).map(NativeCallTarget::Host));
                    let result = match target {
                        Some(NativeCallTarget::Pure(native)) => native(values.as_slice())
                            .map_err(|error| error.with_source_span_if_absent(instruction.span))?,
                        Some(NativeCallTarget::Host(native)) => {
                            let host = host.as_deref_mut().ok_or_else(|| {
                                VmError::new(VmErrorKind::TypeMismatch {
                                    operation: "host context",
                                })
                            })?;
                            let tx_checkpoint = host.tx.clone();
                            let result =
                                match native(values.as_slice(), host, budget.as_deref_mut()) {
                                    Ok(result) => result,
                                    Err(error) => {
                                        *host.tx = tx_checkpoint;
                                        return Err(
                                            error.with_source_span_if_absent(instruction.span)
                                        );
                                    }
                                };
                            if let Some(budget) = budget.as_deref()
                                && let Err(error) =
                                    budget.check_patch_count(host.tx.patches().len())
                            {
                                *host.tx = tx_checkpoint;
                                return Err(error.with_source_span_if_absent(instruction.span));
                            }
                            result
                        }
                        None => {
                            return Err(VmError::new(VmErrorKind::UnknownNative {
                                name: name.clone(),
                            })
                            .with_source_span_if_absent(instruction.span));
                        }
                    };
                    if let (Some(budget), Some(host)) = (budget.as_deref(), host.as_deref()) {
                        budget.check_patch_count(host.tx.patches().len())?;
                    }
                    if let Some(dst) = dst {
                        let result = owned_to_value(
                            result,
                            heap.as_deref_mut().ok_or_else(|| {
                                VmError::new(VmErrorKind::TypeMismatch {
                                    operation: "native heap",
                                })
                            })?,
                            budget.as_deref_mut(),
                        )?;
                        frame.write(*dst, result)?;
                    }
                }
                InstructionKind::CallFunction { dst, name, args } => {
                    let program = program.ok_or_else(|| {
                        VmError::new(VmErrorKind::UnknownFunction { name: name.clone() })
                    })?;
                    let function = program.function(name).ok_or_else(|| {
                        VmError::new(VmErrorKind::UnknownFunction { name: name.clone() })
                    })?;
                    let values = script_call_args_from_call_arguments(&frame, args)?;
                    let protected_root_len = heap
                        .as_deref_mut()
                        .map(|heap| heap.push_frame_roots(&frame));
                    let result = self.execute_call(
                        ExecutionCall {
                            code: function,
                            program: Some(program),
                            captures: &[],
                            args: values.as_slice(),
                            call_site: instruction.span,
                            call_site_offset: Some(instruction_offset),
                        },
                        host.as_deref_mut(),
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                    );
                    if let (Some(heap), Some(protected_root_len)) =
                        (heap.as_deref_mut(), protected_root_len)
                    {
                        heap.truncate_protected_roots(protected_root_len);
                    }
                    let result = store_value_in_heap_if_needed(
                        result?,
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                    )?;
                    frame.write(*dst, result)?;
                }
                InstructionKind::MakeClosure {
                    dst,
                    code,
                    captures,
                } => {
                    let captures = captures
                        .iter()
                        .map(|register| frame.read(*register).cloned())
                        .collect::<VmResult<Vec<_>>>()?;
                    let heap = heap.as_deref_mut().ok_or_else(|| {
                        VmError::new(VmErrorKind::TypeMismatch {
                            operation: "closure heap",
                        })
                    })?;
                    let value = allocate_heap_value(
                        HeapValue::Closure(ClosureValue {
                            code: Arc::new((**code).clone()),
                            captures,
                        }),
                        heap,
                        budget.as_deref_mut(),
                    )?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::CallClosure { dst, callee, args } => {
                    let closure =
                        expect_closure(frame.read(*callee)?, heap.as_deref(), "closure call")?;
                    let values = script_call_args_from_registers(&frame, args)?;
                    let protected_root_len = heap
                        .as_deref_mut()
                        .map(|heap| heap.push_frame_roots(&frame));
                    let result = self.execute_call(
                        ExecutionCall {
                            code: &closure.code,
                            program,
                            captures: &closure.captures,
                            args: values.as_slice(),
                            call_site: instruction.span,
                            call_site_offset: Some(instruction_offset),
                        },
                        host.as_deref_mut(),
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                    );
                    if let (Some(heap), Some(protected_root_len)) =
                        (heap.as_deref_mut(), protected_root_len)
                    {
                        heap.truncate_protected_roots(protected_root_len);
                    }
                    let result = store_value_in_heap_if_needed(
                        result?,
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                    )?;
                    frame.write(*dst, result)?;
                }
                InstructionKind::CallMethod {
                    dst,
                    receiver,
                    method,
                    value_method_id,
                    args,
                } => {
                    if args.is_empty() {
                        dispatch_call_method(
                            self,
                            program,
                            &mut host,
                            &mut heap,
                            &mut budget,
                            &mut frame,
                            MethodCall {
                                dst: *dst,
                                receiver: *receiver,
                                method,
                                value_method_id: *value_method_id,
                                values: &[],
                            },
                        )?;
                    } else {
                        let values = script_call_args_from_call_arguments(&frame, args)?;
                        dispatch_call_method(
                            self,
                            program,
                            &mut host,
                            &mut heap,
                            &mut budget,
                            &mut frame,
                            MethodCall {
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
                        dispatch_call_method_id(
                            self,
                            program,
                            &mut host,
                            &mut heap,
                            &mut budget,
                            &mut frame,
                            MethodIdCall {
                                dst: *dst,
                                receiver: *receiver,
                                method,
                                method_id: *method_id,
                                values: &[],
                            },
                        )?;
                    } else {
                        let values = script_call_args_from_call_arguments(&frame, args)?;
                        dispatch_call_method_id(
                            self,
                            program,
                            &mut host,
                            &mut heap,
                            &mut budget,
                            &mut frame,
                            MethodIdCall {
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
                    let Some(heap) = heap.as_deref_mut() else {
                        return Err(VmError::new(VmErrorKind::TypeMismatch {
                            operation: "array heap",
                        }));
                    };
                    let slots = runtime_values_from_registers(
                        &frame,
                        elements,
                        heap,
                        budget.as_deref_mut(),
                    )?;
                    let value =
                        allocate_heap_value(HeapValue::Array(slots), heap, budget.as_deref_mut())?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::MakeMap { dst, entries } => {
                    let Some(heap) = heap.as_deref_mut() else {
                        return Err(VmError::new(VmErrorKind::TypeMismatch {
                            operation: "map heap",
                        }));
                    };
                    let slots =
                        runtime_map_from_registers(&frame, entries, heap, budget.as_deref_mut())?;
                    let value =
                        allocate_heap_value(HeapValue::Map(slots), heap, budget.as_deref_mut())?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::MakeRange {
                    dst,
                    start,
                    end,
                    inclusive,
                } => {
                    let start = expect_int(frame.read(*start)?, "range")?;
                    let end = expect_int(frame.read(*end)?, "range")?;
                    frame.write(*dst, Value::Range(RangeValue::new(start, end, *inclusive)))?;
                }
                InstructionKind::MakeRecord {
                    dst,
                    type_name,
                    fields,
                } => {
                    let Some(heap) = heap.as_deref_mut() else {
                        return Err(VmError::new(VmErrorKind::TypeMismatch {
                            operation: "record heap",
                        }));
                    };
                    let slots = runtime_fields_from_registers(
                        type_name,
                        &frame,
                        fields,
                        heap,
                        budget.as_deref_mut(),
                    )?;
                    let value = allocate_heap_value(
                        HeapValue::Record {
                            type_name: type_name.clone(),
                            fields: slots,
                        },
                        heap,
                        budget.as_deref_mut(),
                    )?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::MakeEnum {
                    dst,
                    enum_name,
                    variant,
                    fields,
                } => {
                    let owner = enum_variant_owner(enum_name, variant);
                    let Some(heap) = heap.as_deref_mut() else {
                        return Err(VmError::new(VmErrorKind::TypeMismatch {
                            operation: "enum heap",
                        }));
                    };
                    let slots = runtime_fields_from_registers(
                        &owner,
                        &frame,
                        fields,
                        heap,
                        budget.as_deref_mut(),
                    )?;
                    let value = allocate_heap_value(
                        HeapValue::Enum {
                            enum_name: enum_name.clone(),
                            variant: variant.clone(),
                            fields: slots,
                        },
                        heap,
                        budget.as_deref_mut(),
                    )?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::GetRecordField { dst, record, field } => {
                    let value =
                        get_record_field_value(frame.read(*record)?, field, heap.as_deref())?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::GetRecordSlot {
                    dst,
                    record,
                    field,
                    slot,
                } => {
                    let value =
                        get_record_slot_value(frame.read(*record)?, field, *slot, heap.as_deref())?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::SetRecordField { record, field, src } => {
                    let mut record_value = *frame.read(*record)?;
                    record_fields::set_record_field_value(
                        &mut record_value,
                        field,
                        frame.read(*src)?,
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                    )?;
                    frame.write(*record, record_value)?;
                }
                InstructionKind::SetRecordSlot {
                    record,
                    field,
                    slot,
                    src,
                } => {
                    let mut record_value = *frame.read(*record)?;
                    record_fields::set_record_slot_value(
                        &mut record_value,
                        field,
                        *slot,
                        frame.read(*src)?,
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                    )?;
                    frame.write(*record, record_value)?;
                }
                InstructionKind::GetEnumField { dst, value, field } => {
                    let value = get_enum_field_value(frame.read(*value)?, field, heap.as_deref())?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::GetEnumSlot {
                    dst,
                    value,
                    field,
                    slot,
                } => {
                    let value =
                        get_enum_slot_value(frame.read(*value)?, field, *slot, heap.as_deref())?;
                    frame.write(*dst, value)?;
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
                    let iterator =
                        iteration::make_iterator(frame.read(*iterable)?, heap.as_deref())?;
                    let Some(heap) = heap.as_deref_mut() else {
                        return Err(VmError::new(VmErrorKind::TypeMismatch {
                            operation: "iterator heap",
                        }));
                    };
                    let value = allocate_heap_value(
                        HeapValue::Iterator(iterator),
                        heap,
                        budget.as_deref_mut(),
                    )?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::IterNext {
                    iterator,
                    dst,
                    jump_if_done,
                } => {
                    let value = *frame.read(*iterator)?;
                    let next = match value {
                        Value::HeapRef(reference) => {
                            let Some(HeapValue::Iterator(iterator_state)) = heap
                                .as_deref_mut()
                                .and_then(|heap| heap.heap.get_mut(reference).ok())
                            else {
                                return Err(VmError::new(VmErrorKind::TypeMismatch {
                                    operation: "iterator",
                                }));
                            };
                            iterator_state.next()
                        }
                        _ => {
                            return Err(VmError::new(VmErrorKind::TypeMismatch {
                                operation: "iterator",
                            }));
                        }
                    };
                    match next {
                        Some(value) => {
                            frame.write(*dst, value)?;
                        }
                        None => {
                            validate_jump(code, jump_if_done.0)?;
                            ip = jump_if_done.0;
                        }
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
                    let is_done = match frame.read(*done)? {
                        Value::Bool(value) => *value,
                        _ => {
                            return Err(VmError::new(VmErrorKind::TypeMismatch {
                                operation: "range",
                            }));
                        }
                    };
                    if is_done {
                        validate_jump(code, jump_if_done.0)?;
                        ip = jump_if_done.0;
                    } else {
                        let current = expect_int(frame.read(*cursor)?, "range")?;
                        let end = expect_int(frame.read(*end)?, "range")?;
                        let has_next = if *inclusive {
                            current <= end
                        } else {
                            current < end
                        };
                        if has_next {
                            frame.write(*dst, Value::Int(current))?;
                            if current == i64::MAX {
                                frame.write(*done, Value::Bool(true))?;
                            } else {
                                frame.write(*cursor, Value::Int(current + 1))?;
                            }
                        } else {
                            frame.write(*done, Value::Bool(true))?;
                            validate_jump(code, jump_if_done.0)?;
                            ip = jump_if_done.0;
                        }
                    }
                }
                InstructionKind::EnumTagEqual {
                    dst,
                    value,
                    enum_name,
                    variant,
                } => {
                    let matches =
                        enum_tag_equal(frame.read(*value)?, enum_name, variant, heap.as_deref());
                    frame.write(*dst, Value::Bool(matches))?;
                }
                InstructionKind::GetHostField { dst, root, field } => {
                    let root = expect_host_ref(frame.read(*root)?, "get_host_field")?;
                    let path = HostPath::new(root).field(*field);
                    let host = host.as_deref_mut().ok_or_else(|| {
                        VmError::new(VmErrorKind::TypeMismatch {
                            operation: "host context",
                        })
                    })?;
                    let value = host
                        .tx
                        .read_path_at(host.adapter, &path, instruction.span)?;
                    let value =
                        runtime_value_from_host(value, heap.as_deref_mut(), budget.as_deref_mut())?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::GetHostPath {
                    dst,
                    root,
                    segments,
                } => {
                    let root = expect_host_ref(frame.read(*root)?, "get_host_path")?;
                    let mut symbols = self.host_path_symbols.borrow_mut();
                    let path = host_path_from_segments(
                        root,
                        segments,
                        &frame,
                        heap.as_deref(),
                        &mut symbols,
                    )?;
                    let host = host.as_deref_mut().ok_or_else(|| {
                        VmError::new(VmErrorKind::TypeMismatch {
                            operation: "host context",
                        })
                    })?;
                    let value = host
                        .tx
                        .read_path_at(host.adapter, &path, instruction.span)?;
                    let value =
                        runtime_value_from_host(value, heap.as_deref_mut(), budget.as_deref_mut())?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::SetHostField { root, field, src } => {
                    let root = expect_host_ref(frame.read(*root)?, "set_host_field")?;
                    let value =
                        value_to_host(frame.read(*src)?, "set_host_field", heap.as_deref())?;
                    let path = HostPath::new(root).field(*field);
                    let host = host.as_deref_mut().ok_or_else(|| {
                        VmError::new(VmErrorKind::TypeMismatch {
                            operation: "host context",
                        })
                    })?;
                    if let Some(budget) = budget.as_deref() {
                        budget.reserve_patch(host.tx.patches().len())?;
                    }
                    host.tx.set_path(path, value, instruction.span)?;
                }
                InstructionKind::SetHostPath {
                    root,
                    segments,
                    src,
                } => {
                    let root = expect_host_ref(frame.read(*root)?, "set_host_path")?;
                    let value = value_to_host(frame.read(*src)?, "set_host_path", heap.as_deref())?;
                    let mut symbols = self.host_path_symbols.borrow_mut();
                    let path = host_path_from_segments(
                        root,
                        segments,
                        &frame,
                        heap.as_deref(),
                        &mut symbols,
                    )?;
                    let host = host.as_deref_mut().ok_or_else(|| {
                        VmError::new(VmErrorKind::TypeMismatch {
                            operation: "host context",
                        })
                    })?;
                    if let Some(budget) = budget.as_deref() {
                        budget.reserve_patch(host.tx.patches().len())?;
                    }
                    host.tx.set_path(path, value, instruction.span)?;
                }
                InstructionKind::AddHostField { root, field, rhs } => {
                    host_patches::apply_host_field_numeric_patch(
                        host_patches::HostPatchRuntime {
                            frame: &frame,
                            heap: heap.as_deref(),
                            budget: budget.as_deref(),
                            host: host.as_deref_mut(),
                            source_span: instruction.span,
                        },
                        *root,
                        *field,
                        *rhs,
                        host_patches::HostNumericPatch::Add,
                    )?;
                }
                InstructionKind::SubHostField { root, field, rhs } => {
                    host_patches::apply_host_field_numeric_patch(
                        host_patches::HostPatchRuntime {
                            frame: &frame,
                            heap: heap.as_deref(),
                            budget: budget.as_deref(),
                            host: host.as_deref_mut(),
                            source_span: instruction.span,
                        },
                        *root,
                        *field,
                        *rhs,
                        host_patches::HostNumericPatch::Sub,
                    )?;
                }
                InstructionKind::MulHostField { root, field, rhs } => {
                    host_patches::apply_host_field_numeric_patch(
                        host_patches::HostPatchRuntime {
                            frame: &frame,
                            heap: heap.as_deref(),
                            budget: budget.as_deref(),
                            host: host.as_deref_mut(),
                            source_span: instruction.span,
                        },
                        *root,
                        *field,
                        *rhs,
                        host_patches::HostNumericPatch::Mul,
                    )?;
                }
                InstructionKind::DivHostField { root, field, rhs } => {
                    host_patches::apply_host_field_numeric_patch(
                        host_patches::HostPatchRuntime {
                            frame: &frame,
                            heap: heap.as_deref(),
                            budget: budget.as_deref(),
                            host: host.as_deref_mut(),
                            source_span: instruction.span,
                        },
                        *root,
                        *field,
                        *rhs,
                        host_patches::HostNumericPatch::Div,
                    )?;
                }
                InstructionKind::RemHostField { root, field, rhs } => {
                    host_patches::apply_host_field_numeric_patch(
                        host_patches::HostPatchRuntime {
                            frame: &frame,
                            heap: heap.as_deref(),
                            budget: budget.as_deref(),
                            host: host.as_deref_mut(),
                            source_span: instruction.span,
                        },
                        *root,
                        *field,
                        *rhs,
                        host_patches::HostNumericPatch::Rem,
                    )?;
                }
                InstructionKind::AddHostPath {
                    root,
                    segments,
                    rhs,
                } => {
                    let mut symbols = self.host_path_symbols.borrow_mut();
                    host_patches::apply_host_path_numeric_patch(
                        host_patches::HostPatchRuntime {
                            frame: &frame,
                            heap: heap.as_deref(),
                            budget: budget.as_deref(),
                            host: host.as_deref_mut(),
                            source_span: instruction.span,
                        },
                        *root,
                        segments,
                        *rhs,
                        host_patches::HostNumericPatch::Add,
                        &mut symbols,
                    )?;
                }
                InstructionKind::SubHostPath {
                    root,
                    segments,
                    rhs,
                } => {
                    let mut symbols = self.host_path_symbols.borrow_mut();
                    host_patches::apply_host_path_numeric_patch(
                        host_patches::HostPatchRuntime {
                            frame: &frame,
                            heap: heap.as_deref(),
                            budget: budget.as_deref(),
                            host: host.as_deref_mut(),
                            source_span: instruction.span,
                        },
                        *root,
                        segments,
                        *rhs,
                        host_patches::HostNumericPatch::Sub,
                        &mut symbols,
                    )?;
                }
                InstructionKind::MulHostPath {
                    root,
                    segments,
                    rhs,
                } => {
                    let mut symbols = self.host_path_symbols.borrow_mut();
                    host_patches::apply_host_path_numeric_patch(
                        host_patches::HostPatchRuntime {
                            frame: &frame,
                            heap: heap.as_deref(),
                            budget: budget.as_deref(),
                            host: host.as_deref_mut(),
                            source_span: instruction.span,
                        },
                        *root,
                        segments,
                        *rhs,
                        host_patches::HostNumericPatch::Mul,
                        &mut symbols,
                    )?;
                }
                InstructionKind::DivHostPath {
                    root,
                    segments,
                    rhs,
                } => {
                    let mut symbols = self.host_path_symbols.borrow_mut();
                    host_patches::apply_host_path_numeric_patch(
                        host_patches::HostPatchRuntime {
                            frame: &frame,
                            heap: heap.as_deref(),
                            budget: budget.as_deref(),
                            host: host.as_deref_mut(),
                            source_span: instruction.span,
                        },
                        *root,
                        segments,
                        *rhs,
                        host_patches::HostNumericPatch::Div,
                        &mut symbols,
                    )?;
                }
                InstructionKind::RemHostPath {
                    root,
                    segments,
                    rhs,
                } => {
                    let mut symbols = self.host_path_symbols.borrow_mut();
                    host_patches::apply_host_path_numeric_patch(
                        host_patches::HostPatchRuntime {
                            frame: &frame,
                            heap: heap.as_deref(),
                            budget: budget.as_deref(),
                            host: host.as_deref_mut(),
                            source_span: instruction.span,
                        },
                        *root,
                        segments,
                        *rhs,
                        host_patches::HostNumericPatch::Rem,
                        &mut symbols,
                    )?;
                }
                InstructionKind::PushHostPath {
                    root,
                    segments,
                    value,
                } => {
                    let root = expect_host_ref(frame.read(*root)?, "push_host_path")?;
                    let value =
                        value_to_host(frame.read(*value)?, "push_host_path", heap.as_deref())?;
                    let mut symbols = self.host_path_symbols.borrow_mut();
                    let path = host_path_from_segments(
                        root,
                        segments,
                        &frame,
                        heap.as_deref(),
                        &mut symbols,
                    )?;
                    let host = host.as_deref_mut().ok_or_else(|| {
                        VmError::new(VmErrorKind::TypeMismatch {
                            operation: "host context",
                        })
                    })?;
                    let base_value = host
                        .tx
                        .read_path_at(host.adapter, &path, instruction.span)?;
                    if let Some(budget) = budget.as_deref() {
                        budget.reserve_patch(host.tx.patches().len())?;
                    }
                    host.tx
                        .push_path(path, value, base_value, instruction.span)?;
                }
                InstructionKind::RemoveHostPath { root, segments } => {
                    let root = expect_host_ref(frame.read(*root)?, "remove_host_path")?;
                    let mut symbols = self.host_path_symbols.borrow_mut();
                    let path = host_path_from_segments(
                        root,
                        segments,
                        &frame,
                        heap.as_deref(),
                        &mut symbols,
                    )?;
                    let host = host.as_deref_mut().ok_or_else(|| {
                        VmError::new(VmErrorKind::TypeMismatch {
                            operation: "host context",
                        })
                    })?;
                    if let Some(budget) = budget.as_deref() {
                        budget.reserve_patch(host.tx.patches().len())?;
                    }
                    host.tx.remove_path(path, instruction.span)?;
                }
                InstructionKind::CallHostMethod {
                    dst,
                    root,
                    segments,
                    method,
                    args,
                } => {
                    let root = expect_host_ref(frame.read(*root)?, "call_host_method")?;
                    let mut symbols = self.host_path_symbols.borrow_mut();
                    let path = host_path_from_segments(
                        root,
                        segments,
                        &frame,
                        heap.as_deref(),
                        &mut symbols,
                    )?;
                    let values = args
                        .iter()
                        .map(|register| {
                            value_to_host(
                                frame.read(*register)?,
                                "call_host_method",
                                heap.as_deref(),
                            )
                        })
                        .collect::<VmResult<Vec<_>>>()?;
                    let host = host.as_deref_mut().ok_or_else(|| {
                        VmError::new(VmErrorKind::TypeMismatch {
                            operation: "host context",
                        })
                    })?;
                    if let Some(budget) = budget.as_deref() {
                        budget.reserve_patch(host.tx.patches().len())?;
                    }
                    let return_value = host
                        .adapter
                        .preview_method_return(&path, *method, &values)
                        .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    host.tx
                        .call_method(path, *method, values, instruction.span)?;
                    if let Some(dst) = dst {
                        let return_value = runtime_value_from_host(
                            return_value,
                            heap.as_deref_mut(),
                            budget.as_deref_mut(),
                        )?;
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

fn caller_roots_for_heap(frame: &CallFrame, heap: Option<&HeapExecution<'_>>) -> Vec<GcRef> {
    if heap.is_some() {
        frame.heap_roots()
    } else {
        Vec::new()
    }
}

fn runtime_value_from_host(
    value: vela_host::value::HostValue,
    heap: Option<&mut HeapExecution<'_>>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    if let Some(heap) = heap {
        host_to_value(value, heap, budget)
    } else {
        Ok(value_from_host(value))
    }
}

struct MethodCall<'a> {
    dst: Register,
    receiver: Register,
    method: &'a str,
    value_method_id: Option<HostMethodId>,
    values: &'a [Value],
}

fn dispatch_call_method(
    vm: &Vm,
    program: Option<&Program>,
    host: &mut Option<&mut HostExecution<'_>>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    frame: &mut CallFrame,
    call: MethodCall<'_>,
) -> VmResult<()> {
    if let Some(result) = call_readonly_method_without_callbacks(
        frame.read(call.receiver)?,
        call.method,
        call.value_method_id,
        call.values,
        heap.as_deref(),
    ) {
        let result =
            store_value_in_heap_if_needed(result?, heap.as_deref_mut(), budget.as_deref_mut())?;
        frame.write(call.dst, result)?;
        return Ok(());
    }

    let caller_roots = caller_roots_for_heap(frame, heap.as_deref());
    if let Some(result) = call_non_mutating_method(
        frame.read(call.receiver)?,
        call.method,
        call.value_method_id,
        call.values,
        ScriptMethodDispatch {
            vm,
            program,
            host: host.as_deref_mut(),
            heap: heap.as_deref_mut(),
            budget: budget.as_deref_mut(),
            caller_roots,
        },
    ) {
        let result =
            store_value_in_heap_if_needed(result?, heap.as_deref_mut(), budget.as_deref_mut())?;
        frame.write(call.dst, result)?;
    } else {
        let mut receiver_value = *frame.read(call.receiver)?;
        let caller_roots = caller_roots_for_heap(frame, heap.as_deref());
        let result = call_method(
            &mut receiver_value,
            call.method,
            call.value_method_id,
            call.values,
            ScriptMethodDispatch {
                vm,
                program,
                host: host.as_deref_mut(),
                heap: heap.as_deref_mut(),
                budget: budget.as_deref_mut(),
                caller_roots,
            },
        )?;
        let result =
            store_value_in_heap_if_needed(result, heap.as_deref_mut(), budget.as_deref_mut())?;
        frame.write(call.receiver, receiver_value)?;
        frame.write(call.dst, result)?;
    }
    Ok(())
}

struct MethodIdCall<'a> {
    dst: Register,
    receiver: Register,
    method: &'a str,
    method_id: MethodId,
    values: &'a [Value],
}

fn dispatch_call_method_id(
    vm: &Vm,
    program: Option<&Program>,
    host: &mut Option<&mut HostExecution<'_>>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    frame: &mut CallFrame,
    call: MethodIdCall<'_>,
) -> VmResult<()> {
    let receiver_value = *frame.read(call.receiver)?;
    let caller_roots = caller_roots_for_heap(frame, heap.as_deref());
    let result = call_method_id(
        &receiver_value,
        call.method,
        call.method_id,
        call.values,
        ScriptMethodDispatch {
            vm,
            program,
            host: host.as_deref_mut(),
            heap: heap.as_deref_mut(),
            budget: budget.as_deref_mut(),
            caller_roots,
        },
    )?;
    let result = store_value_in_heap_if_needed(result, heap.as_deref_mut(), budget.as_deref_mut())?;
    frame.write(call.dst, result)
}

#[inline]
fn native_call_args_from_registers(
    frame: &CallFrame,
    registers: &[Register],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<SmallStorage<OwnedValue>> {
    SmallStorage::try_from_slice_map(registers, 4, |register| {
        value_to_owned(frame.read(*register)?, heap)
    })
}

#[inline]
fn script_call_args_from_call_arguments(
    frame: &CallFrame,
    args: &[CallArgument],
) -> VmResult<SmallStorage<Value>> {
    SmallStorage::try_from_slice_map(args, 4, |arg| match arg {
        CallArgument::Register(register) => Ok(*frame.read(*register)?),
        CallArgument::Missing => Ok(Value::Missing),
    })
}

#[inline]
fn script_call_args_from_registers(
    frame: &CallFrame,
    registers: &[Register],
) -> VmResult<SmallStorage<Value>> {
    SmallStorage::try_from_slice_map(registers, 4, |register| Ok(*frame.read(*register)?))
}

#[inline]
fn runtime_values_from_registers(
    frame: &CallFrame,
    registers: &[Register],
    heap: &mut HeapExecution<'_>,
    mut budget: Option<&mut ExecutionBudget>,
) -> VmResult<Vec<Value>> {
    SmallStorage::try_from_slice_map(registers, 8, |register| {
        runtime_value_from_register(frame, *register, heap, budget.as_deref_mut())
    })
    .map(SmallStorage::into_vec)
}

#[inline]
fn runtime_value_from_register(
    frame: &CallFrame,
    register: Register,
    heap: &mut HeapExecution<'_>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    store_runtime_value(frame.read(register)?, heap, budget)
}

fn runtime_map_from_registers(
    frame: &CallFrame,
    entries: &[(String, Register)],
    heap: &mut HeapExecution<'_>,
    mut budget: Option<&mut ExecutionBudget>,
) -> VmResult<BTreeMap<String, Value>> {
    entries
        .iter()
        .map(|(key, register)| {
            Ok((
                key.clone(),
                store_runtime_value(frame.read(*register)?, heap, budget.as_deref_mut())?,
            ))
        })
        .collect()
}

fn runtime_fields_from_registers(
    owner: &str,
    frame: &CallFrame,
    fields: &[(String, Register)],
    heap: &mut HeapExecution<'_>,
    mut budget: Option<&mut ExecutionBudget>,
) -> VmResult<ScriptFields<Value>> {
    match fields {
        [] => Ok(ScriptFields::empty(owner)),
        [(name, register)] => {
            let value = store_runtime_value(frame.read(*register)?, heap, budget.as_deref_mut())?;
            Ok(ScriptFields::single(owner, name.clone(), value))
        }
        [(first_name, first_register), (second_name, second_register)] => {
            let first_value =
                store_runtime_value(frame.read(*first_register)?, heap, budget.as_deref_mut())?;
            let second_value =
                store_runtime_value(frame.read(*second_register)?, heap, budget.as_deref_mut())?;
            Ok(ScriptFields::two(
                owner,
                first_name.clone(),
                first_value,
                second_name.clone(),
                second_value,
            ))
        }
        [
            (first_name, first_register),
            (second_name, second_register),
            (third_name, third_register),
        ] => {
            let first_value =
                store_runtime_value(frame.read(*first_register)?, heap, budget.as_deref_mut())?;
            let second_value =
                store_runtime_value(frame.read(*second_register)?, heap, budget.as_deref_mut())?;
            let third_value =
                store_runtime_value(frame.read(*third_register)?, heap, budget.as_deref_mut())?;
            Ok(ScriptFields::three(
                owner,
                first_name.clone(),
                first_value,
                second_name.clone(),
                second_value,
                third_name.clone(),
                third_value,
            ))
        }
        [
            (first_name, first_register),
            (second_name, second_register),
            (third_name, third_register),
            (fourth_name, fourth_register),
        ] => {
            let first_value =
                store_runtime_value(frame.read(*first_register)?, heap, budget.as_deref_mut())?;
            let second_value =
                store_runtime_value(frame.read(*second_register)?, heap, budget.as_deref_mut())?;
            let third_value =
                store_runtime_value(frame.read(*third_register)?, heap, budget.as_deref_mut())?;
            let fourth_value =
                store_runtime_value(frame.read(*fourth_register)?, heap, budget.as_deref_mut())?;
            Ok(ScriptFields::four(
                owner,
                [
                    (first_name.clone(), first_value),
                    (second_name.clone(), second_value),
                    (third_name.clone(), third_value),
                    (fourth_name.clone(), fourth_value),
                ],
            ))
        }
        [
            (first_name, first_register),
            (second_name, second_register),
            (third_name, third_register),
            (fourth_name, fourth_register),
            (fifth_name, fifth_register),
        ] => {
            let first_value =
                store_runtime_value(frame.read(*first_register)?, heap, budget.as_deref_mut())?;
            let second_value =
                store_runtime_value(frame.read(*second_register)?, heap, budget.as_deref_mut())?;
            let third_value =
                store_runtime_value(frame.read(*third_register)?, heap, budget.as_deref_mut())?;
            let fourth_value =
                store_runtime_value(frame.read(*fourth_register)?, heap, budget.as_deref_mut())?;
            let fifth_value =
                store_runtime_value(frame.read(*fifth_register)?, heap, budget.as_deref_mut())?;
            Ok(ScriptFields::five(
                owner,
                [
                    (first_name.clone(), first_value),
                    (second_name.clone(), second_value),
                    (third_name.clone(), third_value),
                    (fourth_name.clone(), fourth_value),
                    (fifth_name.clone(), fifth_value),
                ],
            ))
        }
        [
            (first_name, first_register),
            (second_name, second_register),
            (third_name, third_register),
            (fourth_name, fourth_register),
            (fifth_name, fifth_register),
            (sixth_name, sixth_register),
        ] => {
            let first_value =
                store_runtime_value(frame.read(*first_register)?, heap, budget.as_deref_mut())?;
            let second_value =
                store_runtime_value(frame.read(*second_register)?, heap, budget.as_deref_mut())?;
            let third_value =
                store_runtime_value(frame.read(*third_register)?, heap, budget.as_deref_mut())?;
            let fourth_value =
                store_runtime_value(frame.read(*fourth_register)?, heap, budget.as_deref_mut())?;
            let fifth_value =
                store_runtime_value(frame.read(*fifth_register)?, heap, budget.as_deref_mut())?;
            let sixth_value =
                store_runtime_value(frame.read(*sixth_register)?, heap, budget.as_deref_mut())?;
            Ok(ScriptFields::six(
                owner,
                [
                    (first_name.clone(), first_value),
                    (second_name.clone(), second_value),
                    (third_name.clone(), third_value),
                    (fourth_name.clone(), fourth_value),
                    (fifth_name.clone(), fifth_value),
                    (sixth_name.clone(), sixth_value),
                ],
            ))
        }
        _ => fields
            .iter()
            .map(|(name, register)| {
                Ok((
                    name.clone(),
                    store_runtime_value(frame.read(*register)?, heap, budget.as_deref_mut())?,
                ))
            })
            .collect::<VmResult<Vec<_>>>()
            .map(|fields| ScriptFields::from_pairs(owner, fields)),
    }
}

fn loaded_string_constant(
    current: Option<&Value>,
    constant: &str,
    heap: Option<&HeapExecution<'_>>,
) -> Option<Value> {
    let Value::HeapRef(reference) = current? else {
        return None;
    };
    match heap?.heap.get(*reference)? {
        HeapValue::String(value) if value == constant => Some(Value::HeapRef(*reference)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crate::heap::ScriptHeap;

    use super::*;

    #[test]
    fn loaded_string_constant_reuses_matching_heap_string() {
        let mut heap = ScriptHeap::new();
        let tick = Value::HeapRef(heap.allocate(HeapValue::String("tick".to_owned())));
        let other = Value::HeapRef(heap.allocate(HeapValue::String("other".to_owned())));
        let array = Value::HeapRef(heap.allocate(HeapValue::Array(Vec::new())));
        let heap = HeapExecution::new(&mut heap);

        assert_eq!(
            loaded_string_constant(Some(&tick), "tick", Some(&heap)),
            Some(tick)
        );
        assert_eq!(
            loaded_string_constant(Some(&other), "tick", Some(&heap)),
            None
        );
        assert_eq!(
            loaded_string_constant(Some(&array), "tick", Some(&heap)),
            None
        );
        assert_eq!(loaded_string_constant(Some(&tick), "tick", None), None);
        assert_eq!(
            loaded_string_constant(Some(&Value::Null), "tick", Some(&heap)),
            None
        );
    }
}
