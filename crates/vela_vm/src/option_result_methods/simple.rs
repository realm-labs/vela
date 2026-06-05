use crate::option_result::{option_value, result_value};
use crate::{ExecutionBudget, HeapExecution, Value, VmError, VmErrorKind, VmResult};

use super::access::{
    EnumKind, EnumVariant, enum_payload, enum_tag, expect_arity, expect_enum_kind, option_variant,
    result_variant, type_error,
};

pub(crate) fn is_some(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("is_some", args, 0)?;
    match option_variant(receiver, heap, "method is_some")? {
        EnumVariant::Some => Ok(Value::Bool(true)),
        EnumVariant::None => Ok(Value::Bool(false)),
        _ => type_error("method is_some"),
    }
}

pub(crate) fn is_none(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("is_none", args, 0)?;
    match option_variant(receiver, heap, "method is_none")? {
        EnumVariant::Some => Ok(Value::Bool(false)),
        EnumVariant::None => Ok(Value::Bool(true)),
        _ => type_error("method is_none"),
    }
}

pub(crate) fn is_ok(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("is_ok", args, 0)?;
    match result_variant(receiver, heap, "method is_ok")? {
        EnumVariant::Ok => Ok(Value::Bool(true)),
        EnumVariant::Err => Ok(Value::Bool(false)),
        _ => type_error("method is_ok"),
    }
}

pub(crate) fn is_err(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("is_err", args, 0)?;
    match result_variant(receiver, heap, "method is_err")? {
        EnumVariant::Ok => Ok(Value::Bool(false)),
        EnumVariant::Err => Ok(Value::Bool(true)),
        _ => type_error("method is_err"),
    }
}

pub(crate) fn unwrap_or(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("unwrap_or", args, 1)?;
    match enum_tag(receiver, heap).ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "method unwrap_or",
        })
    })? {
        tag if tag.kind == EnumKind::Option && tag.variant == EnumVariant::Some => {
            enum_payload(receiver, heap, "method unwrap_or")
        }
        tag if tag.kind == EnumKind::Option && tag.variant == EnumVariant::None => Ok(args[0]),
        tag if tag.kind == EnumKind::Result && tag.variant == EnumVariant::Ok => {
            enum_payload(receiver, heap, "method unwrap_or")
        }
        tag if tag.kind == EnumKind::Result && tag.variant == EnumVariant::Err => Ok(args[0]),
        _ => type_error("method unwrap_or"),
    }
}

pub(crate) fn ok_or(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("ok_or", args, 1)?;
    match option_variant(receiver, heap.as_deref(), "method ok_or")? {
        EnumVariant::Some => {
            let payload = enum_payload(receiver, heap.as_deref(), "method ok_or")?;
            result_result("Ok", payload, heap, budget, "method ok_or")
        }
        EnumVariant::None => result_result("Err", args[0], heap, budget, "method ok_or"),
        _ => type_error("method ok_or"),
    }
}

pub(crate) fn to_option(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("to_option", args, 0)?;
    match result_variant(receiver, heap.as_deref(), "method to_option")? {
        EnumVariant::Ok => {
            let payload = enum_payload(receiver, heap.as_deref(), "method to_option")?;
            option_result(Some(payload), heap, budget, "method to_option")
        }
        EnumVariant::Err => option_result(None, heap, budget, "method to_option"),
        _ => type_error("method to_option"),
    }
}

pub(crate) fn to_error_option(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("to_error_option", args, 0)?;
    match result_variant(receiver, heap.as_deref(), "method to_error_option")? {
        EnumVariant::Ok => option_result(None, heap, budget, "method to_error_option"),
        EnumVariant::Err => {
            let payload = enum_payload(receiver, heap.as_deref(), "method to_error_option")?;
            option_result(Some(payload), heap, budget, "method to_error_option")
        }
        _ => type_error("method to_error_option"),
    }
}

pub(crate) fn flatten(
    receiver: &Value,
    args: &[Value],
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    expect_arity("flatten", args, 0)?;
    let tag = enum_tag(receiver, heap.as_deref()).ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "method flatten",
        })
    })?;

    match (tag.kind, tag.variant) {
        (EnumKind::Option, EnumVariant::Some) => {
            let payload = enum_payload(receiver, heap.as_deref(), "method flatten")?;
            expect_enum_kind(payload, heap.as_deref(), EnumKind::Option, "method flatten")
        }
        (EnumKind::Option, EnumVariant::None) => {
            option_result(None, heap, budget, "method flatten")
        }
        (EnumKind::Result, EnumVariant::Ok) => {
            let payload = enum_payload(receiver, heap.as_deref(), "method flatten")?;
            expect_enum_kind(payload, heap.as_deref(), EnumKind::Result, "method flatten")
        }
        (EnumKind::Result, EnumVariant::Err) => {
            let payload = enum_payload(receiver, heap.as_deref(), "method flatten")?;
            result_result("Err", payload, heap, budget, "method flatten")
        }
        _ => type_error("method flatten"),
    }
}

fn option_result(
    payload: Option<Value>,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<Value> {
    let Some(heap) = heap.as_deref_mut() else {
        return type_error(operation);
    };
    option_value(payload, heap, budget.as_deref_mut())
}

fn result_result(
    variant: &str,
    payload: Value,
    heap: &mut Option<&mut HeapExecution<'_>>,
    budget: &mut Option<&mut ExecutionBudget>,
    operation: &'static str,
) -> VmResult<Value> {
    let Some(heap) = heap.as_deref_mut() else {
        return type_error(operation);
    };
    result_value(variant, payload, heap, budget.as_deref_mut())
}
