use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, Weak};

use crate::frame::CallFrame;
use crate::heap::{GcBudget, GcRef, GcStepStats, ScriptHeap};
use crate::{ExecutionBudget, Value};

pub struct HeapExecution<'heap> {
    pub heap: &'heap mut ScriptHeap,
    protected_roots: Vec<GcRef>,
    dynamic_roots: Option<Arc<Mutex<DynamicRootRegistry>>>,
    safe_point_roots: Vec<GcRef>,
    safe_point_gc_budget: GcBudget,
    gc_in_progress: bool,
    last_gc_step: Option<GcStepStats>,
}

#[derive(Debug)]
struct DynamicRoot {
    roots: Vec<GcRef>,
    refs: usize,
}

#[derive(Debug, Default)]
struct DynamicRootRegistry {
    next_id: u64,
    roots: BTreeMap<u64, DynamicRoot>,
}

#[derive(Debug)]
pub struct ActiveExecutionRoot {
    id: u64,
    registry: Weak<Mutex<DynamicRootRegistry>>,
}

impl Clone for ActiveExecutionRoot {
    fn clone(&self) -> Self {
        if let Some(registry) = self.registry.upgrade() {
            let mut registry = registry
                .lock()
                .expect("active execution root registry mutex poisoned");
            if let Some(root) = registry.roots.get_mut(&self.id) {
                root.refs = root.refs.saturating_add(1);
            }
        }
        Self {
            id: self.id,
            registry: Weak::clone(&self.registry),
        }
    }
}

impl Drop for ActiveExecutionRoot {
    fn drop(&mut self) {
        let Some(registry) = self.registry.upgrade() else {
            return;
        };
        let mut registry = registry
            .lock()
            .expect("active execution root registry mutex poisoned");
        let Some(root) = registry.roots.get_mut(&self.id) else {
            return;
        };
        root.refs = root.refs.saturating_sub(1);
        if root.refs == 0 {
            registry.roots.remove(&self.id);
        }
    }
}

pub struct ActiveExecutionValue {
    value: Value,
    root: ActiveExecutionRoot,
}

impl ActiveExecutionValue {
    #[must_use]
    pub fn into_parts(self) -> (Value, ActiveExecutionRoot) {
        (self.value, self.root)
    }
}

impl<'heap> HeapExecution<'heap> {
    #[must_use]
    pub fn new(heap: &'heap mut ScriptHeap) -> Self {
        let max_pause_micros = heap.gc_config().max_pause_micros;
        Self {
            heap,
            protected_roots: Vec::new(),
            dynamic_roots: None,
            safe_point_roots: Vec::new(),
            safe_point_gc_budget: GcBudget::micros(max_pause_micros),
            gc_in_progress: false,
            last_gc_step: None,
        }
    }

    #[must_use]
    pub fn with_safe_point_gc_budget(mut self, budget: GcBudget) -> Self {
        self.safe_point_gc_budget = budget;
        self
    }

    #[must_use]
    pub fn last_gc_step(&self) -> Option<&GcStepStats> {
        self.last_gc_step.as_ref()
    }

    pub(crate) fn push_frame_roots(&mut self, frame: &CallFrame) -> usize {
        let previous_len = self.protected_roots.len();
        frame.extend_heap_roots(&mut self.protected_roots);
        previous_len
    }

    pub(crate) fn truncate_protected_roots(&mut self, len: usize) {
        self.protected_roots.truncate(len);
    }

    pub(crate) fn protect_values(&mut self, values: &[Value]) {
        values
            .iter()
            .for_each(|value| value.trace_heap_refs(&mut self.protected_roots));
    }

    pub(crate) fn admit_dynamic_value(&mut self, value: Value) -> ActiveExecutionValue {
        let mut roots = Vec::new();
        value.trace_heap_refs(&mut roots);
        let registry = Arc::clone(
            self.dynamic_roots
                .get_or_insert_with(|| Arc::new(Mutex::new(DynamicRootRegistry::default()))),
        );
        let mut registry_guard = registry
            .lock()
            .expect("active execution root registry mutex poisoned");
        let id = registry_guard.next_id;
        registry_guard.next_id = registry_guard.next_id.saturating_add(1);
        registry_guard
            .roots
            .insert(id, DynamicRoot { roots, refs: 1 });
        drop(registry_guard);
        ActiveExecutionValue {
            value,
            root: ActiveExecutionRoot {
                id,
                registry: Arc::downgrade(&registry),
            },
        }
    }

    pub(crate) fn extend_dynamic_roots(&self, roots: &mut Vec<GcRef>) {
        let Some(registry) = self.dynamic_roots.as_ref() else {
            return;
        };
        let registry = registry
            .lock()
            .expect("active execution root registry mutex poisoned");
        registry
            .roots
            .values()
            .for_each(|root| roots.extend_from_slice(&root.roots));
    }

    fn dynamic_root_snapshot(&self) -> Vec<GcRef> {
        let mut roots = Vec::new();
        self.extend_dynamic_roots(&mut roots);
        roots
    }

    #[inline(always)]
    pub(crate) fn needs_safe_point(&self) -> bool {
        self.gc_in_progress || self.heap.should_collect()
    }

    #[inline(always)]
    pub(crate) fn collect_frame_at_safe_point(
        &mut self,
        frame: &CallFrame,
        budget: Option<&mut ExecutionBudget>,
    ) {
        if !self.needs_safe_point() {
            return;
        }

        self.safe_point_roots.clear();
        self.safe_point_roots.extend(&self.protected_roots);
        let dynamic_roots = self.dynamic_root_snapshot();
        self.safe_point_roots.extend(dynamic_roots);
        frame.extend_heap_roots(&mut self.safe_point_roots);
        let stats = self.heap.step_gc_with_budget(
            &self.safe_point_roots,
            self.safe_point_gc_budget,
            budget,
        );
        self.gc_in_progress = !stats.complete;
        self.last_gc_step = Some(stats);
    }
}

#[cfg(test)]
mod tests {
    use crate::heap::{GcBudget, HeapValue, ScriptHeap};
    use crate::{CallFrame, HeapExecution, Value};

    #[test]
    fn dynamic_root_admission_survives_the_next_sweep_slice() {
        let mut heap = ScriptHeap::new();
        let first = heap.allocate(HeapValue::String("first".into()));
        let admitted = heap.allocate(HeapValue::String("admitted".into()));
        let mut execution =
            HeapExecution::new(&mut heap).with_safe_point_gc_budget(GcBudget::sweep_slots(1));
        let frame = CallFrame::new(0);

        execution.collect_frame_at_safe_point(&frame, None);
        assert!(!execution.heap.contains(first));
        assert!(execution.heap.contains(admitted));

        let rooted = execution.admit_dynamic_value(Value::HeapRef(admitted));
        execution.collect_frame_at_safe_point(&frame, None);

        assert!(execution.heap.contains(admitted));
        drop(rooted);
    }

    #[test]
    fn dynamic_root_guard_release_allows_next_collection_to_reclaim_value() {
        let mut heap = ScriptHeap::new();
        let admitted = heap.allocate(HeapValue::String("admitted".into()));
        let mut execution =
            HeapExecution::new(&mut heap).with_safe_point_gc_budget(GcBudget::unlimited());
        let frame = CallFrame::new(0);
        let rooted = execution.admit_dynamic_value(Value::HeapRef(admitted));

        execution.collect_frame_at_safe_point(&frame, None);
        assert!(execution.heap.contains(admitted));

        drop(rooted);
        let garbage = execution.heap.allocate(HeapValue::String("trigger".into()));
        execution.collect_frame_at_safe_point(&frame, None);

        assert!(!execution.heap.contains(admitted));
        assert!(!execution.heap.contains(garbage));
    }

    #[test]
    fn sweep_slices_refresh_frame_and_protected_roots() {
        let mut heap = ScriptHeap::new();
        let garbage = heap.allocate(HeapValue::String("garbage".into()));
        let active = heap.allocate(HeapValue::String("active frame".into()));
        let protected = heap.allocate(HeapValue::String("caller frame".into()));
        let mut execution =
            HeapExecution::new(&mut heap).with_safe_point_gc_budget(GcBudget::sweep_slots(1));
        let mut frame = CallFrame::new(1);
        execution.collect_frame_at_safe_point(&frame, None);
        assert!(!execution.heap.contains(garbage));
        frame
            .write(vela_bytecode::Register(0), Value::HeapRef(active))
            .unwrap();
        execution.protect_values(&[Value::HeapRef(protected)]);
        execution.collect_frame_at_safe_point(&frame, None);
        execution.collect_frame_at_safe_point(&frame, None);
        assert!(execution.heap.contains(active));
        assert!(execution.heap.contains(protected));
    }
}
