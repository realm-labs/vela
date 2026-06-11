use vela_bytecode::linked::{InstructionKind, LinkedMethodDispatchKind};
use vela_bytecode::{
    DebugNameId, FieldSlot, InstructionOffset, LinkedCodeObject, LinkedProgram, LinkedType,
    LinkedVariant, Register, ScriptCallMode, TypeHandle, VariantHandle,
};
use vela_common::Span;

use crate::heap::{EnumIdentity, HeapValue};
use crate::option_result::std_enum_identity_for_names;
use crate::value::{ClosureCode, ClosureValue};

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
        mut host: Option<&mut HostExecution<'_>>,
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
        if call.check_param_guards {
            execute_linked_param_guards(code, call.program, &frame, heap.as_deref())?;
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
                        Constant::Bytes(value) => {
                            if let Some(value) = constant_loads::loaded_bytes_constant(
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
                InstructionKind::BinaryIntLiteral {
                    dst,
                    op,
                    value,
                    literal,
                    side,
                } => {
                    let value = binary_int_literal_numeric(
                        *op,
                        frame.read(*value)?,
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
                        frame.read(*value)?,
                        literal.as_str(),
                        *side,
                    )
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
                InstructionKind::GuardType { src, guard } => {
                    let guard = code.type_guard(*guard).ok_or_else(|| {
                        VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
                            opcode: "GuardType",
                        })
                        .with_source_span_if_absent(instruction.span)
                    })?;
                    runtime_type_guards::execute_linked_guard(
                        frame.read(*src)?,
                        guard,
                        call.program,
                        heap.as_deref(),
                        call.program.debug_name(guard.context.debug_name),
                    )
                    .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
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
                InstructionKind::CallNative {
                    dst,
                    native,
                    debug_name,
                    args,
                } => {
                    let target = call.program.native_function(*native).ok_or_else(|| {
                        VmError::new(VmErrorKind::UnknownNative {
                            name: call.program.debug_name(*debug_name).to_owned(),
                        })
                        .with_source_span_if_absent(instruction.span)
                    })?;
                    native_function_calls::dispatch_linked_native_function_call(
                        self,
                        &mut host,
                        &mut heap,
                        &mut budget,
                        &mut frame,
                        native_function_calls::NativeFunctionCall {
                            dst: *dst,
                            name: call.program.debug_name(target.debug_name),
                            native: target.id,
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
                    let function_code = call.program.function(*function).ok_or_else(|| {
                        VmError::new(VmErrorKind::UnknownFunction {
                            name: call.program.debug_name(*debug_name).to_owned(),
                        })
                        .with_source_span_if_absent(instruction.span)
                    })?;
                    let values =
                        script_function_calls::script_call_args_from_call_arguments(&frame, args)?;
                    let protected_root_len = heap
                        .as_deref_mut()
                        .map(|heap| heap.push_frame_roots(&frame));
                    let result = self.execute_linked_call(
                        LinkedExecutionCall {
                            code: function_code,
                            program: call.program,
                            captures: &[],
                            args: values.as_slice(),
                            check_param_guards: matches!(mode, ScriptCallMode::Checked),
                            call_site: instruction.span,
                            call_site_offset: Some(instruction_offset),
                            inline_caches: call.inline_caches,
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
                    function,
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
                        .with_source_span_if_absent(instruction.span)
                    })?;
                    let value = allocate_heap_value(
                        HeapValue::Closure(ClosureValue {
                            code: ClosureCode::Linked(*function),
                            captures,
                        }),
                        heap,
                        budget.as_deref_mut(),
                    )?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::CallClosure { dst, callee, args } => {
                    let (function, captures) = {
                        let closure = runtime_checks::expect_closure_ref(
                            frame.read(*callee)?,
                            heap.as_deref(),
                            "closure call",
                        )?;
                        let ClosureCode::Linked(function) = &closure.code else {
                            return Err(VmError::new(VmErrorKind::TypeMismatch {
                                operation: "closure call",
                            })
                            .with_source_span_if_absent(instruction.span));
                        };
                        let function = *function;
                        let captures =
                            SmallStorage::try_from_slice_map(&closure.captures, 4, |value| {
                                Ok::<_, VmError>(*value)
                            })?;
                        (function, captures)
                    };
                    let function_code = call.program.function(function).ok_or_else(|| {
                        VmError::new(VmErrorKind::UnknownFunction {
                            name: format!("<linked closure#{}>", function.index()),
                        })
                        .with_source_span_if_absent(instruction.span)
                    })?;
                    let values = SmallStorage::try_from_slice_map(args, 4, |register| {
                        Ok::<_, VmError>(*frame.read(*register)?)
                    })?;
                    let protected_root_len = heap
                        .as_deref_mut()
                        .map(|heap| heap.push_frame_roots(&frame));
                    let result = self.execute_linked_call(
                        LinkedExecutionCall {
                            code: function_code,
                            program: call.program,
                            captures: captures.as_slice(),
                            args: values.as_slice(),
                            check_param_guards: true,
                            call_site: instruction.span,
                            call_site_offset: Some(instruction_offset),
                            inline_caches: call.inline_caches,
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
                    dispatch,
                    debug_name,
                    args,
                } => {
                    let dispatch = call.program.method_dispatch(*dispatch).ok_or_else(|| {
                        VmError::new(VmErrorKind::UnknownMethod {
                            method: call.program.debug_name(*debug_name).to_owned(),
                        })
                        .with_source_span_if_absent(instruction.span)
                    })?;
                    let values =
                        script_function_calls::script_call_args_from_call_arguments(&frame, args)?;
                    match &dispatch.kind {
                        LinkedMethodDispatchKind::Script {
                            method_id: _,
                            function,
                        } => {
                            let function_code =
                                call.program.function(*function).ok_or_else(|| {
                                    VmError::new(VmErrorKind::UnknownMethod {
                                        method: call
                                            .program
                                            .debug_name(dispatch.debug_name)
                                            .to_owned(),
                                    })
                                    .with_source_span_if_absent(instruction.span)
                                })?;
                            let receiver_value = *frame.read(*receiver)?;
                            let mut method_args = Vec::with_capacity(values.as_slice().len() + 1);
                            method_args.push(receiver_value);
                            method_args.extend(values.as_slice().iter().copied());
                            let protected_root_len = heap
                                .as_deref_mut()
                                .map(|heap| heap.push_frame_roots(&frame));
                            let result = self.execute_linked_call(
                                LinkedExecutionCall {
                                    code: function_code,
                                    program: call.program,
                                    captures: &[],
                                    args: method_args.as_slice(),
                                    check_param_guards: true,
                                    call_site: instruction.span,
                                    call_site_offset: Some(instruction_offset),
                                    inline_caches: call.inline_caches,
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
                        LinkedMethodDispatchKind::Value { method_id } => {
                            script_method_calls::dispatch_linked_method_id_call(
                                self,
                                call.program,
                                &mut host,
                                &mut heap,
                                &mut budget,
                                &mut frame,
                                script_method_calls::ScriptMethodIdCall {
                                    dst: *dst,
                                    receiver: *receiver,
                                    method: call.program.debug_name(dispatch.debug_name),
                                    method_id: *method_id,
                                    values: values.as_slice(),
                                },
                            )?;
                        }
                        LinkedMethodDispatchKind::Host { method_id } => {
                            let return_value = host_access::execute_host_root_method_call(
                                host_access::HostAccessRuntime {
                                    frame: &frame,
                                    heap: heap.as_deref_mut(),
                                    budget: budget.as_deref_mut(),
                                    host: host.as_deref_mut(),
                                    inline_caches: call.inline_caches,
                                    source_span: instruction.span,
                                },
                                *receiver,
                                host_access::HostRootMethodCall {
                                    method: *method_id,
                                    args: values.as_slice(),
                                    wants_return: true,
                                },
                            )?;
                            if let Some(return_value) = return_value {
                                frame.write(*dst, return_value)?;
                            }
                        }
                    }
                }
                InstructionKind::TryPropagate { dst, src } => {
                    match try_propagate_value(frame.read(*src)?, heap.as_deref())? {
                        TryPropagation::Continue(value) => frame.write(*dst, value)?,
                        TryPropagation::Return(value) => {
                            return execute_linked_return_guard(
                                code,
                                call.program,
                                value,
                                heap.as_deref(),
                            );
                        }
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
                    let entries = entries
                        .iter()
                        .map(|(key, register)| {
                            let Some(Constant::String(key)) = code.constants.get(key.0) else {
                                return Err(VmError::new(VmErrorKind::ConstantOutOfBounds {
                                    constant: key.0,
                                })
                                .with_source_span(instruction.span));
                            };
                            Ok((key.clone(), *register))
                        })
                        .collect::<VmResult<Vec<_>>>()?;
                    script_aggregate_construction::make_map(
                        &mut frame,
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                        *dst,
                        &entries,
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
                    let linked_ty = linked_type(call.program, *ty, "MakeRecord")?;
                    let type_name = call.program.debug_name(linked_ty.debug_name);
                    let fields = linked_object_fields(call.program, fields);
                    script_object_construction::make_record_with_identity(
                        &mut frame,
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                        *dst,
                        script_object_construction::RecordConstruction {
                            type_name,
                            type_id: Some(linked_ty.id),
                            fields: &fields,
                        },
                    )?;
                }
                InstructionKind::GetRecordSlot {
                    dst,
                    record,
                    field,
                    debug_name,
                } => {
                    field_access::dispatch_get_record_slot(
                        &mut frame,
                        heap.as_deref_mut(),
                        *dst,
                        *record,
                        call.program.debug_name(*debug_name),
                        field.index(),
                    )?;
                }
                InstructionKind::SetRecordSlot {
                    record,
                    field,
                    debug_name,
                    src,
                } => {
                    field_access::dispatch_set_record_slot(
                        &mut frame,
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                        *record,
                        call.program.debug_name(*debug_name),
                        field.index(),
                        *src,
                    )?;
                }
                InstructionKind::MakeEnum {
                    dst,
                    enum_ty,
                    variant,
                    fields,
                } => {
                    let enum_ty = linked_type(call.program, *enum_ty, "MakeEnum")?;
                    let variant = linked_variant(call.program, *variant, "MakeEnum")?;
                    let enum_name = call.program.debug_name(enum_ty.debug_name);
                    let variant_name = linked_variant_short_name(call.program, variant);
                    let identity = std_enum_identity_for_names(enum_name, variant_name)
                        .unwrap_or_else(|| linked_enum_identity(enum_ty, variant));
                    let fields = linked_object_fields(call.program, fields);
                    script_object_construction::make_enum_with_identity(
                        &mut frame,
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                        *dst,
                        script_object_construction::EnumConstruction {
                            enum_name,
                            variant: variant_name,
                            identity: Some(identity),
                            fields: &fields,
                        },
                    )?;
                }
                InstructionKind::GetEnumSlot {
                    dst,
                    value,
                    field,
                    debug_name,
                } => {
                    field_access::dispatch_get_enum_slot(
                        &mut frame,
                        heap.as_deref_mut(),
                        *dst,
                        *value,
                        call.program.debug_name(*debug_name),
                        field.index(),
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
                    if let Some(target) = linked_iter_next(
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
                    if let Some(target) = linked_range_next(
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
                    let enum_ty = linked_type(call.program, *enum_ty, "EnumTagEqual")?;
                    let variant = linked_variant(call.program, *variant, "EnumTagEqual")?;
                    let matches = field_access::enum_tag_id_equal(
                        frame.read(*value)?,
                        enum_ty.id,
                        variant.id,
                        heap.as_deref(),
                    );
                    frame.write(*dst, Value::Bool(matches))?;
                }
                InstructionKind::LoadGlobal {
                    dst,
                    slot,
                    debug_name,
                    cache_site,
                } => {
                    let cached_slot = cache_site
                        .and_then(|site| {
                            call.inline_caches
                                .and_then(|caches| caches.global_read_slot(site))
                        })
                        .or(Some(*slot));
                    let value = host_access::load_host_global(
                        host_access::HostAccessRuntime {
                            frame: &frame,
                            heap: heap.as_deref_mut(),
                            budget: budget.as_deref_mut(),
                            host: host.as_deref_mut(),
                            inline_caches: call.inline_caches,
                            source_span: instruction.span,
                        },
                        call.program.debug_name(*debug_name),
                        cached_slot,
                    )?;
                    if let (Some(caches), Some(cache_site)) = (call.inline_caches, *cache_site)
                        && caches.global_read_slot(cache_site).is_none()
                    {
                        caches.set_global_read_slot(cache_site, *slot);
                    }
                    frame.write(*dst, value)?;
                }
                InstructionKind::HostRead {
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
                InstructionKind::HostWrite {
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
                InstructionKind::HostMutate {
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
                InstructionKind::HostRemove {
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
                    let method_id = match call.program.method_dispatch(*method).map(|d| &d.kind) {
                        Some(LinkedMethodDispatchKind::Host { method_id }) => *method_id,
                        _ => {
                            return Err(VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
                                opcode: "HostCall",
                            })
                            .with_source_span_if_absent(instruction.span));
                        }
                    };
                    let plan = host_access::code_host_target(
                        &code.host_targets,
                        *target,
                        instruction.span,
                    )?;
                    let value = host_access::execute_host_call(
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
                            method: method_id,
                            args,
                            wants_return: dst.is_some(),
                            cache_site: *cache_site,
                        },
                    )?;
                    if let (Some(dst), Some(value)) = (dst, value) {
                        frame.write(*dst, value)?;
                    }
                }
                InstructionKind::Return { src } => {
                    return execute_linked_return_guard(
                        code,
                        call.program,
                        *frame.read(*src)?,
                        heap.as_deref(),
                    );
                }
            }

            if let Some(heap) = heap.as_deref_mut() {
                heap.collect_frame_at_safe_point(&frame, budget.as_deref_mut());
            }
        }

        Err(VmError::new(VmErrorKind::MissingReturn))
    }
}

fn execute_linked_param_guards(
    code: &LinkedCodeObject,
    program: &LinkedProgram,
    frame: &CallFrame,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<()> {
    let param_offset = usize::from(code.capture_count);
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
        let guard = code.type_guard(param_guard.guard).ok_or_else(|| {
            VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
                opcode: "param_guard",
            })
        })?;
        runtime_type_guards::execute_linked_guard(
            value,
            guard,
            program,
            heap,
            program.debug_name(guard.context.debug_name),
        )?;
        debug_assert!(usize::from(param_guard.parameter) < code.params.len());
        debug_assert!(usize::from(register.0) >= param_offset);
    }
    Ok(())
}

fn execute_linked_return_guard(
    code: &LinkedCodeObject,
    program: &LinkedProgram,
    value: Value,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    let Some(guard_id) = code.return_guard else {
        return Ok(value);
    };
    let guard = code.type_guard(guard_id).ok_or_else(|| {
        VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
            opcode: "return_guard",
        })
    })?;
    runtime_type_guards::execute_linked_guard(
        &value,
        guard,
        program,
        heap,
        program.debug_name(guard.context.debug_name),
    )?;
    Ok(value)
}

fn validate_linked_jump(code: &LinkedCodeObject, offset: usize) -> VmResult<()> {
    if offset <= code.instructions.len() {
        Ok(())
    } else {
        Err(VmError::new(VmErrorKind::InstructionOutOfBounds { offset }))
    }
}

fn linked_iter_next(
    mut runtime: iteration::IterRuntime<'_, '_>,
    code: &LinkedCodeObject,
    iterator: Register,
    dst: Register,
    jump_if_done: InstructionOffset,
) -> VmResult<Option<usize>> {
    let value = *runtime.frame.read(iterator)?;
    let next = match value {
        Value::HeapRef(reference) => {
            let Some(HeapValue::Iterator(iterator_state)) = runtime
                .heap
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
            runtime.frame.write(dst, value)?;
            Ok(None)
        }
        None => {
            validate_linked_jump(code, jump_if_done.0)?;
            Ok(Some(jump_if_done.0))
        }
    }
}

fn linked_range_next(
    frame: &mut CallFrame,
    code: &LinkedCodeObject,
    step: iteration::RangeNextStep,
) -> VmResult<Option<usize>> {
    let is_done = match frame.read(step.done)? {
        Value::Bool(value) => *value,
        _ => {
            return Err(VmError::new(VmErrorKind::TypeMismatch {
                operation: "range",
            }));
        }
    };
    if is_done {
        validate_linked_jump(code, step.jump_if_done.0)?;
        return Ok(Some(step.jump_if_done.0));
    }

    let current = expect_int(frame.read(step.cursor)?, "range")?;
    let end = expect_int(frame.read(step.end)?, "range")?;
    let has_next = if step.inclusive {
        current <= end
    } else {
        current < end
    };
    if has_next {
        frame.write(
            step.dst,
            Value::Scalar(vela_common::ScalarValue::I64(current)),
        )?;
        if current == i64::MAX {
            frame.write(step.done, Value::Bool(true))?;
        } else {
            frame.write(
                step.cursor,
                Value::Scalar(vela_common::ScalarValue::I64(current + 1)),
            )?;
        }
        Ok(None)
    } else {
        frame.write(step.done, Value::Bool(true))?;
        validate_linked_jump(code, step.jump_if_done.0)?;
        Ok(Some(step.jump_if_done.0))
    }
}

fn linked_type<'program>(
    program: &'program LinkedProgram,
    ty: TypeHandle,
    opcode: &'static str,
) -> VmResult<&'program LinkedType> {
    program
        .ty(ty)
        .ok_or_else(|| VmError::new(VmErrorKind::UnsupportedLinkedInstruction { opcode }))
}

fn linked_variant<'program>(
    program: &'program LinkedProgram,
    variant: VariantHandle,
    opcode: &'static str,
) -> VmResult<&'program LinkedVariant> {
    program
        .variant(variant)
        .ok_or_else(|| VmError::new(VmErrorKind::UnsupportedLinkedInstruction { opcode }))
}

fn linked_variant_short_name<'program>(
    program: &'program LinkedProgram,
    variant: &LinkedVariant,
) -> &'program str {
    program
        .debug_name(variant.debug_name)
        .rsplit_once("::")
        .map_or_else(|| program.debug_name(variant.debug_name), |(_, name)| name)
}

fn linked_enum_identity(enum_ty: &LinkedType, variant: &LinkedVariant) -> EnumIdentity {
    EnumIdentity::new(enum_ty.id, variant.id, None)
}

fn linked_object_fields(
    program: &LinkedProgram,
    fields: &[(FieldSlot, DebugNameId, Register)],
) -> Vec<(String, Register)> {
    fields
        .iter()
        .map(|(_, debug_name, register)| (program.debug_name(*debug_name).to_owned(), *register))
        .collect()
}
