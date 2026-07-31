use vela_vm::budget::ExecutionBudget;

use super::control::{CallControl, CallPolicy};

#[derive(Clone, Debug)]
pub struct CallOptions {
    pub execution_unit_budget: u64,
    pub memory_budget: usize,
    pub call_depth: usize,
    pub managed_heap: bool,
    deadline: Option<std::time::Instant>,
    control: Option<CallControl>,
}

impl PartialEq for CallOptions {
    fn eq(&self, other: &Self) -> bool {
        self.execution_unit_budget == other.execution_unit_budget
            && self.memory_budget == other.memory_budget
            && self.call_depth == other.call_depth
            && self.managed_heap == other.managed_heap
            && self.deadline == other.deadline
            && self.control == other.control
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
            deadline: None,
            control: None,
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
    }
}
