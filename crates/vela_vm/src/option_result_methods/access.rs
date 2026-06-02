use crate::heap::HeapValue;
use crate::{HeapExecution, Value, VmError, VmErrorKind, VmResult, value_from_heap_slot};

pub(super) struct EnumTag {
    pub(super) kind: EnumKind,
    pub(super) variant: String,
}

impl EnumTag {
    pub(super) fn is_option(&self) -> bool {
        self.kind == EnumKind::Option
    }

    pub(super) fn is_result(&self) -> bool {
        self.kind == EnumKind::Result
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum EnumKind {
    Option,
    Result,
}

pub(super) fn enum_tag(receiver: &Value, heap: Option<&HeapExecution<'_>>) -> Option<EnumTag> {
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

    let kind = match enum_name.rsplit("::").next() {
        Some("Option") => EnumKind::Option,
        Some("Result") => EnumKind::Result,
        _ => return None,
    };
    Some(EnumTag {
        kind,
        variant: variant.to_owned(),
    })
}

pub(super) fn option_variant(
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

pub(super) fn result_variant(
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

pub(super) fn enum_payload(
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

pub(super) fn expect_arity(name: &str, args: &[Value], expected: usize) -> VmResult<()> {
    if args.len() == expected {
        return Ok(());
    }
    Err(VmError::new(VmErrorKind::ArityMismatch {
        name: name.to_owned(),
        expected,
        actual: args.len(),
    }))
}

pub(super) fn expect_enum_kind(
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

pub(super) fn is_truthy(value: &Value) -> bool {
    !matches!(value, Value::Missing | Value::Null | Value::Bool(false))
}

pub(super) fn type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}
