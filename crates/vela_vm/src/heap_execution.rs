use crate::frame::CallFrame;
use crate::heap::{GcBudget, GcRef, GcStepStats, ScriptHeap};
use crate::{ExecutionBudget, Value};

pub struct HeapExecution<'heap> {
    pub heap: &'heap mut ScriptHeap,
    protected_roots: Vec<GcRef>,
    safe_point_roots: Vec<GcRef>,
    safe_point_gc_budget: GcBudget,
    gc_in_progress: bool,
    last_gc_step: Option<GcStepStats>,
}

impl<'heap> HeapExecution<'heap> {
    #[must_use]
    pub fn new(heap: &'heap mut ScriptHeap) -> Self {
        let max_pause_micros = heap.gc_config().max_pause_micros;
        Self {
            heap,
            protected_roots: Vec::new(),
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

    pub(crate) fn push_protected_roots(&mut self, roots: &[GcRef]) -> usize {
        let previous_len = self.protected_roots.len();
        self.protected_roots.extend_from_slice(roots);
        previous_len
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

    pub(crate) fn protect_value_refs<'value>(
        &mut self,
        values: impl IntoIterator<Item = &'value Value>,
    ) {
        values
            .into_iter()
            .for_each(|value| value.trace_heap_refs(&mut self.protected_roots));
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

        let stats = if self.gc_in_progress {
            self.heap
                .step_gc_with_budget(&[], self.safe_point_gc_budget, budget)
        } else {
            self.safe_point_roots.clear();
            self.safe_point_roots.extend(&self.protected_roots);
            frame.extend_heap_roots(&mut self.safe_point_roots);
            self.heap
                .step_gc_with_budget(&self.safe_point_roots, self.safe_point_gc_budget, budget)
        };
        self.gc_in_progress = !stats.complete;
        self.last_gc_step = Some(stats);
    }
}
