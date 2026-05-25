use crate::heap::HeapValue;
use crate::method_runtime::{MethodRuntime, call_callback};
use crate::option_result::{option_value, result_value};
use crate::{HeapExecution, Value, VmError, VmErrorKind, VmResult, value_from_heap_slot};

pub(crate) fn is_option_or_result(receiver: &Value, heap: Option<&HeapExecution<'_>>) -> bool {
    enum_tag(receiver, heap).is_some_and(|tag| tag.is_option() || tag.is_result())
}

pub(crate) fn is_option(receiver: &Value, heap: Option<&HeapExecution<'_>>) -> bool {
    enum_tag(receiver, heap).is_some_and(|tag| tag.is_option())
}

pub(crate) fn is_result(receiver: &Value, heap: Option<&HeapExecution<'_>>) -> bool {
    enum_tag(receiver, heap).is_some_and(|tag| tag.is_result())
}

pub(crate) fn is_some(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("is_some", args, 0)?;
    match option_variant(receiver, heap, "method is_some")?.as_str() {
        "Some" => Ok(Value::Bool(true)),
        "None" => Ok(Value::Bool(false)),
        _ => type_error("method is_some"),
    }
}

pub(crate) fn is_none(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("is_none", args, 0)?;
    match option_variant(receiver, heap, "method is_none")?.as_str() {
        "Some" => Ok(Value::Bool(false)),
        "None" => Ok(Value::Bool(true)),
        _ => type_error("method is_none"),
    }
}

pub(crate) fn is_ok(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("is_ok", args, 0)?;
    match result_variant(receiver, heap, "method is_ok")?.as_str() {
        "Ok" => Ok(Value::Bool(true)),
        "Err" => Ok(Value::Bool(false)),
        _ => type_error("method is_ok"),
    }
}

pub(crate) fn is_err(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("is_err", args, 0)?;
    match result_variant(receiver, heap, "method is_err")?.as_str() {
        "Ok" => Ok(Value::Bool(false)),
        "Err" => Ok(Value::Bool(true)),
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
        EnumTag {
            kind: EnumKind::Option,
            variant,
        } => match variant.as_str() {
            "Some" => enum_payload(receiver, heap, "method unwrap_or"),
            "None" => Ok(args[0].clone()),
            _ => type_error("method unwrap_or"),
        },
        EnumTag {
            kind: EnumKind::Result,
            variant,
        } => match variant.as_str() {
            "Ok" => enum_payload(receiver, heap, "method unwrap_or"),
            "Err" => Ok(args[0].clone()),
            _ => type_error("method unwrap_or"),
        },
    }
}

pub(crate) fn ok_or(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("ok_or", args, 1)?;
    match option_variant(receiver, heap, "method ok_or")?.as_str() {
        "Some" => {
            enum_payload(receiver, heap, "method ok_or").map(|payload| result_value("Ok", payload))
        }
        "None" => Ok(result_value("Err", args[0].clone())),
        _ => type_error("method ok_or"),
    }
}

pub(crate) fn to_option(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("to_option", args, 0)?;
    match result_variant(receiver, heap, "method to_option")?.as_str() {
        "Ok" => enum_payload(receiver, heap, "method to_option")
            .map(Some)
            .map(option_value),
        "Err" => Ok(option_value(None)),
        _ => type_error("method to_option"),
    }
}

pub(crate) fn to_error_option(
    receiver: &Value,
    args: &[Value],
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Value> {
    expect_arity("to_error_option", args, 0)?;
    match result_variant(receiver, heap, "method to_error_option")?.as_str() {
        "Ok" => Ok(option_value(None)),
        "Err" => enum_payload(receiver, heap, "method to_error_option")
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

    match (tag.kind, tag.variant.as_str()) {
        (EnumKind::Option, "Some") => {
            let payload = enum_payload(receiver, heap, "method flatten")?;
            expect_enum_kind(payload, heap, EnumKind::Option, "method flatten")
        }
        (EnumKind::Option, "None") => Ok(option_value(None)),
        (EnumKind::Result, "Ok") => {
            let payload = enum_payload(receiver, heap, "method flatten")?;
            expect_enum_kind(payload, heap, EnumKind::Result, "method flatten")
        }
        (EnumKind::Result, "Err") => enum_payload(receiver, heap, "method flatten")
            .map(|payload| result_value("Err", payload)),
        _ => type_error("method flatten"),
    }
}

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

struct EnumTag {
    kind: EnumKind,
    variant: String,
}

impl EnumTag {
    fn is_option(&self) -> bool {
        self.kind == EnumKind::Option
    }

    fn is_result(&self) -> bool {
        self.kind == EnumKind::Result
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum EnumKind {
    Option,
    Result,
}

fn enum_tag(receiver: &Value, heap: Option<&HeapExecution<'_>>) -> Option<EnumTag> {
    let (enum_name, variant) = match receiver {
        Value::Enum {
            enum_name, variant, ..
        } => (enum_name.as_str(), variant.as_str()),
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::Enum {
                enum_name, variant, ..
            }) => (enum_name.as_str(), variant.as_str()),
            _ => return None,
        },
        _ => return None,
    };

    let kind = match enum_name.rsplit('.').next() {
        Some("Option") => EnumKind::Option,
        Some("Result") => EnumKind::Result,
        _ => return None,
    };
    Some(EnumTag {
        kind,
        variant: variant.to_owned(),
    })
}

fn option_variant(
    receiver: &Value,
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<String> {
    let tag = enum_tag(receiver, heap)
        .ok_or_else(|| VmError::new(VmErrorKind::TypeMismatch { operation }))?;
    if tag.kind == EnumKind::Option {
        return Ok(tag.variant);
    }
    type_error(operation)
}

fn result_variant(
    receiver: &Value,
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<String> {
    let tag = enum_tag(receiver, heap)
        .ok_or_else(|| VmError::new(VmErrorKind::TypeMismatch { operation }))?;
    if tag.kind == EnumKind::Result {
        return Ok(tag.variant);
    }
    type_error(operation)
}

fn enum_payload(
    receiver: &Value,
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<Value> {
    match receiver {
        Value::Enum { fields, .. } => fields
            .get("0")
            .cloned()
            .ok_or_else(|| VmError::new(VmErrorKind::TypeMismatch { operation })),
        Value::HeapRef(reference) => {
            let Some(HeapValue::Enum { fields, .. }) =
                heap.and_then(|heap| heap.heap.get(*reference))
            else {
                return type_error(operation);
            };
            fields
                .get("0")
                .map(value_from_heap_slot)
                .ok_or_else(|| VmError::new(VmErrorKind::TypeMismatch { operation }))
        }
        _ => type_error(operation),
    }
}

fn expect_arity(name: &str, args: &[Value], expected: usize) -> VmResult<()> {
    if args.len() == expected {
        return Ok(());
    }
    Err(VmError::new(VmErrorKind::ArityMismatch {
        name: name.to_owned(),
        expected,
        actual: args.len(),
    }))
}

fn expect_enum_kind(
    value: Value,
    heap: Option<&HeapExecution<'_>>,
    expected: EnumKind,
    operation: &'static str,
) -> VmResult<Value> {
    match enum_tag(&value, heap) {
        Some(tag) if tag.kind == expected => Ok(value),
        _ => type_error(operation),
    }
}

fn is_truthy(value: &Value) -> bool {
    !matches!(value, Value::Missing | Value::Null | Value::Bool(false))
}

fn type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}
