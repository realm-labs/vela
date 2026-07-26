use std::mem;

use vela_common::{HostObjectId, HostTypeId};
use vela_def::FieldId;
use vela_host::path::{HostPath, HostRef};
use vela_host::proxy::PathProxy;

use crate::budget::CollectionLimits;
use crate::heap::{HeapValue, ScriptHeap};
use crate::script_set::ScriptSet;
use crate::{ExecutionBudget, HeapExecution, Value, VmErrorKind};

use super::{
    extend_map_slots, extend_set_slots, insert_map_slot, push_array_slot, push_set_slot,
    retain_array_slots,
};

#[test]
fn array_push_charges_container_slot_growth() {
    let mut heap = ScriptHeap::new();
    let reference = heap.allocate(HeapValue::Array(Vec::new()));
    let initial_bytes = heap.allocated_bytes();
    let mut heap_execution = HeapExecution::new(&mut heap);
    let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX);

    push_array_slot(
        &mut heap_execution,
        reference,
        Value::I64(10),
        Some(&mut budget),
        "test array push",
    )
    .expect("array push should fit");

    assert!(heap_execution.heap.allocated_bytes() > initial_bytes);
    assert_eq!(
        heap_execution.heap.allocated_bytes() - initial_bytes,
        budget.memory_bytes_allocated()
    );
}

#[test]
fn unbounded_budget_skips_collection_growth_accounting() {
    let mut heap = ScriptHeap::new();
    let reference = heap.allocate(HeapValue::Array(Vec::new()));
    let initial_bytes = heap.allocated_bytes();
    let mut heap_execution = HeapExecution::new(&mut heap);
    let mut budget = ExecutionBudget::unbounded();

    push_array_slot(
        &mut heap_execution,
        reference,
        Value::I64(10),
        Some(&mut budget),
        "test array push",
    )
    .expect("array push should fit");

    assert_eq!(heap_execution.heap.allocated_bytes(), initial_bytes);
    assert_eq!(budget.memory_bytes_allocated(), 0);
    assert_eq!(
        heap_execution.heap.get(reference),
        Some(&HeapValue::Array(vec![Value::I64(10)]))
    );
}

#[test]
fn array_push_rejects_memory_growth_before_mutation() {
    let mut heap = ScriptHeap::new();
    let reference = heap.allocate(HeapValue::Array(Vec::new()));
    let initial_bytes = heap.allocated_bytes();
    let mut heap_execution = HeapExecution::new(&mut heap);
    let mut budget = ExecutionBudget::new(u64::MAX, 1, usize::MAX);

    let error = push_array_slot(
        &mut heap_execution,
        reference,
        Value::I64(10),
        Some(&mut budget),
        "test array push",
    )
    .expect_err("array push should exceed memory budget");

    assert!(matches!(
        error.kind_ref(),
        VmErrorKind::BudgetExceeded { .. }
    ));
    assert_eq!(heap_execution.heap.allocated_bytes(), initial_bytes);
    assert_eq!(
        heap_execution.heap.get(reference),
        Some(&HeapValue::Array(Vec::new()))
    );
}

#[test]
fn array_retain_rejects_budget_and_stale_snapshot_before_mutation() {
    let mut heap = ScriptHeap::new();
    let reference = heap.allocate(HeapValue::Array(vec![
        Value::I64(1),
        Value::I64(2),
        Value::I64(3),
    ]));
    let mut heap_execution = HeapExecution::new(&mut heap);
    let mut budget = ExecutionBudget::new(2, usize::MAX, usize::MAX);

    let budget_error = retain_array_slots(
        &mut heap_execution,
        reference,
        3,
        &[true, false, true],
        Some(&mut budget),
        "test array retain",
    )
    .expect_err("the complete retain traversal must be precharged");
    assert!(matches!(
        budget_error.kind_ref(),
        VmErrorKind::BudgetExceeded { .. }
    ));
    assert_eq!(
        heap_execution.heap.get(reference),
        Some(&HeapValue::Array(vec![
            Value::I64(1),
            Value::I64(2),
            Value::I64(3)
        ]))
    );

    let stale_error = retain_array_slots(
        &mut heap_execution,
        reference,
        2,
        &[true, false],
        None,
        "test array retain",
    )
    .expect_err("a changed sequence snapshot must reject callback decisions");
    assert!(matches!(
        stale_error.kind_ref(),
        VmErrorKind::CollectionChangedDuringCallback {
            operation: "test array retain"
        }
    ));
    assert_eq!(
        heap_execution.heap.get(reference),
        Some(&HeapValue::Array(vec![
            Value::I64(1),
            Value::I64(2),
            Value::I64(3)
        ]))
    );
}

#[test]
fn map_insert_rejects_entry_limit_before_mutation() {
    let mut heap = ScriptHeap::new();
    let reference = heap.allocate(HeapValue::Map(Default::default()));
    let mut heap_execution = HeapExecution::new(&mut heap);
    let mut budget = ExecutionBudget::unbounded().with_collection_limits(CollectionLimits {
        max_array_len: usize::MAX,
        max_map_entries: 0,
        max_set_len: usize::MAX,
    });

    let error = insert_map_slot(
        &mut heap_execution,
        reference,
        Value::I64(1),
        Value::I64(10),
        Some(&mut budget),
        "test map set",
    )
    .expect_err("map insert should exceed entry limit");

    assert!(matches!(
        error.kind_ref(),
        VmErrorKind::CollectionLimitExceeded {
            collection: "map",
            limit: 0
        }
    ));
    assert_eq!(
        heap_execution.heap.get(reference),
        Some(&HeapValue::Map(Default::default()))
    );
}

#[test]
fn map_insert_rejects_infinite_float_key_before_mutation() {
    let mut heap = ScriptHeap::new();
    let reference = heap.allocate(HeapValue::Map(Default::default()));
    let mut heap_execution = HeapExecution::new(&mut heap);

    let error = insert_map_slot(
        &mut heap_execution,
        reference,
        Value::F64(f64::INFINITY),
        Value::I64(10),
        None,
        "test map set",
    )
    .expect_err("infinite map keys should be rejected");

    assert_eq!(
        error.kind_ref(),
        &VmErrorKind::TypeMismatch {
            operation: "test map set"
        }
    );
    assert_eq!(
        heap_execution.heap.get(reference),
        Some(&HeapValue::Map(Default::default()))
    );
}

#[test]
fn map_insert_rejects_transient_keys_before_mutation() {
    for key in transient_values() {
        let mut heap = ScriptHeap::new();
        let key = key(&mut heap);
        let reference = heap.allocate(HeapValue::Map(Default::default()));
        let mut heap_execution = HeapExecution::new(&mut heap);

        let error = insert_map_slot(
            &mut heap_execution,
            reference,
            key,
            Value::I64(10),
            None,
            "test map set",
        )
        .expect_err("transient map keys should be rejected");

        assert_eq!(
            error.kind_ref(),
            &VmErrorKind::TypeMismatch {
                operation: "test map set"
            }
        );
        assert_eq!(
            heap_execution.heap.get(reference),
            Some(&HeapValue::Map(Default::default()))
        );
    }
}

#[test]
fn map_extend_duplicate_new_key_counts_once_and_preserves_key() {
    let mut heap = ScriptHeap::new();
    let reference = heap.allocate(HeapValue::Map(Default::default()));
    let mut heap_execution = HeapExecution::new(&mut heap);
    let mut budget = ExecutionBudget::unbounded().with_collection_limits(CollectionLimits {
        max_array_len: usize::MAX,
        max_map_entries: 1,
        max_set_len: usize::MAX,
    });

    extend_map_slots(
        &mut heap_execution,
        reference,
        vec![
            (Value::F64(-0.0), Value::I64(10)),
            (Value::F64(0.0), Value::I64(20)),
        ],
        Some(&mut budget),
        "test map extend",
    )
    .expect("duplicate new map key should count as one entry");

    let Some(HeapValue::Map(values)) = heap_execution.heap.get(reference) else {
        panic!("expected map value");
    };
    let entries = values.entries_vec();
    assert_eq!(entries.len(), 1);
    let (key, value) = entries[0];
    let Value::F64(key) = key else {
        panic!("stored map key should remain f64");
    };
    assert!(
        key.is_sign_negative(),
        "duplicate map insertion must preserve the first stored key"
    );
    assert_eq!(value, Value::I64(20));
}

#[test]
fn set_add_rejects_length_limit_before_mutation() {
    let mut heap = ScriptHeap::new();
    let reference = heap.allocate(HeapValue::Set(ScriptSet::new()));
    let mut heap_execution = HeapExecution::new(&mut heap);
    let mut budget = ExecutionBudget::unbounded().with_collection_limits(CollectionLimits {
        max_array_len: usize::MAX,
        max_map_entries: usize::MAX,
        max_set_len: 0,
    });

    let error = push_set_slot(
        &mut heap_execution,
        reference,
        Value::I64(10),
        Some(&mut budget),
        "test set add",
    )
    .expect_err("set add should exceed length limit");

    assert!(matches!(
        error.kind_ref(),
        VmErrorKind::CollectionLimitExceeded {
            collection: "set",
            limit: 0
        }
    ));
    assert_eq!(
        heap_execution.heap.get(reference),
        Some(&HeapValue::Set(ScriptSet::new()))
    );
}

#[test]
fn set_add_rejects_infinite_float_key_before_mutation() {
    let mut heap = ScriptHeap::new();
    let reference = heap.allocate(HeapValue::Set(ScriptSet::new()));
    let mut heap_execution = HeapExecution::new(&mut heap);

    let error = push_set_slot(
        &mut heap_execution,
        reference,
        Value::F32(f32::NEG_INFINITY),
        None,
        "test set add",
    )
    .expect_err("infinite set elements should be rejected");

    assert_eq!(
        error.kind_ref(),
        &VmErrorKind::TypeMismatch {
            operation: "test set add"
        }
    );
    assert_eq!(
        heap_execution.heap.get(reference),
        Some(&HeapValue::Set(ScriptSet::new()))
    );
}

#[test]
fn set_add_rejects_transient_elements_before_mutation() {
    for value in transient_values() {
        let mut heap = ScriptHeap::new();
        let value = value(&mut heap);
        let reference = heap.allocate(HeapValue::Set(ScriptSet::new()));
        let mut heap_execution = HeapExecution::new(&mut heap);

        let error = push_set_slot(&mut heap_execution, reference, value, None, "test set add")
            .expect_err("transient set elements should be rejected");

        assert_eq!(
            error.kind_ref(),
            &VmErrorKind::TypeMismatch {
                operation: "test set add"
            }
        );
        assert_eq!(
            heap_execution.heap.get(reference),
            Some(&HeapValue::Set(ScriptSet::new()))
        );
    }
}

#[test]
fn set_add_rejects_value_key_payload_memory_before_mutation() {
    let mut heap = ScriptHeap::new();
    let value = Value::HeapRef(heap.allocate(HeapValue::String("large-key".to_owned())));
    let reference = heap.allocate(HeapValue::Set(ScriptSet::new()));
    let mut heap_execution = HeapExecution::new(&mut heap);
    let mut budget = ExecutionBudget::new(u64::MAX, mem::size_of::<Value>(), usize::MAX);

    let error = push_set_slot(
        &mut heap_execution,
        reference,
        value,
        Some(&mut budget),
        "test set add",
    )
    .expect_err("set add should exceed key payload memory before mutation");

    assert!(matches!(
        error.kind_ref(),
        VmErrorKind::BudgetExceeded { .. }
    ));
    assert_eq!(
        heap_execution.heap.get(reference),
        Some(&HeapValue::Set(ScriptSet::new()))
    );
}

#[test]
fn set_extend_rejects_value_key_payload_memory_before_mutation() {
    let mut heap = ScriptHeap::new();
    let values = vec![
        Value::HeapRef(heap.allocate(HeapValue::String("large-a".to_owned()))),
        Value::HeapRef(heap.allocate(HeapValue::String("large-b".to_owned()))),
    ];
    let reference = heap.allocate(HeapValue::Set(ScriptSet::new()));
    let mut heap_execution = HeapExecution::new(&mut heap);
    let mut budget =
        ExecutionBudget::new(u64::MAX, mem::size_of::<Value>() * values.len(), usize::MAX);

    let error = extend_set_slots(
        &mut heap_execution,
        reference,
        values,
        Some(&mut budget),
        "test set extend",
    )
    .expect_err("set extend should exceed key payload memory before mutation");

    assert!(matches!(
        error.kind_ref(),
        VmErrorKind::BudgetExceeded { .. }
    ));
    assert_eq!(
        heap_execution.heap.get(reference),
        Some(&HeapValue::Set(ScriptSet::new()))
    );
}

#[test]
fn set_add_duplicate_skips_length_limit_and_preserves_element() {
    let mut heap = ScriptHeap::new();
    let reference = heap.allocate(HeapValue::Set(ScriptSet::new()));
    let mut heap_execution = HeapExecution::new(&mut heap);
    push_set_slot(
        &mut heap_execution,
        reference,
        Value::F64(-0.0),
        None,
        "test set add",
    )
    .expect("initial set add should insert");
    let allocated_before = heap_execution.heap.allocated_bytes();
    let mut budget = ExecutionBudget::unbounded().with_collection_limits(CollectionLimits {
        max_array_len: usize::MAX,
        max_map_entries: usize::MAX,
        max_set_len: 1,
    });

    push_set_slot(
        &mut heap_execution,
        reference,
        Value::F64(0.0),
        Some(&mut budget),
        "test set add",
    )
    .expect("duplicate set add should not count as growth");

    assert_eq!(heap_execution.heap.allocated_bytes(), allocated_before);
    assert_eq!(budget.memory_bytes_allocated(), 0);
    let Some(HeapValue::Set(values)) = heap_execution.heap.get(reference) else {
        panic!("expected set value");
    };
    let values = values.values_vec();
    assert_eq!(values.len(), 1);
    let Value::F64(value) = values[0] else {
        panic!("stored set value should remain f64");
    };
    assert!(
        value.is_sign_negative(),
        "duplicate set insertion must preserve the first stored element"
    );
}

fn transient_values() -> [fn(&mut ScriptHeap) -> Value; 2] {
    [
        |_| Value::Missing,
        |heap| {
            let proxy =
                PathProxy::from_diagnostic_path(HostPath::new(host_ref()).field(FieldId::new(2)));
            Value::HeapRef(heap.allocate(HeapValue::PathProxy(proxy)))
        },
    ]
}

fn host_ref() -> HostRef {
    HostRef::new(HostTypeId::new(1), HostObjectId::new(7), 3)
}
