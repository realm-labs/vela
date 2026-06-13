mod methods;
mod source;
mod state;
mod step;

pub(crate) use methods::{
    chars_method, collect_array_method, count_method, is_iterator, iter_method, next_method,
    string_bytes_method,
};
pub(crate) use source::make_iterator;
pub use state::IteratorState;
pub(crate) use step::{
    RangeNextStep, dispatch_i64_range_next, dispatch_linked_i64_range_next,
    dispatch_linked_range_next, dispatch_range_next,
};

use crate::heap::HeapValue;
use crate::heap_values::allocate_heap_value;
use crate::{CallFrame, ExecutionBudget, HeapExecution, Value, VmError, VmErrorKind, VmResult};
use vela_bytecode::{InstructionOffset, LinkedCodeObject, Register, UnlinkedCodeObject};

pub(crate) struct IterRuntime<'a, 'heap> {
    pub(crate) frame: &'a mut CallFrame,
    pub(crate) heap: Option<&'a mut HeapExecution<'heap>>,
    pub(crate) budget: Option<&'a mut ExecutionBudget>,
}

pub(crate) fn dispatch_iter_init(
    mut runtime: IterRuntime<'_, '_>,
    dst: Register,
    iterable: Register,
) -> VmResult<()> {
    let iterator = make_iterator(&runtime.frame.read(iterable)?, runtime.heap.as_deref())?;
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
    mut runtime: IterRuntime<'_, '_>,
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
    mut runtime: IterRuntime<'_, '_>,
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
    runtime: &mut IterRuntime<'_, '_>,
    iterator: Register,
) -> VmResult<Option<Value>> {
    let value = runtime.frame.read(iterator)?;
    match value {
        Value::HeapRef(reference) => {
            let Some(HeapValue::Iterator(iterator_state)) =
                heap_ref(&mut runtime.heap).and_then(|heap| heap.heap.get_mut(reference).ok())
            else {
                return Err(VmError::new(VmErrorKind::TypeMismatch {
                    operation: "iterator",
                }));
            };
            Ok(iterator_state.next())
        }
        _ => Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "iterator",
        })),
    }
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
