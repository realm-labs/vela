use super::*;

#[test]
fn gc_config_tracks_next_collection_threshold() {
    let mut heap = ScriptHeap::new();
    heap.set_gc_config(GcConfig {
        max_pause_micros: 200,
        heap_growth_factor: 1.0,
    });
    let live = heap.allocate(HeapValue::String("live".into()));

    let stats = heap.collect_full(&[live]);

    assert_eq!(stats.swept, 0);
    assert_eq!(heap.gc_config().max_pause_micros, 200);
    assert_eq!(heap.next_gc_at_bytes(), heap.allocated_bytes() + 1);
    assert!(!heap.should_collect());

    let _extra = heap.allocate(HeapValue::String("extra".into()));

    assert!(heap.should_collect());
}
