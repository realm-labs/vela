use crate::heap::{GcRef, HeapValue};
use crate::{HeapExecution, Value, VmError, VmErrorKind, VmResult};

pub(crate) fn values_equal(
    lhs: &Value,
    rhs: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<bool> {
    if let Some(equal) = leaf_values_equal(lhs, rhs, heap)? {
        return Ok(equal);
    }
    non_comparable("equal")
}

pub(crate) fn values_not_equal(
    lhs: &Value,
    rhs: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<bool> {
    values_equal(lhs, rhs, heap).map(|equal| !equal)
}

pub(crate) fn identity_equal(
    lhs: &Value,
    rhs: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<bool> {
    match (identity_key(lhs, heap)?, identity_key(rhs, heap)?) {
        (IdentityKey::Heap(lhs), IdentityKey::Heap(rhs)) => Ok(lhs == rhs),
        (IdentityKey::Host(lhs), IdentityKey::Host(rhs)) => Ok(lhs == rhs),
        (IdentityKey::Heap(_), IdentityKey::Host(_))
        | (IdentityKey::Host(_), IdentityKey::Heap(_)) => Ok(false),
    }
}

pub(crate) fn identity_not_equal(
    lhs: &Value,
    rhs: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<bool> {
    identity_equal(lhs, rhs, heap).map(|equal| !equal)
}

pub(crate) fn simple_values_equal(
    lhs: &Value,
    rhs: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Option<bool>> {
    leaf_values_equal(lhs, rhs, heap)
}

fn leaf_values_equal(
    lhs: &Value,
    rhs: &Value,
    heap: Option<&HeapExecution<'_>>,
) -> VmResult<Option<bool>> {
    if let Some(equal) = immediate_leaf_values_equal(lhs, rhs) {
        return Ok(Some(equal));
    }

    match (heap_leaf(lhs, heap)?, heap_leaf(rhs, heap)?) {
        (Some(HeapLeaf::String(lhs)), Some(HeapLeaf::String(rhs))) => Ok(Some(lhs == rhs)),
        (Some(HeapLeaf::Bytes(lhs)), Some(HeapLeaf::Bytes(rhs))) => Ok(Some(lhs == rhs)),
        (Some(_), Some(_)) => Ok(Some(false)),
        (Some(_), None) | (None, Some(_)) if is_immediate_comparable_leaf(lhs, rhs) => {
            Ok(Some(false))
        }
        (Some(_), None) | (None, Some(_)) => Ok(None),
        (None, None) => Ok(None),
    }
}

fn immediate_leaf_values_equal(lhs: &Value, rhs: &Value) -> Option<bool> {
    match (lhs, rhs) {
        (Value::Missing, _) | (_, Value::Missing) => None,
        (Value::Null, Value::Null) => Some(true),
        (Value::Bool(lhs), Value::Bool(rhs)) => Some(lhs == rhs),
        (Value::Char(lhs), Value::Char(rhs)) => Some(lhs == rhs),
        (Value::Range(lhs), Value::Range(rhs)) => Some(lhs == rhs),
        (lhs, rhs) if lhs.is_scalar() && rhs.is_scalar() => {
            Some(lhs.as_scalar() == rhs.as_scalar())
        }
        (lhs, rhs)
            if is_immediate_comparable_leaf(lhs, rhs)
                && (is_immediate_leaf(lhs) || is_immediate_leaf(rhs)) =>
        {
            Some(false)
        }
        _ => None,
    }
}

fn is_immediate_comparable_leaf(lhs: &Value, rhs: &Value) -> bool {
    is_immediate_leaf(lhs) || is_immediate_leaf(rhs)
}

fn is_immediate_leaf(value: &Value) -> bool {
    matches!(
        value,
        Value::Null
            | Value::Bool(_)
            | Value::Char(_)
            | Value::I8(_)
            | Value::I16(_)
            | Value::I32(_)
            | Value::I64(_)
            | Value::U8(_)
            | Value::U16(_)
            | Value::U32(_)
            | Value::U64(_)
            | Value::F32(_)
            | Value::F64(_)
            | Value::Range(_)
    )
}

fn heap_leaf<'a>(
    value: &'a Value,
    heap: Option<&'a HeapExecution<'_>>,
) -> VmResult<Option<HeapLeaf<'a>>> {
    let Value::HeapRef(reference) = value else {
        return Ok(None);
    };
    let Some(heap_value) = heap.and_then(|heap| heap.heap.get(*reference)) else {
        return non_comparable("equal");
    };
    match heap_value {
        HeapValue::String(value) => Ok(Some(HeapLeaf::String(value))),
        HeapValue::Bytes(value) => Ok(Some(HeapLeaf::Bytes(value))),
        HeapValue::PathProxy(_) => non_comparable("equal"),
        HeapValue::Array(_)
        | HeapValue::Map(_)
        | HeapValue::Set(_)
        | HeapValue::Record { .. }
        | HeapValue::Enum { .. }
        | HeapValue::Closure(_)
        | HeapValue::Iterator(_) => Ok(None),
    }
}

fn identity_key(value: &Value, heap: Option<&HeapExecution<'_>>) -> VmResult<IdentityKey> {
    match value {
        Value::HeapRef(reference) => heap_identity_key(*reference, heap),
        Value::HostRef(reference) => Ok(IdentityKey::Host(*reference)),
        Value::Missing => non_comparable("identity equal"),
        Value::Null
        | Value::Bool(_)
        | Value::Char(_)
        | Value::I8(_)
        | Value::I16(_)
        | Value::I32(_)
        | Value::I64(_)
        | Value::U8(_)
        | Value::U16(_)
        | Value::U32(_)
        | Value::U64(_)
        | Value::F32(_)
        | Value::F64(_)
        | Value::Range(_) => non_comparable("identity equal"),
    }
}

fn heap_identity_key(reference: GcRef, heap: Option<&HeapExecution<'_>>) -> VmResult<IdentityKey> {
    let Some(heap_value) = heap.and_then(|heap| heap.heap.get(reference)) else {
        return non_comparable("identity equal");
    };
    match heap_value {
        HeapValue::Array(_)
        | HeapValue::Map(_)
        | HeapValue::Set(_)
        | HeapValue::Record { .. }
        | HeapValue::Enum { .. }
        | HeapValue::Closure(_)
        | HeapValue::Iterator(_) => Ok(IdentityKey::Heap(reference)),
        HeapValue::String(_) | HeapValue::Bytes(_) | HeapValue::PathProxy(_) => {
            non_comparable("identity equal")
        }
    }
}

fn non_comparable<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}

enum HeapLeaf<'a> {
    String(&'a str),
    Bytes(&'a [u8]),
}

enum IdentityKey {
    Heap(GcRef),
    Host(vela_host::path::HostRef),
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use vela_common::{HostObjectId, HostTypeId, ShapeId};
    use vela_def::TypeId;
    use vela_host::path::HostRef;
    use vela_host::proxy::PathProxy;
    use vela_host::target::HostTargetPlan;

    use crate::heap::{RecordIdentity, ScriptHeap};
    use crate::ranges::RangeValue;
    use crate::script_object::ScriptFields;

    use super::*;

    #[test]
    fn semantic_equality_is_tag_exact_for_leaf_values() {
        assert_eq!(equal(Value::Null, Value::Null), Ok(true));
        assert_eq!(equal(Value::Bool(true), Value::Bool(false)), Ok(false));
        assert_eq!(equal(Value::Char('v'), Value::Char('v')), Ok(true));
        assert_eq!(equal(Value::I64(1), Value::I64(1)), Ok(true));
        assert_eq!(equal(Value::I64(1), Value::U64(1)), Ok(false));
        assert_eq!(equal(Value::F64(f64::NAN), Value::F64(f64::NAN)), Ok(false));
        assert_eq!(equal(Value::F64(-0.0), Value::F64(0.0)), Ok(true));
        assert_eq!(
            equal(
                Value::Range(RangeValue::new(0, 10, false)),
                Value::Range(RangeValue::new(0, 10, false))
            ),
            Ok(true)
        );
    }

    #[test]
    fn semantic_equality_compares_string_and_bytes_payloads() {
        let mut heap = ScriptHeap::new();
        let left = Value::HeapRef(heap.allocate(HeapValue::String("gold".to_owned())));
        let right = Value::HeapRef(heap.allocate(HeapValue::String("gold".to_owned())));
        let bytes = Value::HeapRef(heap.allocate(HeapValue::Bytes(vec![1, 2, 3])));
        let same_bytes = Value::HeapRef(heap.allocate(HeapValue::Bytes(vec![1, 2, 3])));
        let heap = HeapExecution::new(&mut heap);

        assert_eq!(values_equal(&left, &right, Some(&heap)), Ok(true));
        assert_eq!(values_equal(&bytes, &same_bytes, Some(&heap)), Ok(true));
        assert_eq!(values_equal(&left, &bytes, Some(&heap)), Ok(false));
        assert_eq!(values_equal(&left, &Value::I64(1), Some(&heap)), Ok(false));
    }

    #[test]
    fn semantic_equality_rejects_objects_without_partial_eq() {
        let mut heap = ScriptHeap::new();
        let array = Value::HeapRef(heap.allocate(HeapValue::Array(Vec::new())));
        let record = Value::HeapRef(heap.allocate(record("Reward")));
        let heap = HeapExecution::new(&mut heap);

        assert_type_mismatch(values_equal(&array, &array, Some(&heap)), "equal");
        assert_type_mismatch(values_equal(&record, &record, Some(&heap)), "equal");
    }

    #[test]
    fn semantic_equality_rejects_missing_and_path_proxy() {
        assert_type_mismatch(
            values_equal(&Value::Missing, &Value::Missing, None),
            "equal",
        );

        let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(7), 1);
        let plan = HostTargetPlan::new(host_ref.type_id);
        let mut heap = ScriptHeap::new();
        let proxy =
            Value::HeapRef(heap.allocate(HeapValue::PathProxy(PathProxy::new(host_ref, plan))));
        let heap = HeapExecution::new(&mut heap);

        assert_type_mismatch(values_equal(&proxy, &proxy, Some(&heap)), "equal");
    }

    #[test]
    fn identity_equality_accepts_only_identity_values() {
        let mut heap = ScriptHeap::new();
        let first = Value::HeapRef(heap.allocate(record("Reward")));
        let second = Value::HeapRef(heap.allocate(record("Reward")));
        let string = Value::HeapRef(heap.allocate(HeapValue::String("Reward".to_owned())));
        let heap = HeapExecution::new(&mut heap);

        assert_eq!(identity_equal(&first, &first, Some(&heap)), Ok(true));
        assert_eq!(identity_equal(&first, &second, Some(&heap)), Ok(false));
        assert_type_mismatch(
            identity_equal(&string, &string, Some(&heap)),
            "identity equal",
        );
        assert_type_mismatch(
            identity_equal(&Value::I64(1), &Value::I64(1), Some(&heap)),
            "identity equal",
        );
    }

    #[test]
    fn identity_equality_compares_host_refs_without_host_reads() {
        let first = HostRef::new(HostTypeId::new(1), HostObjectId::new(7), 1);
        let same = HostRef::new(HostTypeId::new(1), HostObjectId::new(7), 1);
        let stale = HostRef::new(HostTypeId::new(1), HostObjectId::new(7), 2);

        assert_eq!(
            identity_equal(&Value::HostRef(first), &Value::HostRef(same), None),
            Ok(true)
        );
        assert_eq!(
            identity_equal(&Value::HostRef(first), &Value::HostRef(stale), None),
            Ok(false)
        );
    }

    fn equal(lhs: Value, rhs: Value) -> VmResult<bool> {
        values_equal(&lhs, &rhs, None)
    }

    fn assert_type_mismatch(result: VmResult<bool>, operation: &'static str) {
        let error = result.expect_err("operation should reject non-comparable value");
        assert_eq!(error.kind(), VmErrorKind::TypeMismatch { operation });
    }

    fn record(type_name: &str) -> HeapValue {
        HeapValue::Record {
            type_name: type_name.to_owned(),
            identity: Some(RecordIdentity::new(TypeId::new(1), ShapeId::new(1))),
            fields: ScriptFields::from(BTreeMap::from([("id".to_owned(), Value::I64(1))])),
        }
    }
}
