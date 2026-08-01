use vela_vm::budget::ExecutionBudget;

use super::control::{CallControl, CallPolicy};

#[derive(Clone, Debug)]
pub struct CallOptions {
    pub execution_unit_budget: u64,
    pub memory_budget: usize,
    pub call_depth: usize,
    pub managed_heap: bool,
    collection_limits: vela_vm::budget::CollectionLimits,
    host_call_budget: u64,
    deadline: Option<std::time::Instant>,
    control: Option<CallControl>,
    task_scope: Option<crate::task::TaskScope>,
}

impl PartialEq for CallOptions {
    fn eq(&self, other: &Self) -> bool {
        self.execution_unit_budget == other.execution_unit_budget
            && self.memory_budget == other.memory_budget
            && self.call_depth == other.call_depth
            && self.managed_heap == other.managed_heap
            && self.collection_limits == other.collection_limits
            && self.host_call_budget == other.host_call_budget
            && self.deadline == other.deadline
            && self.control == other.control
            && self.task_scope == other.task_scope
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
            collection_limits: vela_vm::budget::CollectionLimits::unbounded(),
            host_call_budget: u64::MAX,
            deadline: None,
            control: None,
            task_scope: None,
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

    #[must_use]
    pub const fn with_collection_limits(
        mut self,
        limits: vela_vm::budget::CollectionLimits,
    ) -> Self {
        self.collection_limits = limits;
        self
    }

    #[must_use]
    pub const fn with_host_call_budget(mut self, limit: u64) -> Self {
        self.host_call_budget = limit;
        self
    }

    #[must_use]
    /// Sets a cooperative deadline checked whenever the Runtime future is polled.
    /// The host must arrange a wake at the deadline if the awaited operation
    /// does not wake independently.
    pub fn with_deadline(mut self, deadline: std::time::Instant) -> Self {
        self.deadline = Some(deadline);
        self
    }

    #[must_use]
    /// Creates a deadline relative to the current host clock.
    pub fn with_timeout(self, timeout: std::time::Duration) -> Self {
        self.with_deadline(std::time::Instant::now() + timeout)
    }

    #[must_use]
    /// Attaches a one-call cancellation and observation handle.
    pub fn with_control(mut self, control: CallControl) -> Self {
        self.control = Some(control);
        self
    }

    #[must_use]
    pub fn with_task_scope(mut self, task_scope: crate::task::TaskScope) -> Self {
        self.task_scope = Some(task_scope);
        self
    }

    pub(super) const fn task_scope(&self) -> Option<&crate::task::TaskScope> {
        self.task_scope.as_ref()
    }

    pub(super) fn call_policy(&self) -> Option<CallPolicy> {
        (self.deadline.is_some() || self.control.is_some()).then(|| CallPolicy {
            deadline: self.deadline,
            control: self.control.clone(),
        })
    }

    pub(super) fn budget(&self) -> ExecutionBudget {
        ExecutionBudget::new(
            self.execution_unit_budget,
            self.memory_budget,
            self.call_depth,
        )
        .with_collection_limits(self.collection_limits)
        .with_host_call_limit(self.host_call_budget)
    }
}
