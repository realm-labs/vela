use crate::heap::HeapValue;
use crate::option_result::{StdEnumKind, StdEnumVariant, std_enum_identity, std_enum_tag};
use crate::{HeapExecution, Value, VmError, VmErrorKind, VmResult, stored_runtime_value};

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

pub(super) type EnumKind = StdEnumKind;

pub(super) type EnumVariant = StdEnumVariant;

pub(super) fn enum_tag(receiver: &Value, heap: Option<&HeapExecution<'_>>) -> Option<EnumTag> {
    let identity = match receiver {
        Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
            Some(HeapValue::Enum {
                identity: Some(identity),
                ..
            }) => *identity,
            _ => return None,
        },
        _ => return None,
    };

    let (kind, variant) = std_enum_tag(identity)?;
    Some(EnumTag { kind, variant })
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
    match receiver {
        Value::HeapRef(reference) => {
            let Some(HeapValue::Enum {
                identity: Some(identity),
                fields,
                ..
            }) = heap.and_then(|heap| heap.heap.get(*reference))
            else {
                return type_error(operation);
            };
            let Some((_, variant)) = std_enum_tag(*identity) else {
                return type_error(operation);
            };
            if !variant.has_payload()
                || identity.payload_field_id != std_enum_identity(variant).payload_field_id
            {
                return type_error(operation);
            }
            fields
                .get_slot(0, "0")
                .map(stored_runtime_value)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::heap::{HeapValue, ScriptHeap};
    use crate::script_object::ScriptFields;
    use crate::{HeapExecution, Value};

    #[test]
    fn standard_enum_tag_uses_identity_not_debug_names() {
        let mut heap = ScriptHeap::new();
        let reference = heap.allocate(HeapValue::Enum {
            enum_name: "NotOption".to_owned(),
            variant: "Definitely".to_owned(),
            identity: Some(std_enum_identity(EnumVariant::Some)),
            fields: ScriptFields::single(
                "NotOption::Definitely",
                "0",
                Value::Scalar(vela_common::ScalarValue::I64(7)),
            ),
        });
        let execution = HeapExecution::new(&mut heap);
        let value = Value::HeapRef(reference);

        let tag = enum_tag(&value, Some(&execution)).expect("typed standard enum tag");
        assert_eq!(tag.kind, EnumKind::Option);
        assert_eq!(tag.variant, EnumVariant::Some);
        assert_eq!(
            enum_payload(&value, Some(&execution), "test payload").expect("typed payload"),
            Value::Scalar(vela_common::ScalarValue::I64(7))
        );
    }

    #[test]
    fn standard_enum_tag_rejects_name_only_values() {
        let mut heap = ScriptHeap::new();
        let reference = heap.allocate(HeapValue::Enum {
            enum_name: "Option".to_owned(),
            variant: "Some".to_owned(),
            identity: None,
            fields: ScriptFields::single(
                "Option::Some",
                "0",
                Value::Scalar(vela_common::ScalarValue::I64(7)),
            ),
        });
        let execution = HeapExecution::new(&mut heap);
        let value = Value::HeapRef(reference);

        assert!(enum_tag(&value, Some(&execution)).is_none());
        assert!(enum_payload(&value, Some(&execution), "test payload").is_err());
    }
}
