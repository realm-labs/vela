use crate::option_result::{option_value, result_value};
use crate::{HeapExecution, Value, VmError, VmErrorKind, VmResult};

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
        tag if tag.kind == EnumKind::Option && tag.variant == EnumVariant::None => {
            Ok(args[0].clone())
        }
        tag if tag.kind == EnumKind::Result && tag.variant == EnumVariant::Ok => {
            enum_payload(receiver, heap, "method unwrap_or")
        }
        tag if tag.kind == EnumKind::Result && tag.variant == EnumVariant::Err => {
            Ok(args[0].clone())
        }
        _ => type_error("method unwrap_or"),
    }
}

pub(crate) fn ok_or(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("ok_or", args, 1)?;
    match option_variant(receiver, heap, "method ok_or")? {
        EnumVariant::Some => {
            enum_payload(receiver, heap, "method ok_or").map(|payload| result_value("Ok", payload))
        }
        EnumVariant::None => Ok(result_value("Err", args[0].clone())),
        _ => type_error("method ok_or"),
    }
}

pub(crate) fn to_option(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("to_option", args, 0)?;
    match result_variant(receiver, heap, "method to_option")? {
        EnumVariant::Ok => enum_payload(receiver, heap, "method to_option")
            .map(Some)
            .map(option_value),
        EnumVariant::Err => Ok(option_value(None)),
        _ => type_error("method to_option"),
    }
}

pub(crate) fn to_error_option(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("to_error_option", args, 0)?;
    match result_variant(receiver, heap, "method to_error_option")? {
        EnumVariant::Ok => Ok(option_value(None)),
        EnumVariant::Err => enum_payload(receiver, heap, "method to_error_option")
            .map(Some)
            .map(option_value),
        _ => type_error("method to_error_option"),
    }
}

pub(crate) fn flatten(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("flatten", args, 0)?;
    let tag = enum_tag(receiver, heap).ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "method flatten",
        })
    })?;

    match (tag.kind, tag.variant) {
        (EnumKind::Option, EnumVariant::Some) => {
            let payload = enum_payload(receiver, heap, "method flatten")?;
            expect_enum_kind(payload, heap, EnumKind::Option, "method flatten")
        }
        (EnumKind::Option, EnumVariant::None) => Ok(option_value(None)),
        (EnumKind::Result, EnumVariant::Ok) => {
            let payload = enum_payload(receiver, heap, "method flatten")?;
            expect_enum_kind(payload, heap, EnumKind::Result, "method flatten")
        }
        (EnumKind::Result, EnumVariant::Err) => enum_payload(receiver, heap, "method flatten")
            .map(|payload| result_value("Err", payload)),
        _ => type_error("method flatten"),
    }
}
