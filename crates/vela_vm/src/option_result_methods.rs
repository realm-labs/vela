use crate::heap::HeapValue;
use crate::method_runtime::{MethodRuntime, call_callback};
use crate::option_result::{option_value, result_value};
use crate::{HeapExecution, Value, VmError, VmErrorKind, VmResult, value_from_heap_slot};

pub(crate) fn is_option_or_result(receiver: &Value, heap: Option<&HeapExecution<'_>>) -> bool {
    enum_tag(receiver, heap).is_some_and(|tag| tag.is_option() || tag.is_result())
}

pub(crate) fn is_result(receiver: &Value, heap: Option<&HeapExecution<'_>>) -> bool {
    enum_tag(receiver, heap).is_some_and(|tag| tag.is_result())
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

fn type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}
