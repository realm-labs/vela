use crate::heap::HeapValue;
use crate::{HeapExecution, Value, VmError, VmErrorKind, VmResult, stored_runtime_value};

pub(crate) fn tuple_arity_equal(
    value: &Value,
    heap: Option<&HeapExecution<'_>>,
    arity: usize,
) -> VmResult<bool> {
    let Some(values) = tuple_values(value, heap, "tuple pattern")? else {
        return Ok(false);
    };
    Ok(values.len() == arity)
}

pub(crate) fn guard_tuple_arity(
    value: &Value,
    heap: Option<&HeapExecution<'_>>,
    arity: usize,
) -> VmResult<()> {
    let Some(values) = tuple_values(value, heap, "tuple destructuring")? else {
        return Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "tuple destructuring",
        }));
    };
    if values.len() != arity {
        return Err(VmError::new(VmErrorKind::ArityMismatch {
            name: "tuple destructuring".to_owned(),
            expected: arity,
            actual: values.len(),
        }));
    }
    Ok(())
}

pub(crate) fn get_tuple_field(
    value: &Value,
    heap: Option<&HeapExecution<'_>>,
    index: usize,
) -> VmResult<Value> {
    let Some(values) = tuple_values(value, heap, "tuple field")? else {
        return Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "tuple field",
        }));
    };
    values.get(index).map(stored_runtime_value).ok_or_else(|| {
        VmError::new(VmErrorKind::IndexOutOfBounds {
            index: i64::try_from(index).unwrap_or(i64::MAX),
            len: values.len(),
        })
    })
}

fn tuple_values<'a>(
    value: &Value,
    heap: Option<&'a HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<Option<&'a [Value]>> {
    let Value::HeapRef(reference) = value else {
        return Ok(None);
    };
    let Some(heap) = heap else {
        return Err(VmError::new(VmErrorKind::TypeMismatch { operation }));
    };
    match heap.heap.get(*reference) {
        Some(HeapValue::Tuple(values)) => Ok(Some(values)),
        Some(_) | None => Ok(None),
    }
}
