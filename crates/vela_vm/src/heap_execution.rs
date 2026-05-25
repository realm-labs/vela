use crate::ExecutionBudget;
use crate::heap::{GcBudget, GcRef, GcStepStats, ScriptHeap};

pub struct HeapExecution<'heap> {
    pub heap: &'heap mut ScriptHeap,
    protected_roots: Vec<GcRef>,
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

    pub(crate) fn push_protected_roots(&mut self, roots: Vec<GcRef>) -> usize {
        let previous_len = self.protected_roots.len();
        self.protected_roots.extend(roots);
        previous_len
    }

    pub(crate) fn truncate_protected_roots(&mut self, len: usize) {
        self.protected_roots.truncate(len);
    }

    pub(crate) fn collect_at_safe_point(
        &mut self,
        frame_roots: Vec<GcRef>,
        budget: Option<&mut ExecutionBudget>,
    ) {
        if !self.gc_in_progress && !self.heap.should_collect() {
            return;
        }

        let mut roots = self.protected_roots.clone();
        roots.extend(frame_roots);
        let stats = self
            .heap
            .step_gc_with_budget(&roots, self.safe_point_gc_budget, budget);
        self.gc_in_progress = !stats.complete;
        self.last_gc_step = Some(stats);
    }
}
