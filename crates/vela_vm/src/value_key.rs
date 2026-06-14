use crate::heap::HeapValue;
use crate::{HeapExecution, Value, VmError, VmErrorKind, VmResult};
use vela_host::path::HostRef;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ValueKey {
    Null,
    Bool(bool),
    Char(char),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    F32(u32),
    F64(u64),
    String(String),
    Bytes(Vec<u8>),
    HeapIdentity(crate::heap::GcRef),
    HostIdentity(HostRef),
}

impl ValueKey {
    pub(crate) fn from_value(
        value: &Value,
        heap: Option<&HeapExecution<'_>>,
        operation: &'static str,
    ) -> VmResult<Self> {
        match value {
            Value::Missing => type_error(operation),
            Value::Null => Ok(Self::Null),
            Value::Bool(value) => Ok(Self::Bool(*value)),
            Value::Char(value) => Ok(Self::Char(*value)),
            Value::I8(value) => Ok(Self::I8(*value)),
            Value::I16(value) => Ok(Self::I16(*value)),
            Value::I32(value) => Ok(Self::I32(*value)),
            Value::I64(value) => Ok(Self::I64(*value)),
            Value::U8(value) => Ok(Self::U8(*value)),
            Value::U16(value) => Ok(Self::U16(*value)),
            Value::U32(value) => Ok(Self::U32(*value)),
            Value::U64(value) => Ok(Self::U64(*value)),
            Value::F32(value) => finite_f32_key(*value, operation).map(Self::F32),
            Value::F64(value) => finite_f64_key(*value, operation).map(Self::F64),
            Value::Range(_) => type_error(operation),
            Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
                Some(HeapValue::String(value)) => Ok(Self::String(value.clone())),
                Some(HeapValue::Bytes(value)) => Ok(Self::Bytes(value.clone())),
                Some(HeapValue::PathProxy(_)) | None => type_error(operation),
                Some(
                    HeapValue::Array(_)
                    | HeapValue::Map(_)
                    | HeapValue::Set(_)
                    | HeapValue::Record { .. }
                    | HeapValue::Enum { .. }
                    | HeapValue::Closure(_)
                    | HeapValue::Iterator(_),
                ) => Ok(Self::HeapIdentity(*reference)),
            },
            Value::HostRef(reference) => Ok(Self::HostIdentity(*reference)),
        }
    }
}

fn finite_f32_key(value: f32, operation: &'static str) -> VmResult<u32> {
    if value.is_nan() {
        return type_error(operation);
    }
    Ok(if value == 0.0 {
        0.0f32.to_bits()
    } else {
        value.to_bits()
    })
}

fn finite_f64_key(value: f64, operation: &'static str) -> VmResult<u64> {
    if value.is_nan() {
        return type_error(operation);
    }
    Ok(if value == 0.0 {
        0.0f64.to_bits()
    } else {
        value.to_bits()
    })
}

fn type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use vela_common::{HostObjectId, HostTypeId, ShapeId};
    use vela_def::TypeId;
    use vela_host::path::HostRef;
    use vela_host::proxy::PathProxy;
    use vela_host::target::HostTargetPlan;

    use crate::heap::{HeapValue, RecordIdentity, ScriptHeap};
    use crate::script_object::ScriptFields;
    use crate::{HeapExecution, Value, VmErrorKind};

    use super::ValueKey;

    #[test]
    fn value_key_accepts_leaf_values_by_exact_value() {
        assert_eq!(key(&Value::Null), ValueKey::Null);
        assert_eq!(key(&Value::Bool(true)), ValueKey::Bool(true));
        assert_eq!(key(&Value::Char('v')), ValueKey::Char('v'));
        assert_eq!(key(&Value::I8(-1)), ValueKey::I8(-1));
        assert_eq!(key(&Value::I16(-2)), ValueKey::I16(-2));
        assert_eq!(key(&Value::I32(-3)), ValueKey::I32(-3));
        assert_eq!(key(&Value::I64(-4)), ValueKey::I64(-4));
        assert_eq!(key(&Value::U8(1)), ValueKey::U8(1));
        assert_eq!(key(&Value::U16(2)), ValueKey::U16(2));
        assert_eq!(key(&Value::U32(3)), ValueKey::U32(3));
        assert_eq!(key(&Value::U64(4)), ValueKey::U64(4));
    }

    #[test]
    fn value_key_uses_tag_exact_scalar_classes() {
        assert_ne!(key(&Value::I64(1)), key(&Value::U64(1)));
        assert_ne!(key(&Value::F32(1.0)), key(&Value::F64(1.0)));
    }

    #[test]
    fn value_key_rejects_nan_and_normalizes_negative_zero() {
        assert_type_mismatch(&Value::F32(f32::NAN));
        assert_type_mismatch(&Value::F64(f64::NAN));
        assert_eq!(key(&Value::F32(-0.0)), key(&Value::F32(0.0)));
        assert_eq!(key(&Value::F64(-0.0)), key(&Value::F64(0.0)));
    }

    #[test]
    fn value_key_clones_string_and_bytes_payloads() {
        let mut heap = ScriptHeap::new();
        let string = heap.allocate(HeapValue::String("player".to_owned()));
        let bytes = heap.allocate(HeapValue::Bytes(vec![1, 2, 3]));
        let heap = HeapExecution::new(&mut heap);

        assert_eq!(
            ValueKey::from_value(&Value::HeapRef(string), Some(&heap), "test").unwrap(),
            ValueKey::String("player".to_owned())
        );
        assert_eq!(
            ValueKey::from_value(&Value::HeapRef(bytes), Some(&heap), "test").unwrap(),
            ValueKey::Bytes(vec![1, 2, 3])
        );
    }

    #[test]
    fn value_key_uses_heap_identity_for_script_objects() {
        let mut heap = ScriptHeap::new();
        let first = heap.allocate(record("Player"));
        let second = heap.allocate(record("Player"));
        let heap = HeapExecution::new(&mut heap);

        assert_eq!(
            ValueKey::from_value(&Value::HeapRef(first), Some(&heap), "test").unwrap(),
            ValueKey::HeapIdentity(first)
        );
        assert_ne!(
            ValueKey::from_value(&Value::HeapRef(first), Some(&heap), "test").unwrap(),
            ValueKey::from_value(&Value::HeapRef(second), Some(&heap), "test").unwrap()
        );
    }

    #[test]
    fn value_key_uses_host_ref_identity() {
        let first = HostRef::new(HostTypeId::new(1), HostObjectId::new(7), 1);
        let second = HostRef::new(HostTypeId::new(1), HostObjectId::new(7), 2);

        assert_eq!(key(&Value::HostRef(first)), ValueKey::HostIdentity(first));
        assert_ne!(key(&Value::HostRef(first)), key(&Value::HostRef(second)));
    }

    #[test]
    fn value_key_rejects_transient_values() {
        assert_type_mismatch(&Value::Missing);
        assert_type_mismatch(&Value::Range(crate::ranges::RangeValue::new(0, 1, false)));

        let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(7), 1);
        let plan = HostTargetPlan::new(host_ref.type_id);
        let mut heap = ScriptHeap::new();
        let path_proxy = heap.allocate(HeapValue::PathProxy(PathProxy::new(host_ref, plan)));
        let heap = HeapExecution::new(&mut heap);

        let error =
            ValueKey::from_value(&Value::HeapRef(path_proxy), Some(&heap), "test").unwrap_err();
        assert_eq!(
            error.kind(),
            VmErrorKind::TypeMismatch { operation: "test" }
        );
    }

    fn key(value: &Value) -> ValueKey {
        ValueKey::from_value(value, None, "test").expect("value should be keyable")
    }

    fn assert_type_mismatch(value: &Value) {
        let error = ValueKey::from_value(value, None, "test").unwrap_err();
        assert_eq!(
            error.kind(),
            VmErrorKind::TypeMismatch { operation: "test" }
        );
    }

    fn record(type_name: &str) -> HeapValue {
        HeapValue::Record {
            type_name: type_name.to_owned(),
            identity: Some(RecordIdentity::new(TypeId::new(1), ShapeId::new(1))),
            fields: ScriptFields::from(BTreeMap::from([("id".to_owned(), Value::I64(1))])),
        }
    }
}
