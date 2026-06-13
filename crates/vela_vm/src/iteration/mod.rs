mod methods;
mod source;
mod state;
mod step;

pub(crate) use methods::{
    all_method, any_method, chars_method, collect_array_method, collect_array_method_runtime,
    collect_values, count_method, count_method_runtime, filter_method, find_method, is_iterator,
    iter_method, map_method, next_method, next_method_runtime, skip_method, string_bytes_method,
    take_method,
};
pub(crate) use methods::{
    callback_all, callback_all_over, callback_any, callback_any_over, callback_count,
    callback_count_over, callback_find, callback_find_over,
};
pub(crate) use source::make_iterator;
pub use state::IteratorState;
pub(crate) use step::{
    RangeNextStep, dispatch_i64_range_next, dispatch_linked_i64_range_next,
    dispatch_linked_range_next, dispatch_range_next,
};

use crate::heap::HeapValue;
use crate::heap_values::allocate_heap_value;
use crate::method_runtime::{CallerRoots, MethodRuntime};
use crate::{
    CallFrame, ExecutionBudget, HeapExecution, HostExecution, Value, Vm, VmBytecodeProfiler,
    VmError, VmErrorKind, VmInlineCaches, VmResult,
};
use vela_bytecode::{
    InstructionOffset, LinkedCodeObject, LinkedProgram, Register, UnlinkedCodeObject,
    UnlinkedProgramCode,
};

pub(crate) struct IterRuntime<'a, 'host, 'heap> {
    pub(crate) vm: &'a Vm,
    pub(crate) program: Option<&'a dyn UnlinkedProgramCode>,
    pub(crate) linked_program: Option<&'a LinkedProgram>,
    pub(crate) host: Option<&'a mut HostExecution<'host>>,
    pub(crate) frame: &'a mut CallFrame,
    pub(crate) heap: Option<&'a mut HeapExecution<'heap>>,
    pub(crate) budget: Option<&'a mut ExecutionBudget>,
    pub(crate) inline_caches: Option<&'a dyn VmInlineCaches>,
    pub(crate) bytecode_profiler: Option<&'a dyn VmBytecodeProfiler>,
}

pub(crate) fn dispatch_iter_init(
    mut runtime: IterRuntime<'_, '_, '_>,
    dst: Register,
    iterable: Register,
) -> VmResult<()> {
    let iterable = runtime.frame.read(iterable)?;
    let iterator = if is_iterator(&iterable, runtime.heap.as_deref()) {
        methods::take_iterator_from_heap(&iterable, &mut runtime.heap, "for in")?
    } else {
        make_iterator(&iterable, runtime.heap.as_deref())?
    };
    let Some(heap) = heap_ref(&mut runtime.heap) else {
        return Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "iterator heap",
        }));
    };
    let value = allocate_heap_value(
        HeapValue::Iterator(iterator),
        heap,
        budget_ref(&mut runtime.budget),
    )?;
    runtime.frame.write(dst, value)
}

pub(crate) fn dispatch_iter_next(
    mut runtime: IterRuntime<'_, '_, '_>,
    code: &UnlinkedCodeObject,
    iterator: Register,
    dst: Register,
    jump_if_done: InstructionOffset,
) -> VmResult<Option<usize>> {
    let next = next_iterator_value(&mut runtime, iterator)?;
    match next {
        Some(value) => {
            runtime.frame.write(dst, value)?;
            Ok(None)
        }
        None => {
            crate::runtime_checks::validate_jump(code, jump_if_done.0)?;
            Ok(Some(jump_if_done.0))
        }
    }
}

pub(crate) fn dispatch_linked_iter_next(
    mut runtime: IterRuntime<'_, '_, '_>,
    code: &LinkedCodeObject,
    iterator: Register,
    dst: Register,
    jump_if_done: InstructionOffset,
) -> VmResult<Option<usize>> {
    let next = next_iterator_value(&mut runtime, iterator)?;
    match next {
        Some(value) => {
            runtime.frame.write(dst, value)?;
            Ok(None)
        }
        None => {
            debug_assert!(jump_if_done.0 <= code.instructions.len());
            Ok(Some(jump_if_done.0))
        }
    }
}

fn next_iterator_value(
    runtime: &mut IterRuntime<'_, '_, '_>,
    iterator: Register,
) -> VmResult<Option<Value>> {
    let receiver = runtime.frame.read(iterator)?;
    let mut iterator_state =
        methods::take_iterator_from_heap(&receiver, &mut runtime.heap, "iterator")?;
    let caller_roots = CallerRoots::for_frame(runtime.frame, runtime.heap.as_deref());
    let result = {
        let mut method_runtime = MethodRuntime {
            vm: runtime.vm,
            program: runtime.program,
            linked_program: runtime.linked_program,
            host: runtime.host.as_deref_mut(),
            heap: runtime.heap.as_deref_mut(),
            budget: runtime.budget.as_deref_mut(),
            caller_roots,
            inline_caches: runtime.inline_caches,
            bytecode_profiler: runtime.bytecode_profiler,
        };
        iterator_state.next_with_runtime(&mut method_runtime, "iterator", &[])
    };
    methods::restore_iterator_to_heap(receiver, &mut runtime.heap, iterator_state, "iterator")?;
    result
}

#[inline]
pub(super) fn heap_ref<'a, 'heap>(
    heap: &'a mut Option<&mut HeapExecution<'heap>>,
) -> Option<&'a mut HeapExecution<'heap>> {
    match heap {
        Some(heap) => Some(&mut **heap),
        None => None,
    }
}

#[inline]
fn budget_ref<'a>(budget: &'a mut Option<&mut ExecutionBudget>) -> Option<&'a mut ExecutionBudget> {
    match budget {
        Some(budget) => Some(&mut **budget),
        None => None,
    }
}

pub(super) fn allocate_iterator(
    iterator: IteratorState,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<Value> {
    let Some(heap) = heap.as_deref_mut() else {
        return Err(VmError::new(VmErrorKind::TypeMismatch { operation }));
    };
    allocate_heap_value(HeapValue::Iterator(iterator), heap, budget.as_deref_mut())
}
