use crate::heap::{GcBudget, HeapValue, ScriptHeap};
use crate::value::Value;

#[test]
fn new_children_of_an_already_swept_root_survive() {
    let mut heap = ScriptHeap::new();
    let parent = heap.allocate(HeapValue::Array(vec![]));
    let garbage = heap.allocate(HeapValue::String("garbage".into()));
    assert!(!heap.step_gc(&[parent], GcBudget::sweep_slots(1)).complete);
    let child = heap.allocate(HeapValue::String("live".into()));
    let HeapValue::Array(values) = heap.get_mut(parent).unwrap() else {
        panic!()
    };
    values.push(Value::HeapRef(child));
    let stats = heap.step_gc(&[parent], GcBudget::unlimited());
    assert!(stats.complete);
    assert!(heap.contains(parent));
    assert!(heap.contains(child));
    assert!(!heap.contains(garbage));
}

#[test]
fn each_slice_uses_current_roots_and_retraces_existing_containers() {
    let mut heap = ScriptHeap::new();
    let parent = heap.allocate(HeapValue::Array(vec![]));
    let new_root = heap.allocate(HeapValue::String("new root".into()));
    let linked = heap.allocate(HeapValue::String("new edge".into()));
    let released = heap.allocate(HeapValue::String("released root".into()));
    assert!(
        !heap
            .step_gc(&[parent, released], GcBudget::sweep_slots(1))
            .complete
    );
    let HeapValue::Array(values) = heap.get_mut(parent).unwrap() else {
        panic!()
    };
    values.push(Value::HeapRef(linked));
    heap.step_gc(&[parent, new_root], GcBudget::unlimited());
    assert!(heap.contains(new_root));
    assert!(heap.contains(linked));
    assert!(!heap.contains(released));
}
