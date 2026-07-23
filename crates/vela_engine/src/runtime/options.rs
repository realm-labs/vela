use vela_vm::budget::ExecutionBudget;

#[derive(Clone, Debug)]
pub struct CallOptions {
    pub execution_unit_budget: u64,
    pub memory_budget: usize,
    pub call_depth: usize,
    pub managed_heap: bool,
}

impl PartialEq for CallOptions {
    fn eq(&self, other: &Self) -> bool {
        self.execution_unit_budget == other.execution_unit_budget
            && self.memory_budget == other.memory_budget
            && self.call_depth == other.call_depth
            && self.managed_heap == other.managed_heap
    }
}

impl Eq for CallOptions {}

impl CallOptions {
    #[must_use]
    pub const fn new(execution_unit_budget: u64, memory_budget: usize, call_depth: usize) -> Self {
        Self {
            execution_unit_budget,
            memory_budget,
            call_depth,
            managed_heap: true,
        }
    }

    #[must_use]
    pub const fn unbounded() -> Self {
        Self::new(u64::MAX, usize::MAX, usize::MAX)
    }

    #[must_use]
    pub const fn with_managed_heap(mut self, managed_heap: bool) -> Self {
        self.managed_heap = managed_heap;
        self
    }

    pub(super) fn budget(&self) -> ExecutionBudget {
        ExecutionBudget::new(
            self.execution_unit_budget,
            self.memory_budget,
            self.call_depth,
        )
    }
}
