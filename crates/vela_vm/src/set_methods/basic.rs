use crate::runtime_view::SetView;
use crate::{HeapExecution, Value, VmResult};

use super::{SetKey, expect_arity, materialize_set_values, push_unique, type_error};

pub(crate) fn from_array(args: &[Value]) -> VmResult<Value> {
    expect_arity("set::from_array", args, 1)?;
    let Value::Array(values) = &args[0] else {
        return type_error("set::from_array");
    };
    let mut set = Vec::new();
    for value in values {
        push_unique(&mut set, value.clone(), None, "set::from_array")?;
    }
    Ok(Value::Set(set))
}

pub(crate) fn has(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<bool> {
    expect_arity("has", args, 1)?;
    let key = SetKey::from_value(&args[0], heap, "method has")?;
    match SetView::from_value(receiver, heap, "method has")? {
        SetView::Values(values) => {
            for value in values {
                if key.matches_value(value, heap, "method has")? {
                    return Ok(true);
                }
            }
        }
        SetView::Slots(values, heap) => {
            for value in values {
                if key.matches_slot(value, heap, "method has")? {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

pub(crate) fn values(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("values", args, 0)?;
    materialize_set_values(receiver, heap, "method values").map(Value::Array)
}
