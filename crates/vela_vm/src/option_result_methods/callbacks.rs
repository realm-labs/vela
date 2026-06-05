use crate::method_runtime::{MethodRuntime, call_callback};
use crate::option_result::{option_value, result_value};
use crate::{Value, VmError, VmErrorKind, VmResult};

use super::access::{
    EnumKind, EnumVariant, enum_payload, enum_tag, expect_arity, expect_enum_kind, is_truthy,
    type_error,
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

    match (tag.kind, tag.variant) {
        (EnumKind::Option, EnumVariant::Some) => {
            let payload = enum_payload(receiver, runtime.heap.as_deref(), "method map")?;
            let mapped = call_callback(
                &mut runtime,
                "method map",
                &args[0],
                &[payload],
                std::slice::from_ref(receiver),
            )?;
            option_result(Some(mapped), &mut runtime)
        }
        (EnumKind::Option, EnumVariant::None) => option_result(None, &mut runtime),
        (EnumKind::Result, EnumVariant::Ok) => {
            let payload = enum_payload(receiver, runtime.heap.as_deref(), "method map")?;
            let mapped = call_callback(
                &mut runtime,
                "method map",
                &args[0],
                &[payload],
                std::slice::from_ref(receiver),
            )?;
            result_result("Ok", mapped, &mut runtime)
        }
        (EnumKind::Result, EnumVariant::Err) => {
            enum_payload(receiver, runtime.heap.as_deref(), "method map")
                .and_then(|payload| result_result("Err", payload, &mut runtime))
        }
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

    match (tag.kind, tag.variant) {
        (EnumKind::Result, EnumVariant::Ok) => {
            enum_payload(receiver, runtime.heap.as_deref(), "method map_err")
                .and_then(|payload| result_result("Ok", payload, &mut runtime))
        }
        (EnumKind::Result, EnumVariant::Err) => {
            let payload = enum_payload(receiver, runtime.heap.as_deref(), "method map_err")?;
            let mapped = call_callback(
                &mut runtime,
                "method map_err",
                &args[0],
                &[payload],
                std::slice::from_ref(receiver),
            )?;
            result_result("Err", mapped, &mut runtime)
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

    match (tag.kind, tag.variant) {
        (EnumKind::Option, EnumVariant::Some) => {
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
        (EnumKind::Option, EnumVariant::None) => option_result(None, &mut runtime),
        (EnumKind::Result, EnumVariant::Ok) => {
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
        (EnumKind::Result, EnumVariant::Err) => {
            enum_payload(receiver, runtime.heap.as_deref(), "method and_then")
                .and_then(|payload| result_result("Err", payload, &mut runtime))
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

    match (tag.kind, tag.variant) {
        (EnumKind::Option, EnumVariant::Some) => {
            enum_payload(receiver, runtime.heap.as_deref(), "method or_else")
                .and_then(|payload| option_result(Some(payload), &mut runtime))
        }
        (EnumKind::Option, EnumVariant::None) => {
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
        (EnumKind::Result, EnumVariant::Ok) => {
            enum_payload(receiver, runtime.heap.as_deref(), "method or_else")
                .and_then(|payload| result_result("Ok", payload, &mut runtime))
        }
        (EnumKind::Result, EnumVariant::Err) => {
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

    match (tag.kind, tag.variant) {
        (EnumKind::Option, EnumVariant::Some) => {
            let payload = enum_payload(receiver, runtime.heap.as_deref(), "method filter")?;
            let predicate = call_callback(
                &mut runtime,
                "method filter",
                &args[0],
                std::slice::from_ref(&payload),
                std::slice::from_ref(receiver),
            )?;
            if is_truthy(&predicate) {
                option_result(Some(payload), &mut runtime)
            } else {
                option_result(None, &mut runtime)
            }
        }
        (EnumKind::Option, EnumVariant::None) => option_result(None, &mut runtime),
        _ => type_error("method filter"),
    }
}

fn option_result(
    payload: Option<Value>,
    runtime: &mut MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    let Some(heap) = runtime.heap.as_deref_mut() else {
        return type_error("method option");
    };
    option_value(payload, heap, runtime.budget.as_deref_mut())
}

fn result_result(
    variant: &str,
    payload: Value,
    runtime: &mut MethodRuntime<'_, '_, '_>,
) -> VmResult<Value> {
    let Some(heap) = runtime.heap.as_deref_mut() else {
        return type_error("method result");
    };
    result_value(variant, payload, heap, runtime.budget.as_deref_mut())
}
