use crate::runtime_view::EnumView;
use crate::{HeapExecution, Value, VmError, VmErrorKind, VmResult};

pub(super) struct EnumTag {
    pub(super) kind: EnumKind,
    pub(super) variant: EnumVariant,
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

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum EnumVariant {
    Some,
    None,
    Ok,
    Err,
    Other,
}

pub(super) fn enum_tag(receiver: &Value, heap: Option<&HeapExecution<'_>>) -> Option<EnumTag> {
    let view = EnumView::from_value(receiver, heap)?;

    let kind = match view.enum_name.rsplit("::").next() {
        Some("Option") => EnumKind::Option,
        Some("Result") => EnumKind::Result,
        _ => return None,
    };
    Some(EnumTag {
        kind,
        variant: enum_variant(view.variant),
    })
}

fn enum_variant(variant: &str) -> EnumVariant {
    match variant {
        "Some" => EnumVariant::Some,
        "None" => EnumVariant::None,
        "Ok" => EnumVariant::Ok,
        "Err" => EnumVariant::Err,
        _ => EnumVariant::Other,
    }
}

pub(super) fn option_variant(
    receiver: &Value,
    heap: Option<&HeapExecution<'_>>,
    operation: &'static str,
) -> VmResult<EnumVariant> {
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
) -> VmResult<EnumVariant> {
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
    EnumView::from_value(receiver, heap)
        .and_then(|view| view.fields.get_owned("0"))
        .ok_or_else(|| VmError::new(VmErrorKind::TypeMismatch { operation }))
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
