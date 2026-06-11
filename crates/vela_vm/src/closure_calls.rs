use std::sync::Arc;

use vela_bytecode::{
    FunctionIndex, InstructionOffset, LinkedProgram, Register, UnlinkedCodeObject,
    UnlinkedProgramCode,
};
use vela_common::Span;

use crate::heap::HeapValue;
use crate::linked_execution::LinkedExecutionCall;
use crate::runtime_checks::expect_closure_ref;
use crate::value::{ClosureCode, ClosureValue};
use crate::{
    CallFrame, ExecutionBudget, ExecutionCall, HeapExecution, HostExecution, SmallStorage, Value,
    Vm, VmError, VmErrorKind, VmInlineCaches, VmResult, allocate_heap_value,
    store_value_in_heap_if_needed,
};

pub(crate) struct MakeClosure<'a> {
    pub(crate) dst: Register,
    pub(crate) program: Option<&'a dyn UnlinkedProgramCode>,
    pub(crate) owner: &'a UnlinkedCodeObject,
    pub(crate) function: FunctionIndex,
    pub(crate) captures: &'a [Register],
}

pub(crate) fn make_closure(
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    frame: &mut CallFrame,
    closure: MakeClosure<'_>,
) -> VmResult<()> {
    let captures = closure
        .captures
        .iter()
        .map(|register| frame.read(*register).cloned())
        .collect::<VmResult<Vec<_>>>()?;
    let heap = heap.as_deref_mut().ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "closure heap",
        })
    })?;
    let code = resolve_closure_code(closure.program, closure.owner, closure.function).ok_or_else(
        || {
            VmError::new(VmErrorKind::UnknownFunction {
                name: format!("{}::<closure#{}>", closure.owner.name, closure.function.0),
            })
        },
    )?;
    let value = allocate_heap_value(
        HeapValue::Closure(ClosureValue {
            code: ClosureCode::Unlinked(Arc::new(code.clone())),
            captures,
        }),
        heap,
        budget.as_deref_mut(),
    )?;
    frame.write(closure.dst, value)
}

fn resolve_closure_code<'a>(
    program: Option<&'a dyn UnlinkedProgramCode>,
    owner: &'a UnlinkedCodeObject,
    function: FunctionIndex,
) -> Option<&'a UnlinkedCodeObject> {
    program
        .and_then(|program| program.function_by_index(function))
        .or_else(|| owner.nested_function(function))
}

pub(crate) struct ClosureCall<'a> {
    pub(crate) dst: Register,
    pub(crate) callee: Register,
    pub(crate) args: &'a [Register],
    pub(crate) call_site: Option<Span>,
    pub(crate) call_site_offset: InstructionOffset,
}

pub(crate) fn dispatch_closure_call(
    vm: &Vm,
    program: Option<&dyn UnlinkedProgramCode>,
    host: &mut Option<&mut HostExecution<'_>>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    frame: &mut CallFrame,
    call: ClosureCall<'_>,
) -> VmResult<()> {
    let (code, captures) = {
        let closure =
            expect_closure_ref(frame.read(call.callee)?, heap.as_deref(), "closure call")?;
        let ClosureCode::Unlinked(code) = &closure.code else {
            return Err(VmError::new(VmErrorKind::TypeMismatch {
                operation: "closure call",
            }));
        };
        let captures = SmallStorage::try_from_slice_map(&closure.captures, 4, |value| {
            Ok::<_, VmError>(*value)
        })?;
        (Arc::clone(code), captures)
    };
    let values = script_call_args_from_registers(frame, call.args)?;
    let protected_root_len = heap.as_deref_mut().map(|heap| heap.push_frame_roots(frame));
    let result = vm.execute_call(
        ExecutionCall {
            code: &code,
            program,
            captures: captures.as_slice(),
            args: values.as_slice(),
            check_param_guards: true,
            call_site: call.call_site,
            call_site_offset: Some(call.call_site_offset),
            inline_caches: None,
        },
        host.as_deref_mut(),
        heap.as_deref_mut(),
        budget.as_deref_mut(),
    );
    if let (Some(heap), Some(protected_root_len)) = (heap.as_deref_mut(), protected_root_len) {
        heap.truncate_protected_roots(protected_root_len);
    }
    let result =
        store_value_in_heap_if_needed(result?, heap.as_deref_mut(), budget.as_deref_mut())?;
    frame.write(call.dst, result)
}

pub(crate) struct LinkedClosureCallContext<'a> {
    pub(crate) program: &'a LinkedProgram,
    pub(crate) inline_caches: Option<&'a dyn VmInlineCaches>,
    pub(crate) call_site: Option<Span>,
    pub(crate) call_site_offset: InstructionOffset,
}

pub(crate) struct LinkedClosureCall<'a> {
    pub(crate) dst: Register,
    pub(crate) callee: Register,
    pub(crate) args: &'a [Register],
}

pub(crate) fn dispatch_linked_closure_call(
    vm: &Vm,
    context: LinkedClosureCallContext<'_>,
    host: &mut Option<&mut HostExecution<'_>>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    frame: &mut CallFrame,
    call: LinkedClosureCall<'_>,
) -> VmResult<()> {
    let (function, captures) = {
        let closure =
            expect_closure_ref(frame.read(call.callee)?, heap.as_deref(), "closure call")?;
        let ClosureCode::Linked(function) = &closure.code else {
            return Err(VmError::new(VmErrorKind::TypeMismatch {
                operation: "closure call",
            })
            .with_source_span_if_absent(context.call_site));
        };
        let function = *function;
        let captures = SmallStorage::try_from_slice_map(&closure.captures, 4, |value| {
            Ok::<_, VmError>(*value)
        })?;
        (function, captures)
    };
    let function_code = context.program.function(function).ok_or_else(|| {
        VmError::new(VmErrorKind::UnknownFunction {
            name: format!("<linked closure#{}>", function.index()),
        })
        .with_source_span_if_absent(context.call_site)
    })?;
    let values = script_call_args_from_registers(frame, call.args)?;
    let protected_root_len = heap.as_deref_mut().map(|heap| heap.push_frame_roots(frame));
    let result = vm.execute_linked_call(
        LinkedExecutionCall {
            code: function_code,
            program: context.program,
            captures: captures.as_slice(),
            args: values.as_slice(),
            check_param_guards: true,
            call_site: context.call_site,
            call_site_offset: Some(context.call_site_offset),
            inline_caches: context.inline_caches,
        },
        host.as_deref_mut(),
        heap.as_deref_mut(),
        budget.as_deref_mut(),
    );
    if let (Some(heap), Some(protected_root_len)) = (heap.as_deref_mut(), protected_root_len) {
        heap.truncate_protected_roots(protected_root_len);
    }
    let result =
        store_value_in_heap_if_needed(result?, heap.as_deref_mut(), budget.as_deref_mut())?;
    frame.write(call.dst, result)
}

#[inline]
fn script_call_args_from_registers(
    frame: &CallFrame,
    registers: &[Register],
) -> VmResult<SmallStorage<Value>> {
    SmallStorage::try_from_slice_map(registers, 4, |register| Ok(*frame.read(*register)?))
}
