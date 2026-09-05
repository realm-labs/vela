use crate::heap::{HeapValue, ScriptHeap};
use crate::persistent_value_to_owned;
use crate::value::Value;

use super::{MAX_BYTES, MAX_DEPTH, MAX_VALUES, OwnedValue, VmErrorKind};

#[test]
fn cyclic_owned_export_returns_a_typed_error() {
    let mut heap = ScriptHeap::new();
    let root = heap.allocate(HeapValue::Array(vec![]));
    let child = heap.allocate(HeapValue::Tuple(vec![Value::HeapRef(root)]));
    let HeapValue::Array(values) = heap.get_mut(root).expect("root should remain live") else {
        panic!()
    };
    values.push(Value::HeapRef(child));
    let error = persistent_value_to_owned(&Value::HeapRef(root), &mut heap)
        .expect_err("cyclic or over-limit export must reject");
    assert_eq!(error.kind(), VmErrorKind::OwnedValueCycle);
}

#[test]
fn shared_values_are_exported_as_independent_tree_branches() {
    let mut heap = ScriptHeap::new();
    let child = heap.allocate(HeapValue::Array(vec![Value::i64(7)]));
    let root = heap.allocate(HeapValue::Array(vec![Value::HeapRef(child); 2]));
    assert_eq!(
        persistent_value_to_owned(&Value::HeapRef(root), &mut heap)
            .expect("valid export fixture operation should succeed"),
        OwnedValue::array([OwnedValue::array([7_i64]), OwnedValue::array([7_i64])])
    );
}

#[test]
fn shared_graph_expansion_is_bounded() {
    let mut heap = ScriptHeap::new();
    let mut root = heap.allocate(HeapValue::Array(vec![]));
    for _ in 0..20 {
        root = heap.allocate(HeapValue::Array(vec![Value::HeapRef(root); 2]));
    }
    let error = persistent_value_to_owned(&Value::HeapRef(root), &mut heap)
        .expect_err("cyclic or over-limit export must reject");
    assert!(matches!(
        error.kind(),
        VmErrorKind::OwnedValueLimitExceeded { .. }
    ));
}

#[test]
fn deep_owned_export_stops_before_recursive_conversion_or_drop_overflows() {
    let mut heap = ScriptHeap::new();
    let mut root = heap.allocate(HeapValue::Array(vec![]));
    for _ in 1..MAX_DEPTH {
        root = heap.allocate(HeapValue::Array(vec![Value::HeapRef(root)]));
    }
    drop(
        persistent_value_to_owned(&Value::HeapRef(root), &mut heap)
            .expect("valid export fixture operation should succeed"),
    );
    root = heap.allocate(HeapValue::Array(vec![Value::HeapRef(root)]));
    assert_eq!(
        persistent_value_to_owned(&Value::HeapRef(root), &mut heap)
            .expect_err("cyclic or over-limit export must reject")
            .kind(),
        VmErrorKind::OwnedValueLimitExceeded {
            resource: "depth",
            limit: MAX_DEPTH
        }
    );
}

#[test]
fn flat_owned_export_checks_count_before_reserving_storage() {
    let mut heap = ScriptHeap::new();
    let root = heap.allocate(HeapValue::Array(vec![Value::Unit; MAX_VALUES]));
    assert_eq!(
        persistent_value_to_owned(&Value::HeapRef(root), &mut heap)
            .expect_err("cyclic or over-limit export must reject")
            .kind(),
        VmErrorKind::OwnedValueLimitExceeded {
            resource: "values",
            limit: MAX_VALUES
        }
    );
}

#[test]
fn owned_export_charges_repeated_payload_bytes() {
    let mut heap = ScriptHeap::new();
    let payload = heap.allocate(HeapValue::Bytes(vec![0; MAX_BYTES / 2]));
    let root = heap.allocate(HeapValue::Array(vec![Value::HeapRef(payload); 2]));
    assert_eq!(
        persistent_value_to_owned(&Value::HeapRef(root), &mut heap)
            .expect_err("cyclic or over-limit export must reject")
            .kind(),
        VmErrorKind::OwnedValueLimitExceeded {
            resource: "bytes",
            limit: MAX_BYTES
        }
    );
}
