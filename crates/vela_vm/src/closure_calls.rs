use std::sync::Arc;

use vela_bytecode::{
    FunctionIndex, InstructionOffset, Register, UnlinkedCodeObject, UnlinkedProgramCode,
};
use vela_common::Span;

use crate::heap::HeapValue;
use crate::runtime_checks::expect_closure_ref;
use crate::value::{ClosureCode, ClosureValue};
use crate::{
    CallFrame, ExecutionBudget, ExecutionCall, HeapExecution, HostExecution, SmallStorage, Value,
    Vm, VmError, VmErrorKind, VmResult, allocate_heap_value, store_value_in_heap_if_needed,
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

#[inline]
fn script_call_args_from_registers(
    frame: &CallFrame,
    registers: &[Register],
) -> VmResult<SmallStorage<Value>> {
    SmallStorage::try_from_slice_map(registers, 4, |register| Ok(*frame.read(*register)?))
}
