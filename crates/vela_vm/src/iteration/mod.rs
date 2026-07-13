mod methods;
mod resumable;
mod resumable_method;
mod source;
mod state;
mod step;

pub(crate) use methods::{
    all_method, any_method, chars_method, collect_array_method_runtime, collect_map_method_runtime,
    collect_set_method_runtime, collect_values, collect_values_over, count_method_runtime,
    filter_items_over, filter_method, find_method, is_iterator, iter_method, map_method,
    next_method_runtime, skip_method, string_bytes_method, take_method, try_for_each_over,
};
pub(crate) use methods::{
    callback_all, callback_all_over, callback_any, callback_any_over, callback_count,
    callback_count_over, callback_find, callback_find_over,
};
pub(crate) use resumable::{ResumableIteratorNext, ResumableIteratorStep};
pub(crate) use resumable_method::{ResumableIteratorMethod, ResumableIteratorMethodStep};
pub(crate) use source::make_iterator;
pub(crate) use state::IteratorItemGuard;
pub use state::IteratorState;
pub(crate) use step::{RangeNextStep, dispatch_linked_i64_range_next, dispatch_linked_range_next};

use crate::heap::HeapValue;
use crate::heap_values::allocate_heap_value;
use crate::{CallFrame, ExecutionBudget, HeapExecution, Value, VmError, VmErrorKind, VmResult};
use vela_bytecode::Register;

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
