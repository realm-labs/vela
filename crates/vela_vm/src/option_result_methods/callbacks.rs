use crate::method_runtime::{MethodRuntime, call_callback};
use crate::option_result::{option_value, result_value};
use crate::{Value, VmError, VmErrorKind, VmResult};

use super::access::{
    EnumKind, enum_payload, enum_tag, expect_arity, expect_enum_kind, is_truthy, type_error,
};

pub(crate) fn map(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("map", args, 1)?;
    let tag = enum_tag(receiver, runtime.heap.as_deref()).ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "method map",
        })
    })?;

    match (tag.kind, tag.variant.as_str()) {
        (EnumKind::Option, "Some") => {
            let payload = enum_payload(receiver, runtime.heap.as_deref(), "method map")?;
            let mapped = call_callback(
                &mut runtime,
                "method map",
                &args[0],
                &[payload],
                std::slice::from_ref(receiver),
            )?;
            Ok(option_value(Some(mapped)))
        }
        (EnumKind::Option, "None") => Ok(option_value(None)),
        (EnumKind::Result, "Ok") => {
            let payload = enum_payload(receiver, runtime.heap.as_deref(), "method map")?;
            let mapped = call_callback(
                &mut runtime,
                "method map",
                &args[0],
                &[payload],
                std::slice::from_ref(receiver),
            )?;
            Ok(result_value("Ok", mapped))
        }
        (EnumKind::Result, "Err") => enum_payload(receiver, runtime.heap.as_deref(), "method map")
            .map(|payload| result_value("Err", payload)),
        _ => type_error("method map"),
    }
}

pub(crate) fn map_err(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("map_err", args, 1)?;
    let tag = enum_tag(receiver, runtime.heap.as_deref()).ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "method map_err",
        })
    })?;

    match (tag.kind, tag.variant.as_str()) {
        (EnumKind::Result, "Ok") => {
            enum_payload(receiver, runtime.heap.as_deref(), "method map_err")
                .map(|payload| result_value("Ok", payload))
        }
        (EnumKind::Result, "Err") => {
            let payload = enum_payload(receiver, runtime.heap.as_deref(), "method map_err")?;
            let mapped = call_callback(
                &mut runtime,
                "method map_err",
                &args[0],
                &[payload],
                std::slice::from_ref(receiver),
            )?;
            Ok(result_value("Err", mapped))
        }
        _ => type_error("method map_err"),
    }
}

pub(crate) fn and_then(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("and_then", args, 1)?;
    let tag = enum_tag(receiver, runtime.heap.as_deref()).ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "method and_then",
        })
    })?;

    match (tag.kind, tag.variant.as_str()) {
        (EnumKind::Option, "Some") => {
            let payload = enum_payload(receiver, runtime.heap.as_deref(), "method and_then")?;
            let chained = call_callback(
                &mut runtime,
                "method and_then",
                &args[0],
                &[payload],
                std::slice::from_ref(receiver),
            )?;
            expect_enum_kind(
                chained,
                runtime.heap.as_deref(),
                EnumKind::Option,
                "method and_then",
            )
        }
        (EnumKind::Option, "None") => Ok(option_value(None)),
        (EnumKind::Result, "Ok") => {
            let payload = enum_payload(receiver, runtime.heap.as_deref(), "method and_then")?;
            let chained = call_callback(
                &mut runtime,
                "method and_then",
                &args[0],
                &[payload],
                std::slice::from_ref(receiver),
            )?;
            expect_enum_kind(
                chained,
                runtime.heap.as_deref(),
                EnumKind::Result,
                "method and_then",
            )
        }
        (EnumKind::Result, "Err") => {
            enum_payload(receiver, runtime.heap.as_deref(), "method and_then")
                .map(|payload| result_value("Err", payload))
        }
        _ => type_error("method and_then"),
    }
}

pub(crate) fn or_else(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("or_else", args, 1)?;
    let tag = enum_tag(receiver, runtime.heap.as_deref()).ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "method or_else",
        })
    })?;

    match (tag.kind, tag.variant.as_str()) {
        (EnumKind::Option, "Some") => {
            enum_payload(receiver, runtime.heap.as_deref(), "method or_else")
                .map(|payload| option_value(Some(payload)))
        }
        (EnumKind::Option, "None") => {
            let fallback = call_callback(
                &mut runtime,
                "method or_else",
                &args[0],
                &[],
                std::slice::from_ref(receiver),
            )?;
            expect_enum_kind(
                fallback,
                runtime.heap.as_deref(),
                EnumKind::Option,
                "method or_else",
            )
        }
        (EnumKind::Result, "Ok") => {
            enum_payload(receiver, runtime.heap.as_deref(), "method or_else")
                .map(|payload| result_value("Ok", payload))
        }
        (EnumKind::Result, "Err") => {
            let payload = enum_payload(receiver, runtime.heap.as_deref(), "method or_else")?;
            let fallback = call_callback(
                &mut runtime,
                "method or_else",
                &args[0],
                &[payload],
                std::slice::from_ref(receiver),
            )?;
            expect_enum_kind(
                fallback,
                runtime.heap.as_deref(),
                EnumKind::Result,
                "method or_else",
            )
        }
        _ => type_error("method or_else"),
    }
}

pub(crate) fn filter(
    receiver: &Value,
    args: &[Value],
    mut runtime: MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    expect_arity("filter", args, 1)?;
    let tag = enum_tag(receiver, runtime.heap.as_deref()).ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "method filter",
        })
    })?;

    match (tag.kind, tag.variant.as_str()) {
        (EnumKind::Option, "Some") => {
            let payload = enum_payload(receiver, runtime.heap.as_deref(), "method filter")?;
            let predicate = call_callback(
                &mut runtime,
                "method filter",
                &args[0],
                std::slice::from_ref(&payload),
                std::slice::from_ref(receiver),
            )?;
            if is_truthy(&predicate) {
                Ok(option_value(Some(payload)))
            } else {
                Ok(option_value(None))
            }
        }
        (EnumKind::Option, "None") => Ok(option_value(None)),
        _ => type_error("method filter"),
    }
}
