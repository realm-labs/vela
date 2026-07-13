use std::sync::Arc;
use vela_bytecode::linked::InstructionKind;

use vela_bytecode::{InstructionOffset, LinkedArtifact, Register, ScriptFunctionHandle};
use vela_common::Span;

use crate::budget::ExecutionBudget;
use crate::equality::{ResumableComparison, ResumableComparisonKind, ResumableComparisonStep};
use crate::error::{VmError, VmErrorKind, VmResult, VmStackFrame};
use crate::frame::CallFrame;
use crate::heap_execution::HeapExecution;
use crate::numeric_ops::{
    add_numeric, binary_float_literal_numeric, binary_int_literal_numeric, div_numeric,
    mul_numeric, negate_numeric, rem_numeric, sub_numeric,
};
use crate::resumable_callbacks::{ResumableCallbackMethod, ResumableCallbackStep};
use crate::runtime_checks::is_truthy;
use crate::value::Value;
use crate::{
    HostExecution, Vm, VmBytecodeProfiler, VmInlineCaches, identity_equal, identity_not_equal,
    store_value_in_heap_if_needed, validate_inline_cache_layout,
};
use crate::{
    array_methods, callback_method_dispatch, closure_calls, constant_loads, field_access,
    format_strings, host_access, i64_ops, indexing, iteration, native_function_calls,
    runtime_type_guards, script_aggregate_construction, script_builtin_methods,
    script_function_calls, script_method_calls, script_object_construction, try_propagation,
    tuple_fields,
};

pub(crate) struct LinkedExecutionCall<'a> {
    pub(crate) owner: Arc<LinkedArtifact>,
    pub(crate) function: ScriptFunctionHandle,
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
        let program = self.owner.program();
        let Some(code) = program.function(self.function) else {
            return VmStackFrame::new("<missing linked function>", self.call_site)
                .with_bytecode_offset(self.call_site_offset);
        };
        VmStackFrame::new(program.debug_name(code.debug_name), self.call_site)
            .with_bytecode_offset(self.call_site_offset)
    }
}

struct ExecutionSession<'a> {
    frames: Vec<ExecutionFrame<'a>>,
}

struct ExecutionFrame<'a> {
    owner: Arc<LinkedArtifact>,
    function: ScriptFunctionHandle,
    ip: InstructionOffset,
    registers: CallFrame,
    return_to: Option<ReturnContinuation>,
    pending_operation: Option<PendingFrameOperation>,
    call_site: Option<Span>,
    call_site_offset: Option<InstructionOffset>,
    inline_caches: Option<&'a dyn VmInlineCaches>,
    bytecode_profiler: Option<&'a dyn VmBytecodeProfiler>,
}

impl ExecutionFrame<'_> {
    fn stack_frame(&self) -> VmStackFrame {
        let program = self.owner.program();
        let Some(code) = program.function(self.function) else {
            return VmStackFrame::new("<missing linked function>", self.call_site)
                .with_bytecode_offset(self.call_site_offset);
        };
        VmStackFrame::new(program.debug_name(code.debug_name), self.call_site)
            .with_bytecode_offset(self.call_site_offset)
    }
}

#[derive(Clone, Copy)]
struct ReturnContinuation {
    target: PendingReturnTarget,
    protected_root_len: Option<usize>,
}

#[derive(Clone, Copy)]
enum PendingReturnTarget {
    Register(Register),
    Operation,
}

enum PendingFrameOperation {
    Comparison {
        comparison: ResumableComparison,
        destination: Register,
        returned: Option<Value>,
        source_span: Option<Span>,
    },
    ArrayOrdering {
        ordering: array_methods::ResumableArrayOrdering,
        destination: Register,
        returned: Option<Value>,
        source_span: Option<Span>,
    },
    CallbackMethod {
        callback: ResumableCallbackMethod,
        destination: Register,
        returned: Option<Value>,
        source_span: Option<Span>,
    },
}

struct PendingLinkedCall<'a> {
    owner: Arc<LinkedArtifact>,
    function: ScriptFunctionHandle,
    captures: Vec<Value>,
    args: Vec<Value>,
    check_param_guards: bool,
    call_site: Option<Span>,
    call_site_offset: Option<InstructionOffset>,
    inline_caches: Option<&'a dyn VmInlineCaches>,
    bytecode_profiler: Option<&'a dyn VmBytecodeProfiler>,
    return_target: PendingReturnTarget,
}

impl PendingLinkedCall<'_> {
    fn stack_frame(&self) -> VmStackFrame {
        let program = self.owner.program();
        let Some(code) = program.function(self.function) else {
            return VmStackFrame::new("<missing linked function>", self.call_site)
                .with_bytecode_offset(self.call_site_offset);
        };
        VmStackFrame::new(program.debug_name(code.debug_name), self.call_site)
            .with_bytecode_offset(self.call_site_offset)
    }
}

enum FrameDriveOutcome<'a> {
    Continue,
    Push(PendingLinkedCall<'a>),
    Return(Value),
}

#[derive(Clone, Copy)]
struct FrameDispatchContext<'a> {
    inline_caches: Option<&'a dyn VmInlineCaches>,
    bytecode_profiler: Option<&'a dyn VmBytecodeProfiler>,
}

impl Vm {
    pub(crate) fn execute_linked_call(
        &self,
        call: LinkedExecutionCall<'_>,
        host: Option<&mut HostExecution<'_>>,
        heap: Option<&mut HeapExecution<'_>>,
        mut budget: Option<&mut ExecutionBudget>,
    ) -> VmResult<Value> {
        let program = call.owner.program();
        let code = program.function(call.function).ok_or_else(|| {
            VmError::new(VmErrorKind::UnknownFunction {
                name: format!("<linked function#{}>", call.function.index()),
            })
        })?;
        let limits_call_depth = budget
            .as_deref()
            .is_some_and(ExecutionBudget::limits_call_depth);
        if limits_call_depth {
            let budget = budget
                .as_deref_mut()
                .expect("call-depth budget mode requires a budget");
            budget
                .enter_call()
                .map_err(|error| error.with_call_frame(call.stack_frame()))?;
        }
        let frame = call.stack_frame();
        let fallback_span = call.call_site.or_else(|| {
            code.instructions
                .first()
                .and_then(|instruction| instruction.span)
        });
        let charges_execution_units = budget
            .as_deref()
            .is_some_and(ExecutionBudget::charges_execution_units);
        let has_profiler = call.bytecode_profiler.is_some();
        let result = match (charges_execution_units, has_profiler) {
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
        if limits_call_depth {
            budget
                .expect("call-depth budget mode requires a budget")
                .exit_call();
        }
        result
    }

    fn execute_linked_body<'a, const CHARGE_BUDGET: bool, const PROFILE: bool>(
        &self,
        call: LinkedExecutionCall<'a>,
        mut host: Option<&mut HostExecution<'_>>,
        mut heap: Option<&mut HeapExecution<'_>>,
        mut budget: Option<&mut ExecutionBudget>,
    ) -> VmResult<Value> {
        let entry = self.prepare_execution_frame(
            call.owner,
            call.function,
            call.captures,
            call.args,
            call.check_param_guards,
            call.call_site,
            call.call_site_offset,
            call.inline_caches,
            call.bytecode_profiler,
            None,
            heap.as_deref_mut(),
            budget.as_deref_mut(),
        )?;
        let mut session = ExecutionSession {
            frames: vec![entry],
        };
        let limits_call_depth = budget
            .as_deref()
            .is_some_and(ExecutionBudget::limits_call_depth);

        loop {
            let outcome = {
                let active = session
                    .frames
                    .last_mut()
                    .expect("an execution session retains an active frame");
                self.drive_linked_frame::<CHARGE_BUDGET, PROFILE>(
                    active,
                    &mut host,
                    &mut heap,
                    &mut budget,
                )
            };
            let outcome = match outcome {
                Ok(outcome) => outcome,
                Err(mut error) => {
                    for frame in session.frames.iter().skip(1).rev() {
                        error = error.with_call_frame(frame.stack_frame());
                    }
                    if limits_call_depth {
                        for _ in 1..session.frames.len() {
                            budget
                                .as_deref_mut()
                                .expect("call-depth budget mode requires a budget")
                                .exit_call();
                        }
                    }
                    return Err(error);
                }
            };
            match outcome {
                FrameDriveOutcome::Continue => {}
                FrameDriveOutcome::Push(pending) => {
                    if limits_call_depth {
                        let enter = budget
                            .as_deref_mut()
                            .expect("call-depth budget mode requires a budget")
                            .enter_call();
                        if let Err(mut error) = enter {
                            error = error.with_call_frame(pending.stack_frame());
                            for frame in session.frames.iter().skip(1).rev() {
                                error = error.with_call_frame(frame.stack_frame());
                            }
                            for _ in 1..session.frames.len() {
                                budget
                                    .as_deref_mut()
                                    .expect("call-depth budget mode requires a budget")
                                    .exit_call();
                            }
                            return Err(error);
                        }
                    }
                    let protected_root_len = heap.as_deref_mut().map(|heap| {
                        let caller = session.frames.last().expect("caller frame");
                        let protected_root_len = heap.push_frame_roots(&caller.registers);
                        if let Some(PendingFrameOperation::CallbackMethod { callback, .. }) =
                            caller.pending_operation.as_ref()
                        {
                            callback.protect_roots(heap);
                        }
                        protected_root_len
                    });
                    let return_to = ReturnContinuation {
                        target: pending.return_target,
                        protected_root_len,
                    };
                    let pending_frame = pending.stack_frame();
                    let child = self.prepare_execution_frame(
                        pending.owner,
                        pending.function,
                        &pending.captures,
                        &pending.args,
                        pending.check_param_guards,
                        pending.call_site,
                        pending.call_site_offset,
                        pending.inline_caches,
                        pending.bytecode_profiler,
                        Some(return_to),
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                    );
                    let child = match child {
                        Ok(child) => child,
                        Err(mut error) => {
                            if let (Some(heap), Some(protected_root_len)) =
                                (heap.as_deref_mut(), protected_root_len)
                            {
                                heap.truncate_protected_roots(protected_root_len);
                            }
                            if limits_call_depth {
                                budget
                                    .as_deref_mut()
                                    .expect("call-depth budget mode requires a budget")
                                    .exit_call();
                                for _ in 1..session.frames.len() {
                                    budget
                                        .as_deref_mut()
                                        .expect("call-depth budget mode requires a budget")
                                        .exit_call();
                                }
                            }
                            error = error.with_call_frame(pending_frame);
                            for frame in session.frames.iter().skip(1).rev() {
                                error = error.with_call_frame(frame.stack_frame());
                            }
                            return Err(error);
                        }
                    };
                    session.frames.push(child);
                }
                FrameDriveOutcome::Return(value) => {
                    let finished = session.frames.pop().expect("returning frame");
                    let Some(return_to) = finished.return_to else {
                        return Ok(value);
                    };
                    let value = store_value_in_heap_if_needed(
                        value,
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                    )?;
                    if let (Some(heap), Some(protected_root_len)) =
                        (heap.as_deref_mut(), return_to.protected_root_len)
                    {
                        heap.truncate_protected_roots(protected_root_len);
                    }
                    let caller = session
                        .frames
                        .last_mut()
                        .expect("return continuation has a caller");
                    match return_to.target {
                        PendingReturnTarget::Register(destination) => {
                            caller.registers.write(destination, value)?;
                        }
                        PendingReturnTarget::Operation => {
                            let Some(operation) = caller.pending_operation.as_mut() else {
                                return Err(VmError::new(
                                    VmErrorKind::UnsupportedLinkedInstruction {
                                        opcode: "missing pending operation continuation",
                                    },
                                ));
                            };
                            match operation {
                                PendingFrameOperation::Comparison { returned, .. }
                                | PendingFrameOperation::ArrayOrdering { returned, .. }
                                | PendingFrameOperation::CallbackMethod { returned, .. } => {
                                    *returned = Some(value);
                                }
                            }
                        }
                    }
                    if limits_call_depth {
                        budget
                            .as_deref_mut()
                            .expect("call-depth budget mode requires a budget")
                            .exit_call();
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_execution_frame<'a>(
        &self,
        owner: Arc<LinkedArtifact>,
        function: ScriptFunctionHandle,
        captures: &[Value],
        args: &[Value],
        check_param_guards: bool,
        call_site: Option<Span>,
        call_site_offset: Option<InstructionOffset>,
        inline_caches: Option<&'a dyn VmInlineCaches>,
        bytecode_profiler: Option<&'a dyn VmBytecodeProfiler>,
        return_to: Option<ReturnContinuation>,
        heap: Option<&mut HeapExecution<'_>>,
        budget: Option<&mut ExecutionBudget>,
    ) -> VmResult<ExecutionFrame<'a>> {
        let program = owner.program();
        let code = program.function(function).ok_or_else(|| {
            VmError::new(VmErrorKind::UnknownFunction {
                name: format!("<linked function#{}>", function.index()),
            })
        })?;
        validate_inline_cache_layout(inline_caches, code.cache_sites.len())?;
        let function_name = program.debug_name(code.debug_name);
        if captures.len() != usize::from(code.capture_count) {
            return Err(VmError::new(VmErrorKind::ArityMismatch {
                name: function_name.to_owned(),
                expected: usize::from(code.capture_count),
                actual: captures.len(),
            }));
        }
        if args.len() > code.params.len() {
            return Err(VmError::new(VmErrorKind::ArityMismatch {
                name: function_name.to_owned(),
                expected: code.params.len(),
                actual: args.len(),
            }));
        }

        let mut frame = CallFrame::new_linked(code.register_count, &owner);
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
                    name: function_name.to_owned(),
                    expected: code.params.len(),
                    actual,
                }));
            }
        }
        if check_param_guards {
            let mut guard_context = runtime_type_guards::GuardExecutionContext::new(heap, budget);
            runtime_type_guards::execute_linked_param_guards(
                code,
                program,
                &frame,
                &mut guard_context,
            )?;
        }

        Ok(ExecutionFrame {
            owner,
            function,
            ip: InstructionOffset(0),
            registers: frame,
            return_to,
            pending_operation: None,
            call_site,
            call_site_offset,
            inline_caches,
            bytecode_profiler,
        })
    }

    fn drive_linked_frame<'a, const CHARGE_BUDGET: bool, const PROFILE: bool>(
        &self,
        frame_state: &mut ExecutionFrame<'a>,
        host: &mut Option<&mut HostExecution<'_>>,
        heap: &mut Option<&mut HeapExecution<'_>>,
        budget: &mut Option<&mut ExecutionBudget>,
    ) -> VmResult<FrameDriveOutcome<'a>> {
        let current_owner = Arc::clone(&frame_state.owner);
        let program = current_owner.program();
        let code = program.function(frame_state.function).ok_or_else(|| {
            VmError::new(VmErrorKind::UnknownFunction {
                name: format!("<linked function#{}>", frame_state.function.index()),
            })
        })?;
        let call = FrameDispatchContext {
            inline_caches: frame_state.inline_caches,
            bytecode_profiler: frame_state.bytecode_profiler,
        };
        if let Some(operation) = frame_state.pending_operation.take() {
            let pending = match operation {
                PendingFrameOperation::Comparison {
                    mut comparison,
                    destination,
                    returned,
                    source_span,
                } => match comparison
                    .step(self, program, heap, budget, returned)
                    .map_err(|error| error.with_source_span_if_absent(source_span))?
                {
                    ResumableComparisonStep::Complete(value) => {
                        frame_state.registers.write(destination, value)?;
                        None
                    }
                    ResumableComparisonStep::CompleteOrdering(_) => {
                        return Err(VmError::new(VmErrorKind::UnsupportedLinkedInstruction {
                            opcode: "ordering result escaped comparison continuation",
                        }));
                    }
                    ResumableComparisonStep::Call { function, args } => Some((
                        Arc::clone(&current_owner),
                        function,
                        Vec::new(),
                        args,
                        source_span,
                        PendingFrameOperation::Comparison {
                            comparison,
                            destination,
                            returned: None,
                            source_span,
                        },
                    )),
                },
                PendingFrameOperation::ArrayOrdering {
                    mut ordering,
                    destination,
                    returned,
                    source_span,
                } => match ordering
                    .step(self, program, heap, budget, returned)
                    .map_err(|error| error.with_source_span_if_absent(source_span))?
                {
                    array_methods::ResumableArrayOrderingStep::Complete(value) => {
                        frame_state.registers.write(destination, value)?;
                        None
                    }
                    array_methods::ResumableArrayOrderingStep::Call { function, args } => Some((
                        Arc::clone(&current_owner),
                        function,
                        Vec::new(),
                        args,
                        source_span,
                        PendingFrameOperation::ArrayOrdering {
                            ordering,
                            destination,
                            returned: None,
                            source_span,
                        },
                    )),
                },
                PendingFrameOperation::CallbackMethod {
                    mut callback,
                    destination,
                    returned,
                    source_span,
                } => match callback
                    .step(self, &current_owner, heap, budget, returned)
                    .map_err(|error| error.with_source_span_if_absent(source_span))?
                {
                    ResumableCallbackStep::Complete(value) => {
                        frame_state.registers.write(destination, value)?;
                        None
                    }
                    ResumableCallbackStep::Call {
                        owner,
                        function,
                        captures,
                        args,
                    } => Some((
                        owner,
                        function,
                        captures,
                        args,
                        source_span,
                        PendingFrameOperation::CallbackMethod {
                            callback,
                            destination,
                            returned: None,
                            source_span,
                        },
                    )),
                },
            };
            let Some((owner, function, captures, args, source_span, resumed_operation)) = pending
            else {
                return Ok(FrameDriveOutcome::Continue);
            };
            let target = owner.function(function).ok_or_else(|| {
                VmError::new(VmErrorKind::UnknownFunction {
                    name: format!("<linked function#{}>", function.index()),
                })
            })?;
            if target.asyncness.is_async() {
                return Err(VmError::new(VmErrorKind::AsyncCallRequiresAwait {
                    name: owner.program().debug_name(target.debug_name).to_owned(),
                })
                .with_source_span_if_absent(source_span));
            }
            let inline_caches = if owner.generation() == program.generation() {
                call.inline_caches
            } else {
                call.inline_caches
                    .and_then(|caches| caches.for_generation(owner.generation()))
            };
            let bytecode_profiler = if owner.generation() == program.generation() {
                call.bytecode_profiler
            } else {
                call.bytecode_profiler
                    .and_then(|profiler| profiler.for_generation(owner.generation()))
            };
            frame_state.pending_operation = Some(resumed_operation);
            return Ok(FrameDriveOutcome::Push(PendingLinkedCall {
                owner,
                function,
                captures,
                args,
                check_param_guards: true,
                call_site: source_span,
                call_site_offset: frame_state.ip.0.checked_sub(1).map(InstructionOffset),
                inline_caches,
                bytecode_profiler,
                return_target: PendingReturnTarget::Operation,
            }));
        }
        let mut ip = frame_state.ip.0;
        let frame = &mut frame_state.registers;
        while ip < code.instructions.len() {
            let instruction_offset = InstructionOffset(ip);
            let instruction = &code.instructions[ip];
            if PROFILE {
                call.bytecode_profiler
                    .expect("profile execution mode requires a profiler")
                    .record_instruction(code.debug_name, instruction_offset);
            }
            ip = ip.saturating_add(1);

            if CHARGE_BUDGET && instruction.execution_units != 0 {
                budget
                    .as_deref_mut()
                    .expect("execution-unit budget mode requires a budget")
                    .charge_execution_units(u64::from(instruction.execution_units))
                    .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
            }

            let (instruction_kind, await_resume) = match &instruction.kind {
                InstructionKind::AwaitCall { operation, resume } => {
                    (operation.as_ref(), Some(*resume))
                }
                kind => (kind, None),
            };
            match instruction_kind {
                InstructionKind::ChargeExecutionUnits { units } => {
                    if CHARGE_BUDGET {
                        budget
                            .as_deref_mut()
                            .expect("execution-unit budget mode requires a budget")
                            .charge_execution_units(u64::from(*units))
                            .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                    }
                }
                InstructionKind::LoadConst { dst, constant } => {
                    let constant_value = code.constants.get(constant.0).ok_or_else(|| {
                        VmError::new(VmErrorKind::ConstantOutOfBounds {
                            constant: constant.0,
                        })
                        .with_source_span(instruction.span)
                    })?;
                    constant_loads::dispatch_load_const(
                        frame,
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
                    frame_state.ip = InstructionOffset(ip);
                    frame_state.pending_operation = Some(PendingFrameOperation::Comparison {
                        comparison: ResumableComparison::new(
                            ResumableComparisonKind::Equal,
                            frame.read(*lhs)?,
                            frame.read(*rhs)?,
                        ),
                        destination: *dst,
                        returned: None,
                        source_span: instruction.span,
                    });
                    return Ok(FrameDriveOutcome::Continue);
                }
                InstructionKind::NotEqual { dst, lhs, rhs } => {
                    frame_state.ip = InstructionOffset(ip);
                    frame_state.pending_operation = Some(PendingFrameOperation::Comparison {
                        comparison: ResumableComparison::new(
                            ResumableComparisonKind::NotEqual,
                            frame.read(*lhs)?,
                            frame.read(*rhs)?,
                        ),
                        destination: *dst,
                        returned: None,
                        source_span: instruction.span,
                    });
                    return Ok(FrameDriveOutcome::Continue);
                }
                InstructionKind::IdentityEqual { dst, lhs, rhs } => {
                    let value = Value::Bool(
                        identity_equal(&frame.read(*lhs)?, &frame.read(*rhs)?, heap.as_deref())
                            .map_err(|error| error.with_source_span_if_absent(instruction.span))?,
                    );
                    frame.write(*dst, value)?;
                }
                InstructionKind::IdentityNotEqual { dst, lhs, rhs } => {
                    let value = Value::Bool(
                        identity_not_equal(&frame.read(*lhs)?, &frame.read(*rhs)?, heap.as_deref())
                            .map_err(|error| error.with_source_span_if_absent(instruction.span))?,
                    );
                    frame.write(*dst, value)?;
                }
                InstructionKind::Less { dst, lhs, rhs } => {
                    frame_state.ip = InstructionOffset(ip);
                    frame_state.pending_operation = Some(PendingFrameOperation::Comparison {
                        comparison: ResumableComparison::new(
                            ResumableComparisonKind::Less,
                            frame.read(*lhs)?,
                            frame.read(*rhs)?,
                        ),
                        destination: *dst,
                        returned: None,
                        source_span: instruction.span,
                    });
                    return Ok(FrameDriveOutcome::Continue);
                }
                InstructionKind::LessEqual { dst, lhs, rhs } => {
                    frame_state.ip = InstructionOffset(ip);
                    frame_state.pending_operation = Some(PendingFrameOperation::Comparison {
                        comparison: ResumableComparison::new(
                            ResumableComparisonKind::LessEqual,
                            frame.read(*lhs)?,
                            frame.read(*rhs)?,
                        ),
                        destination: *dst,
                        returned: None,
                        source_span: instruction.span,
                    });
                    return Ok(FrameDriveOutcome::Continue);
                }
                InstructionKind::Greater { dst, lhs, rhs } => {
                    frame_state.ip = InstructionOffset(ip);
                    frame_state.pending_operation = Some(PendingFrameOperation::Comparison {
                        comparison: ResumableComparison::new(
                            ResumableComparisonKind::Greater,
                            frame.read(*lhs)?,
                            frame.read(*rhs)?,
                        ),
                        destination: *dst,
                        returned: None,
                        source_span: instruction.span,
                    });
                    return Ok(FrameDriveOutcome::Continue);
                }
                InstructionKind::GreaterEqual { dst, lhs, rhs } => {
                    frame_state.ip = InstructionOffset(ip);
                    frame_state.pending_operation = Some(PendingFrameOperation::Comparison {
                        comparison: ResumableComparison::new(
                            ResumableComparisonKind::GreaterEqual,
                            frame.read(*lhs)?,
                            frame.read(*rhs)?,
                        ),
                        destination: *dst,
                        returned: None,
                        source_span: instruction.span,
                    });
                    return Ok(FrameDriveOutcome::Continue);
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
                    let mut guard_context = runtime_type_guards::GuardExecutionContext::new(
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                    );
                    runtime_type_guards::execute_linked_register_guard(
                        code,
                        program,
                        frame,
                        *src,
                        *guard,
                        &mut guard_context,
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
                InstructionKind::AwaitCall { .. } => {
                    unreachable!("linked bytecode verification rejects nested await operations")
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
                        host,
                        heap,
                        budget,
                        frame,
                        native_function_calls::LinkedNativeFunctionCall {
                            program,
                            dst: *dst,
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
                    let target_code = program.function(*function).ok_or_else(|| {
                        VmError::new(VmErrorKind::UnknownFunction {
                            name: program.debug_name(*debug_name).to_owned(),
                        })
                        .with_source_span_if_absent(instruction.span)
                    })?;
                    if target_code.asyncness.is_async() && await_resume.is_none() {
                        return Err(VmError::new(VmErrorKind::AsyncCallRequiresAwait {
                            name: program.debug_name(*debug_name).to_owned(),
                        })
                        .with_source_span_if_absent(instruction.span));
                    }
                    let args =
                        script_function_calls::script_call_args_from_call_arguments(frame, args)?
                            .as_slice()
                            .to_vec();
                    frame_state.ip = await_resume.unwrap_or(InstructionOffset(ip));
                    return Ok(FrameDriveOutcome::Push(PendingLinkedCall {
                        owner: Arc::clone(&current_owner),
                        function: *function,
                        captures: Vec::new(),
                        args,
                        check_param_guards: matches!(mode, vela_bytecode::ScriptCallMode::Checked),
                        call_site: instruction.span,
                        call_site_offset: Some(instruction_offset),
                        inline_caches: call.inline_caches,
                        bytecode_profiler: call.bytecode_profiler,
                        return_target: PendingReturnTarget::Register(*dst),
                    }));
                }
                InstructionKind::MakeClosure {
                    dst,
                    function,
                    captures,
                } => {
                    closure_calls::make_linked_closure(
                        heap,
                        budget,
                        frame,
                        closure_calls::LinkedMakeClosure {
                            dst: *dst,
                            function: *function,
                            captures,
                            call_site: instruction.span,
                        },
                    )?;
                }
                InstructionKind::CallClosure { dst, callee, args } => {
                    let (owner, function, captures) = {
                        let closure = crate::runtime_checks::expect_closure_ref(
                            &frame.read(*callee)?,
                            heap.as_deref(),
                            "closure call",
                        )?;
                        (
                            Arc::clone(&closure.owner),
                            closure.function,
                            closure.captures.as_slice().to_vec(),
                        )
                    };
                    let target_code = owner.function(function).ok_or_else(|| {
                        VmError::new(VmErrorKind::UnknownFunction {
                            name: format!("<linked closure#{}>", function.index()),
                        })
                        .with_source_span_if_absent(instruction.span)
                    })?;
                    if target_code.asyncness.is_async() && await_resume.is_none() {
                        return Err(VmError::new(VmErrorKind::AsyncCallRequiresAwait {
                            name: owner
                                .program()
                                .debug_name(target_code.debug_name)
                                .to_owned(),
                        })
                        .with_source_span_if_absent(instruction.span));
                    }
                    let values = args
                        .iter()
                        .map(|register| frame.read(*register))
                        .collect::<VmResult<Vec<_>>>()?;
                    let inline_caches = if owner.generation() == program.generation() {
                        call.inline_caches
                    } else {
                        call.inline_caches
                            .and_then(|caches| caches.for_generation(owner.generation()))
                    };
                    let bytecode_profiler = if owner.generation() == program.generation() {
                        call.bytecode_profiler
                    } else {
                        call.bytecode_profiler
                            .and_then(|profiler| profiler.for_generation(owner.generation()))
                    };
                    frame_state.ip = await_resume.unwrap_or(InstructionOffset(ip));
                    return Ok(FrameDriveOutcome::Push(PendingLinkedCall {
                        owner,
                        function,
                        captures,
                        args: values,
                        check_param_guards: true,
                        call_site: instruction.span,
                        call_site_offset: Some(instruction_offset),
                        inline_caches,
                        bytecode_profiler,
                        return_target: PendingReturnTarget::Register(*dst),
                    }));
                }
                InstructionKind::CallMethod {
                    dst,
                    receiver,
                    dispatch,
                    debug_name,
                    cache_site,
                    args,
                } => {
                    let dispatch_target = program.method_dispatch(*dispatch).ok_or_else(|| {
                        VmError::new(VmErrorKind::UnknownMethod {
                            method: program.debug_name(*debug_name).to_owned(),
                        })
                        .with_source_span_if_absent(instruction.span)
                    })?;
                    if let vela_bytecode::linked::LinkedMethodDispatchKind::Value { method_id } =
                        &dispatch_target.kind
                    {
                        let receiver_value = frame.read(*receiver)?;
                        if let Some(callback_cache) = callback_method_dispatch::callback_cache_entry(
                            *method_id,
                            &receiver_value,
                            heap.as_deref(),
                        ) {
                            let values =
                                script_function_calls::script_call_args_from_call_arguments(
                                    frame, args,
                                )?;
                            if let Some(callback) = ResumableCallbackMethod::new(
                                &receiver_value,
                                callback_cache,
                                values.as_slice(),
                                heap.as_deref(),
                            ) {
                                let callback = callback?;
                                if let (Some(site), Some(caches)) =
                                    (*cache_site, call.inline_caches)
                                {
                                    let existing = caches.method_dispatch(site);
                                    let generic_valid = existing.is_some_and(|entry| {
                                        entry.dispatch == *dispatch
                                            && entry.debug_name == dispatch_target.debug_name
                                            && matches!(
                                                entry.target,
                                                crate::MethodInlineCacheTarget::Value {
                                                    method_id: cached_method,
                                                    ..
                                                } | crate::MethodInlineCacheTarget::CallbackValue {
                                                    method_id: cached_method,
                                                    ..
                                                } if cached_method == *method_id
                                            )
                                    });
                                    let cached = existing.is_some_and(|entry| {
                                        entry.dispatch == *dispatch
                                            && entry.debug_name == dispatch_target.debug_name
                                            && matches!(
                                                entry.target,
                                                crate::MethodInlineCacheTarget::CallbackValue {
                                                    method_id: cached_method,
                                                    callback_method,
                                                } if cached_method == *method_id
                                                    && callback_method == callback_cache
                                            )
                                    });
                                    if !cached {
                                        if !generic_valid {
                                            caches.set_method_dispatch(
                                                site,
                                                crate::MethodInlineCacheEntry {
                                                    dispatch: *dispatch,
                                                    debug_name: dispatch_target.debug_name,
                                                    target: crate::MethodInlineCacheTarget::Value {
                                                        method_id: *method_id,
                                                        standard_method: None,
                                                    },
                                                },
                                            );
                                        }
                                        caches.set_method_dispatch(
                                            site,
                                            crate::MethodInlineCacheEntry {
                                                dispatch: *dispatch,
                                                debug_name: dispatch_target.debug_name,
                                                target:
                                                    crate::MethodInlineCacheTarget::CallbackValue {
                                                        method_id: *method_id,
                                                        callback_method: callback_cache,
                                                    },
                                            },
                                        );
                                    }
                                }
                                frame_state.ip = await_resume.unwrap_or(InstructionOffset(ip));
                                frame_state.pending_operation =
                                    Some(PendingFrameOperation::CallbackMethod {
                                        callback,
                                        destination: *dst,
                                        returned: None,
                                        source_span: instruction.span,
                                    });
                                return Ok(FrameDriveOutcome::Continue);
                            }
                        }
                        if let Some(kind) = array_methods::resumable_ordering_kind(
                            &receiver_value,
                            *method_id,
                            heap.as_deref(),
                        ) {
                            let values =
                                script_function_calls::script_call_args_from_call_arguments(
                                    frame, args,
                                )?;
                            let ordering = array_methods::ResumableArrayOrdering::new(
                                kind,
                                &receiver_value,
                                values.as_slice(),
                                heap.as_deref(),
                            )?;
                            if let (Some(site), Some(caches)) = (*cache_site, call.inline_caches) {
                                let existing = caches.method_dispatch(site);
                                let cached = existing.is_some_and(|entry| {
                                    entry.dispatch == *dispatch
                                        && entry.debug_name == dispatch_target.debug_name
                                        && matches!(
                                            entry.target,
                                            crate::MethodInlineCacheTarget::Value {
                                                method_id: cached_method,
                                                ..
                                            } if cached_method == *method_id
                                        )
                                });
                                if !cached {
                                    if existing.is_none() {
                                        caches.set_method_dispatch(
                                            site,
                                            crate::MethodInlineCacheEntry {
                                                dispatch: *dispatch,
                                                debug_name: dispatch_target.debug_name,
                                                target: crate::MethodInlineCacheTarget::Value {
                                                    method_id: *method_id,
                                                    standard_method: None,
                                                },
                                            },
                                        );
                                    }
                                    caches.set_method_dispatch(
                                        site,
                                        crate::MethodInlineCacheEntry {
                                            dispatch: *dispatch,
                                            debug_name: dispatch_target.debug_name,
                                            target: crate::MethodInlineCacheTarget::Value {
                                                method_id: *method_id,
                                                standard_method:
                                                    script_builtin_methods::standard_cache_entry(
                                                        *method_id,
                                                        &receiver_value,
                                                        heap.as_deref(),
                                                    ),
                                            },
                                        },
                                    );
                                }
                            }
                            frame_state.ip = await_resume.unwrap_or(InstructionOffset(ip));
                            frame_state.pending_operation =
                                Some(PendingFrameOperation::ArrayOrdering {
                                    ordering,
                                    destination: *dst,
                                    returned: None,
                                    source_span: instruction.span,
                                });
                            return Ok(FrameDriveOutcome::Continue);
                        }
                    }
                    if let vela_bytecode::linked::LinkedMethodDispatchKind::Script {
                        method_id,
                        function,
                    } = &dispatch_target.kind
                    {
                        let target_code = program.function(*function).ok_or_else(|| {
                            VmError::new(VmErrorKind::UnknownMethod {
                                method: program.debug_name(dispatch_target.debug_name).to_owned(),
                            })
                            .with_source_span_if_absent(instruction.span)
                        })?;
                        if target_code.asyncness.is_async() && await_resume.is_none() {
                            return Err(VmError::new(VmErrorKind::AsyncCallRequiresAwait {
                                name: program.debug_name(dispatch_target.debug_name).to_owned(),
                            })
                            .with_source_span_if_absent(instruction.span));
                        }
                        if let (Some(site), Some(caches)) = (*cache_site, call.inline_caches) {
                            caches.set_method_dispatch(
                                site,
                                crate::MethodInlineCacheEntry {
                                    dispatch: *dispatch,
                                    debug_name: dispatch_target.debug_name,
                                    target: crate::MethodInlineCacheTarget::Script {
                                        method_id: *method_id,
                                        function: *function,
                                    },
                                },
                            );
                        }
                        let mut values = Vec::with_capacity(args.len().saturating_add(1));
                        values.push(frame.read(*receiver)?);
                        values.extend_from_slice(
                            script_function_calls::script_call_args_from_call_arguments(
                                frame, args,
                            )?
                            .as_slice(),
                        );
                        frame_state.ip = await_resume.unwrap_or(InstructionOffset(ip));
                        return Ok(FrameDriveOutcome::Push(PendingLinkedCall {
                            owner: Arc::clone(&current_owner),
                            function: *function,
                            captures: Vec::new(),
                            args: values,
                            check_param_guards: true,
                            call_site: instruction.span,
                            call_site_offset: Some(instruction_offset),
                            inline_caches: call.inline_caches,
                            bytecode_profiler: call.bytecode_profiler,
                            return_target: PendingReturnTarget::Register(*dst),
                        }));
                    }
                    script_method_calls::dispatch_linked_method_call(
                        self,
                        script_method_calls::LinkedScriptMethodCallContext {
                            program,
                            inline_caches: call.inline_caches,
                            cache_site: *cache_site,
                            call_site: instruction.span,
                            bytecode_profiler: call.bytecode_profiler,
                        },
                        host,
                        heap,
                        budget,
                        frame,
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
                    let dynamic_call = script_method_calls::LinkedDynamicMethodCall {
                        dst: *dst,
                        receiver: *receiver,
                        method_name: *method_name,
                        args,
                    };
                    let context = script_method_calls::LinkedScriptMethodCallContext {
                        program,
                        inline_caches: call.inline_caches,
                        cache_site: *cache_site,
                        call_site: instruction.span,
                        bytecode_profiler: call.bytecode_profiler,
                    };
                    let resolution = script_method_calls::resolve_linked_dynamic_script_target(
                        self,
                        &context,
                        host.as_deref(),
                        heap.as_deref(),
                        frame,
                        &dynamic_call,
                    )?;
                    if let script_method_calls::LinkedDynamicResolution::Script(target) = resolution
                    {
                        let target_code = program.function(target.function).ok_or_else(|| {
                            VmError::new(VmErrorKind::UnknownMethod {
                                method: program.debug_name(*method_name).to_owned(),
                            })
                            .with_source_span_if_absent(instruction.span)
                        })?;
                        if target_code.asyncness.is_async() && await_resume.is_none() {
                            return Err(VmError::new(VmErrorKind::AsyncCallRequiresAwait {
                                name: program.debug_name(*method_name).to_owned(),
                            })
                            .with_source_span_if_absent(instruction.span));
                        }
                        let mut values = Vec::with_capacity(target.args.len().saturating_add(1));
                        values.push(frame.read(*receiver)?);
                        values.extend_from_slice(
                            script_function_calls::script_call_args_from_call_arguments(
                                frame,
                                &target.args,
                            )?
                            .as_slice(),
                        );
                        frame_state.ip = await_resume.unwrap_or(InstructionOffset(ip));
                        return Ok(FrameDriveOutcome::Push(PendingLinkedCall {
                            owner: Arc::clone(&current_owner),
                            function: target.function,
                            captures: Vec::new(),
                            args: values,
                            check_param_guards: true,
                            call_site: instruction.span,
                            call_site_offset: Some(instruction_offset),
                            inline_caches: call.inline_caches,
                            bytecode_profiler: call.bytecode_profiler,
                            return_target: PendingReturnTarget::Register(*dst),
                        }));
                    }
                    if let script_method_calls::LinkedDynamicResolution::Other(
                        script_method_calls::LinkedDynamicNonScriptTarget::StandardValue {
                            method_id,
                            ..
                        },
                    ) = &resolution
                    {
                        let receiver_value = frame.read(*receiver)?;
                        if let Some(callback_cache) = callback_method_dispatch::callback_cache_entry(
                            *method_id,
                            &receiver_value,
                            heap.as_deref(),
                        ) {
                            let values =
                                script_method_calls::dynamic_value_args_from_linked_arguments(
                                    frame, args,
                                )?;
                            if let Some(callback) = ResumableCallbackMethod::new(
                                &receiver_value,
                                callback_cache,
                                values.as_slice(),
                                heap.as_deref(),
                            ) {
                                frame_state.ip = await_resume.unwrap_or(InstructionOffset(ip));
                                frame_state.pending_operation =
                                    Some(PendingFrameOperation::CallbackMethod {
                                        callback: callback?,
                                        destination: *dst,
                                        returned: None,
                                        source_span: instruction.span,
                                    });
                                return Ok(FrameDriveOutcome::Continue);
                            }
                        }
                        if let Some(kind) = array_methods::resumable_ordering_kind(
                            &receiver_value,
                            *method_id,
                            heap.as_deref(),
                        ) {
                            let values =
                                script_method_calls::dynamic_value_args_from_linked_arguments(
                                    frame, args,
                                )?;
                            let ordering = array_methods::ResumableArrayOrdering::new(
                                kind,
                                &receiver_value,
                                values.as_slice(),
                                heap.as_deref(),
                            )?;
                            frame_state.ip = await_resume.unwrap_or(InstructionOffset(ip));
                            frame_state.pending_operation =
                                Some(PendingFrameOperation::ArrayOrdering {
                                    ordering,
                                    destination: *dst,
                                    returned: None,
                                    source_span: instruction.span,
                                });
                            return Ok(FrameDriveOutcome::Continue);
                        }
                    }
                    let script_method_calls::LinkedDynamicResolution::Other(target) = resolution
                    else {
                        unreachable!("script dynamic targets return through a frame push")
                    };
                    script_method_calls::dispatch_resolved_linked_dynamic_method_call(
                        self,
                        context,
                        host,
                        heap,
                        budget,
                        frame,
                        dynamic_call,
                        target,
                    )?;
                }
                InstructionKind::TryPropagate { dst, src, expected } => {
                    if let Some(value) = try_propagation::dispatch_try_propagate(
                        frame,
                        heap.as_deref(),
                        *dst,
                        *src,
                        *expected,
                    )? {
                        let mut guard_context = runtime_type_guards::GuardExecutionContext::new(
                            heap.as_deref_mut(),
                            budget.as_deref_mut(),
                        );
                        return runtime_type_guards::execute_linked_return_guard(
                            code,
                            program,
                            value,
                            &mut guard_context,
                        )
                        .map(FrameDriveOutcome::Return);
                    }
                }
                InstructionKind::MakeArray { dst, elements } => {
                    script_aggregate_construction::make_array(
                        frame,
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                        *dst,
                        elements,
                    )?;
                }
                InstructionKind::MakeTuple { dst, elements } => {
                    script_aggregate_construction::make_tuple(
                        frame,
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                        *dst,
                        elements,
                    )?;
                }
                InstructionKind::MakeSetFromArray { dst, src } => {
                    script_aggregate_construction::make_set_from_array(
                        frame,
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                        *dst,
                        *src,
                    )?;
                }
                InstructionKind::FormatString { dst, parts } => {
                    format_strings::make_format_string(
                        frame,
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                        *dst,
                        &code.constants,
                        parts,
                        instruction.span,
                    )?;
                }
                InstructionKind::MakeMap { dst, entries } => {
                    script_aggregate_construction::make_linked_map(
                        frame,
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
                        frame, *dst, *start, *end, *inclusive,
                    )?;
                }
                InstructionKind::MakeRecord { dst, ty, fields } => {
                    script_object_construction::make_linked_record(
                        frame,
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                        *dst,
                        program,
                        *ty,
                        fields,
                    )?;
                }
                InstructionKind::GetRecordField {
                    dst,
                    record,
                    debug_name,
                } => {
                    field_access::dispatch_get_record_field(
                        frame,
                        heap.as_deref_mut(),
                        *dst,
                        *record,
                        program.debug_name(*debug_name),
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
                        frame,
                        heap.as_deref_mut(),
                        program,
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
                InstructionKind::SetRecordField {
                    record,
                    debug_name,
                    src,
                } => {
                    field_access::dispatch_set_record_field(
                        frame,
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                        *record,
                        program.debug_name(*debug_name),
                        *src,
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
                        frame,
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                        program,
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
                        frame,
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                        *dst,
                        program,
                        script_object_construction::LinkedEnumConstruction {
                            enum_ty: *enum_ty,
                            variant: *variant,
                            fields,
                        },
                    )?;
                }
                InstructionKind::GetEnumField {
                    dst,
                    value,
                    debug_name,
                } => {
                    field_access::dispatch_get_enum_field(
                        frame,
                        heap.as_deref_mut(),
                        *dst,
                        *value,
                        program.debug_name(*debug_name),
                    )?;
                }
                InstructionKind::GetEnumSlot {
                    dst,
                    value,
                    field,
                    debug_name,
                } => {
                    field_access::dispatch_linked_get_enum_slot(
                        frame,
                        heap.as_deref_mut(),
                        program,
                        *dst,
                        *value,
                        *field,
                        *debug_name,
                    )?;
                }
                InstructionKind::TupleArityEqual { dst, value, arity } => {
                    let matched = tuple_fields::tuple_arity_equal(
                        &frame.read(*value)?,
                        heap.as_deref(),
                        *arity,
                    )?;
                    frame.write(*dst, Value::Bool(matched))?;
                }
                InstructionKind::GuardTupleArity { value, arity } => {
                    tuple_fields::guard_tuple_arity(&frame.read(*value)?, heap.as_deref(), *arity)?;
                }
                InstructionKind::GetTupleField { dst, value, index } => {
                    let field = tuple_fields::get_tuple_field(
                        &frame.read(*value)?,
                        heap.as_deref(),
                        *index,
                    )?;
                    frame.write(*dst, field)?;
                }
                InstructionKind::GetIndex { dst, base, index } => {
                    indexing::dispatch_get_index(frame, heap.as_deref(), *dst, *base, *index)?;
                }
                InstructionKind::GetStringKeyIndex { dst, base, key } => {
                    let key =
                        string_key_constant(code.constants.get(key.0), key.0, instruction.span)?;
                    indexing::dispatch_get_string_key_index(
                        frame,
                        heap.as_deref(),
                        *dst,
                        *base,
                        key,
                    )?;
                }
                InstructionKind::SetIndex { base, index, src } => {
                    indexing::dispatch_set_index(
                        frame,
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
                        frame,
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
                            vm: self,
                            program,
                            host: host.as_deref_mut(),
                            frame,
                            heap: heap.as_deref_mut(),
                            budget: budget.as_deref_mut(),
                            inline_caches: call.inline_caches,
                            bytecode_profiler: call.bytecode_profiler,
                        },
                        *dst,
                        *iterable,
                    )
                    .map_err(|error| error.with_source_span_if_absent(instruction.span))?;
                }
                InstructionKind::IterNext {
                    iterator,
                    dst,
                    jump_if_done,
                } => {
                    if let Some(target) = iteration::dispatch_linked_iter_next(
                        iteration::IterRuntime {
                            vm: self,
                            program,
                            host: host.as_deref_mut(),
                            frame,
                            heap: heap.as_deref_mut(),
                            budget: budget.as_deref_mut(),
                            inline_caches: call.inline_caches,
                            bytecode_profiler: call.bytecode_profiler,
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
                            vm: self,
                            program,
                            host: host.as_deref_mut(),
                            frame,
                            heap: heap.as_deref_mut(),
                            budget: budget.as_deref_mut(),
                            inline_caches: call.inline_caches,
                            bytecode_profiler: call.bytecode_profiler,
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
                        frame,
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
                        frame,
                        heap.as_deref(),
                        program,
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
                            frame,
                            heap: heap.as_deref_mut(),
                            budget: budget.as_deref_mut(),
                            host: host.as_deref_mut(),
                            inline_caches: call.inline_caches,
                            source_span: instruction.span,
                        },
                        program,
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
                            frame,
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
                            frame,
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
                            frame,
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
                            frame,
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
                            frame,
                            heap: heap.as_deref_mut(),
                            budget: budget.as_deref_mut(),
                            host: host.as_deref_mut(),
                            inline_caches: call.inline_caches,
                            source_span: instruction.span,
                        },
                        *root,
                        host_access::LinkedCodeHostCallPlan {
                            program,
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
                    let mut guard_context = runtime_type_guards::GuardExecutionContext::new(
                        heap.as_deref_mut(),
                        budget.as_deref_mut(),
                    );
                    return runtime_type_guards::execute_linked_return_guard(
                        code,
                        program,
                        frame.read(*src)?,
                        &mut guard_context,
                    )
                    .map(FrameDriveOutcome::Return);
                }
            }

            if let Some(resume) = await_resume {
                debug_assert!(resume.0 <= code.instructions.len());
                ip = resume.0;
            }

            if let Some(heap) = heap.as_deref_mut()
                && heap.needs_safe_point()
            {
                heap.collect_frame_at_safe_point(frame, budget.as_deref_mut());
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
