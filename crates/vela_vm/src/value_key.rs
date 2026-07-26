use std::hash::{Hash, Hasher};

use crate::heap::HeapValue;
use crate::{HeapExecution, Value, VmError, VmErrorKind, VmResult};
use vela_host::path::HostSlotRef;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ValueKey {
    Unit,
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
    HostIdentity(HostSlotRef),
}

impl ValueKey {
    pub(crate) fn remap_heap_refs(
        &mut self,
        references: &std::collections::BTreeMap<crate::heap::GcRef, crate::heap::GcRef>,
    ) -> VmResult<()> {
        if let Self::HeapIdentity(reference) = self {
            *reference = references.get(reference).copied().ok_or_else(|| {
                VmError::new(VmErrorKind::TypeMismatch {
                    operation: "iterator heap key graph copy",
                })
            })?;
        }
        Ok(())
    }

    pub(crate) fn from_value(
        value: &Value,
        heap: Option<&HeapExecution<'_>>,
        operation: &'static str,
    ) -> VmResult<Self> {
        match value {
            Value::Missing => type_error(operation),
            Value::Unit => Ok(Self::Unit),
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
            Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
                Some(HeapValue::String(value)) => Ok(Self::String(value.clone())),
                Some(HeapValue::Bytes(value)) => Ok(Self::Bytes(value.clone())),
                Some(HeapValue::Tuple(_) | HeapValue::Range(_) | HeapValue::PathProxy(_))
                | None => type_error(operation),
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

    #[must_use]
    pub(crate) fn payload_size_bytes(&self) -> usize {
        match self {
            Self::String(value) => value.len(),
            Self::Bytes(value) => value.len(),
            _ => 0,
        }
    }
}

/// Hashes one key part under an explicit tag byte.
///
/// `ValueKey` and `KeyProbe` are different enums that must hash identically,
/// so neither may rely on its own derived discriminant. Both feed this
/// canonical scheme instead; `str`/`String` and `[u8]`/`Vec<u8>` already share
/// their standard-library hashing.
macro_rules! hash_key_parts {
    ($state:expr, $key:expr, $string:ty, $bytes:ty) => {{
        let state = $state;
        match $key {
            KeyParts::<$string, $bytes>::Unit => state.write_u8(0),
            KeyParts::Bool(value) => {
                state.write_u8(1);
                value.hash(state);
            }
            KeyParts::Char(value) => {
                state.write_u8(2);
                value.hash(state);
            }
            KeyParts::I64Class(value) => {
                state.write_u8(3);
                value.hash(state);
            }
            KeyParts::U64Class(value) => {
                state.write_u8(4);
                value.hash(state);
            }
            KeyParts::F32(value) => {
                state.write_u8(5);
                value.hash(state);
            }
            KeyParts::F64(value) => {
                state.write_u8(6);
                value.hash(state);
            }
            KeyParts::String(value) => {
                state.write_u8(7);
                value.hash(state);
            }
            KeyParts::Bytes(value) => {
                state.write_u8(8);
                value.hash(state);
            }
            KeyParts::HeapIdentity(reference) => {
                state.write_u8(9);
                reference.hash(state);
            }
            KeyParts::HostIdentity(reference) => {
                state.write_u8(10);
                reference.hash(state);
            }
        }
    }};
}

/// Canonical hashed form of one key.
///
/// Integer widths keep their class tag but hash a widened payload, mirroring
/// `PartialEq` on `ValueKey`, where `I8(1) != I64(1)` — the width byte is part
/// of the payload below.
enum KeyParts<S, B> {
    Unit,
    Bool(bool),
    Char(char),
    I64Class((u8, i64)),
    U64Class((u8, u64)),
    F32(u32),
    F64(u64),
    String(S),
    Bytes(B),
    HeapIdentity(crate::heap::GcRef),
    HostIdentity(HostSlotRef),
}

impl ValueKey {
    fn canonical_parts(&self) -> KeyParts<&str, &[u8]> {
        match self {
            Self::Unit => KeyParts::Unit,
            Self::Bool(value) => KeyParts::Bool(*value),
            Self::Char(value) => KeyParts::Char(*value),
            Self::I8(value) => KeyParts::I64Class((8, i64::from(*value))),
            Self::I16(value) => KeyParts::I64Class((16, i64::from(*value))),
            Self::I32(value) => KeyParts::I64Class((32, i64::from(*value))),
            Self::I64(value) => KeyParts::I64Class((64, *value)),
            Self::U8(value) => KeyParts::U64Class((8, u64::from(*value))),
            Self::U16(value) => KeyParts::U64Class((16, u64::from(*value))),
            Self::U32(value) => KeyParts::U64Class((32, u64::from(*value))),
            Self::U64(value) => KeyParts::U64Class((64, *value)),
            Self::F32(value) => KeyParts::F32(*value),
            Self::F64(value) => KeyParts::F64(*value),
            Self::String(value) => KeyParts::String(value.as_str()),
            Self::Bytes(value) => KeyParts::Bytes(value.as_slice()),
            Self::HeapIdentity(reference) => KeyParts::HeapIdentity(*reference),
            Self::HostIdentity(reference) => KeyParts::HostIdentity(*reference),
        }
    }
}

impl Hash for ValueKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_key_parts!(state, self.canonical_parts(), &str, &[u8]);
    }
}

/// Borrowed lookup key built directly against heap storage.
///
/// `ValueKey::from_value` clones string and bytes payloads out of the heap,
/// which put one allocation on every map and set operation. A probe borrows
/// them instead, hashes identically to `ValueKey` through the shared canonical
/// scheme, and is converted to an owned key only when an insert actually
/// creates a new entry.
#[derive(Clone, Copy, Debug)]
pub(crate) enum KeyProbe<'a> {
    Unit,
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
    String(&'a str),
    Bytes(&'a [u8]),
    HeapIdentity(crate::heap::GcRef),
    HostIdentity(HostSlotRef),
}

impl<'a> KeyProbe<'a> {
    pub(crate) fn from_value(
        value: &Value,
        heap: Option<&'a HeapExecution<'_>>,
        operation: &'static str,
    ) -> VmResult<Self> {
        match value {
            Value::Missing => type_error(operation),
            Value::Unit => Ok(Self::Unit),
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
            Value::HeapRef(reference) => match heap.and_then(|heap| heap.heap.get(*reference)) {
                Some(HeapValue::String(value)) => Ok(Self::String(value)),
                Some(HeapValue::Bytes(value)) => Ok(Self::Bytes(value)),
                Some(HeapValue::Tuple(_) | HeapValue::Range(_) | HeapValue::PathProxy(_))
                | None => type_error(operation),
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

    /// Compares this probe against a stored owned key.
    #[must_use]
    pub(crate) fn matches(&self, key: &ValueKey) -> bool {
        match (self, key) {
            (Self::Unit, ValueKey::Unit) => true,
            (Self::Bool(probe), ValueKey::Bool(key)) => probe == key,
            (Self::Char(probe), ValueKey::Char(key)) => probe == key,
            (Self::I8(probe), ValueKey::I8(key)) => probe == key,
            (Self::I16(probe), ValueKey::I16(key)) => probe == key,
            (Self::I32(probe), ValueKey::I32(key)) => probe == key,
            (Self::I64(probe), ValueKey::I64(key)) => probe == key,
            (Self::U8(probe), ValueKey::U8(key)) => probe == key,
            (Self::U16(probe), ValueKey::U16(key)) => probe == key,
            (Self::U32(probe), ValueKey::U32(key)) => probe == key,
            (Self::U64(probe), ValueKey::U64(key)) => probe == key,
            (Self::F32(probe), ValueKey::F32(key)) => probe == key,
            (Self::F64(probe), ValueKey::F64(key)) => probe == key,
            (Self::String(probe), ValueKey::String(key)) => *probe == key.as_str(),
            (Self::Bytes(probe), ValueKey::Bytes(key)) => *probe == key.as_slice(),
            (Self::HeapIdentity(probe), ValueKey::HeapIdentity(key)) => probe == key,
            (Self::HostIdentity(probe), ValueKey::HostIdentity(key)) => probe == key,
            _ => false,
        }
    }

    /// Clones this probe into the owned key an insert stores.
    #[must_use]
    pub(crate) fn to_owned_key(self) -> ValueKey {
        match self {
            Self::Unit => ValueKey::Unit,
            Self::Bool(value) => ValueKey::Bool(value),
            Self::Char(value) => ValueKey::Char(value),
            Self::I8(value) => ValueKey::I8(value),
            Self::I16(value) => ValueKey::I16(value),
            Self::I32(value) => ValueKey::I32(value),
            Self::I64(value) => ValueKey::I64(value),
            Self::U8(value) => ValueKey::U8(value),
            Self::U16(value) => ValueKey::U16(value),
            Self::U32(value) => ValueKey::U32(value),
            Self::U64(value) => ValueKey::U64(value),
            Self::F32(value) => ValueKey::F32(value),
            Self::F64(value) => ValueKey::F64(value),
            Self::String(value) => ValueKey::String(value.to_owned()),
            Self::Bytes(value) => ValueKey::Bytes(value.to_vec()),
            Self::HeapIdentity(reference) => ValueKey::HeapIdentity(reference),
            Self::HostIdentity(reference) => ValueKey::HostIdentity(reference),
        }
    }

    fn canonical_parts(&self) -> KeyParts<&'a str, &'a [u8]> {
        match self {
            Self::Unit => KeyParts::Unit,
            Self::Bool(value) => KeyParts::Bool(*value),
            Self::Char(value) => KeyParts::Char(*value),
            Self::I8(value) => KeyParts::I64Class((8, i64::from(*value))),
            Self::I16(value) => KeyParts::I64Class((16, i64::from(*value))),
            Self::I32(value) => KeyParts::I64Class((32, i64::from(*value))),
            Self::I64(value) => KeyParts::I64Class((64, *value)),
            Self::U8(value) => KeyParts::U64Class((8, u64::from(*value))),
            Self::U16(value) => KeyParts::U64Class((16, u64::from(*value))),
            Self::U32(value) => KeyParts::U64Class((32, u64::from(*value))),
            Self::U64(value) => KeyParts::U64Class((64, *value)),
            Self::F32(value) => KeyParts::F32(*value),
            Self::F64(value) => KeyParts::F64(*value),
            Self::String(value) => KeyParts::String(value),
            Self::Bytes(value) => KeyParts::Bytes(value),
            Self::HeapIdentity(reference) => KeyParts::HeapIdentity(*reference),
            Self::HostIdentity(reference) => KeyParts::HostIdentity(*reference),
        }
    }
}

impl Hash for KeyProbe<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_key_parts!(state, self.canonical_parts(), &str, &[u8]);
    }
}

fn finite_f32_key(value: f32, operation: &'static str) -> VmResult<u32> {
    if !value.is_finite() {
        return type_error(operation);
    }
    Ok(if value == 0.0 {
        0.0f32.to_bits()
    } else {
        value.to_bits()
    })
}

fn finite_f64_key(value: f64, operation: &'static str) -> VmResult<u64> {
    if !value.is_finite() {
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
    use vela_host::path::{HostRef, HostSlotRef};
    use vela_host::proxy::PathProxy;
    use vela_host::target::HostTargetPlan;

    use crate::heap::{HeapValue, RecordIdentity, ScriptHeap};
    use crate::script_object::ScriptFields;
    use crate::{HeapExecution, Value, VmErrorKind};

    use super::ValueKey;

    #[test]
    fn value_key_accepts_leaf_values_by_exact_value() {
        assert_eq!(key(&Value::Unit), ValueKey::Unit);
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
        assert_type_mismatch(&Value::F32(f32::INFINITY));
        assert_type_mismatch(&Value::F32(f32::NEG_INFINITY));
        assert_type_mismatch(&Value::F64(f64::NAN));
        assert_type_mismatch(&Value::F64(f64::INFINITY));
        assert_type_mismatch(&Value::F64(f64::NEG_INFINITY));
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
            ValueKey::from_value(&Value::HeapRef(string), Some(&heap), "test")
                .expect("string heap value should key by payload"),
            ValueKey::String("player".to_owned())
        );
        assert_eq!(
            ValueKey::from_value(&Value::HeapRef(bytes), Some(&heap), "test")
                .expect("bytes heap value should key by payload"),
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
            ValueKey::from_value(&Value::HeapRef(first), Some(&heap), "test")
                .expect("record heap value should key by identity"),
            ValueKey::HeapIdentity(first)
        );
        assert_ne!(
            ValueKey::from_value(&Value::HeapRef(first), Some(&heap), "test")
                .expect("record heap value should key by identity"),
            ValueKey::from_value(&Value::HeapRef(second), Some(&heap), "test")
                .expect("record heap value should key by identity")
        );
    }

    #[test]
    fn value_key_uses_host_ref_identity() {
        let first = HostSlotRef::new(7, 1);
        let second = HostSlotRef::new(7, 2);

        assert_eq!(key(&Value::HostRef(first)), ValueKey::HostIdentity(first));
        assert_ne!(key(&Value::HostRef(first)), key(&Value::HostRef(second)));
    }

    #[test]
    fn value_key_rejects_transient_values() {
        assert_type_mismatch(&Value::Missing);

        let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(7), 1);
        let plan = HostTargetPlan::new(host_ref.type_id);
        let mut heap = ScriptHeap::new();
        let path_proxy = heap.allocate(HeapValue::PathProxy(PathProxy::new(host_ref, plan)));
        let range = heap.allocate(HeapValue::Range(crate::ranges::RangeValue::new(
            0, 1, false,
        )));
        let heap = HeapExecution::new(&mut heap);

        let error = ValueKey::from_value(&Value::HeapRef(range), Some(&heap), "test")
            .expect_err("heap ranges are not stable keys");
        assert_eq!(
            error.kind(),
            VmErrorKind::TypeMismatch { operation: "test" }
        );

        let error = ValueKey::from_value(&Value::HeapRef(path_proxy), Some(&heap), "test")
            .expect_err("path proxies are not stable keys");
        assert_eq!(
            error.kind(),
            VmErrorKind::TypeMismatch { operation: "test" }
        );
    }

    fn key(value: &Value) -> ValueKey {
        ValueKey::from_value(value, None, "test").expect("value should be keyable")
    }

    fn assert_type_mismatch(value: &Value) {
        let error = ValueKey::from_value(value, None, "test")
            .expect_err("unsupported transient value should be rejected");
        assert_eq!(
            error.kind(),
            VmErrorKind::TypeMismatch { operation: "test" }
        );
    }

    fn record(type_name: &str) -> HeapValue {
        HeapValue::Record {
            identity: Some(RecordIdentity::new(TypeId::new(1), ShapeId::new(1))),
            fields: ScriptFields::from_pairs(
                type_name,
                BTreeMap::from([("id".to_owned(), Value::I64(1))]),
            ),
        }
    }
}
