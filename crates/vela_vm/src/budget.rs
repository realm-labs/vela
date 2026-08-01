use crate::{VmError, VmErrorKind, VmResult};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionBudgetKind {
    ExecutionUnits,
    MemoryBytes,
    CallDepth,
    HostCalls,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CollectionLimits {
    pub max_array_len: usize,
    pub max_map_entries: usize,
    pub max_set_len: usize,
}

impl CollectionLimits {
    #[must_use]
    pub const fn unbounded() -> Self {
        Self {
            max_array_len: usize::MAX,
            max_map_entries: usize::MAX,
            max_set_len: usize::MAX,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionLimits {
    pub execution_unit_limit: u64,
    pub memory_limit_bytes: usize,
    pub max_call_depth: usize,
    pub collection_limits: CollectionLimits,
    pub host_call_limit: u64,
}

impl ExecutionLimits {
    #[must_use]
    pub const fn new(
        execution_unit_limit: u64,
        memory_limit_bytes: usize,
        max_call_depth: usize,
    ) -> Self {
        Self {
            execution_unit_limit,
            memory_limit_bytes,
            max_call_depth,
            collection_limits: CollectionLimits::unbounded(),
            host_call_limit: u64::MAX,
        }
    }

    #[must_use]
    pub const fn unbounded() -> Self {
        Self::new(u64::MAX, usize::MAX, usize::MAX)
    }

    #[must_use]
    pub const fn with_collection_limits(mut self, limits: CollectionLimits) -> Self {
        self.collection_limits = limits;
        self
    }

    #[must_use]
    pub const fn with_host_call_limit(mut self, limit: u64) -> Self {
        self.host_call_limit = limit;
        self
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ExecutionCounters {
    execution_units_consumed: u64,
    memory_bytes_allocated: usize,
    current_call_depth: usize,
    host_calls: u64,
}

impl ExecutionCounters {
    #[must_use]
    pub fn execution_units_consumed(self) -> u64 {
        self.execution_units_consumed
    }

    #[must_use]
    pub fn memory_bytes_allocated(self) -> usize {
        self.memory_bytes_allocated
    }

    #[must_use]
    pub fn current_call_depth(self) -> usize {
        self.current_call_depth
    }

    #[must_use]
    pub fn host_calls(self) -> u64 {
        self.host_calls
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BudgetFlags {
    bits: u8,
}

impl BudgetFlags {
    const EXECUTION_UNITS: u8 = 0b0001;
    const MEMORY: u8 = 0b0010;
    const CALL_DEPTH: u8 = 0b0100;
    const COLLECTION_LIMITS: u8 = 0b1000;
    const HOST_CALLS: u8 = 0b1_0000;

    #[must_use]
    const fn from_limits(limits: &ExecutionLimits) -> Self {
        let mut bits = 0;
        if limits.execution_unit_limit != u64::MAX {
            bits |= Self::EXECUTION_UNITS;
        }
        if limits.memory_limit_bytes != usize::MAX {
            bits |= Self::MEMORY;
        }
        if limits.max_call_depth != usize::MAX {
            bits |= Self::CALL_DEPTH;
        }
        if limits.collection_limits.max_array_len != usize::MAX
            || limits.collection_limits.max_map_entries != usize::MAX
            || limits.collection_limits.max_set_len != usize::MAX
        {
            bits |= Self::COLLECTION_LIMITS;
        }
        if limits.host_call_limit != u64::MAX {
            bits |= Self::HOST_CALLS;
        }
        Self { bits }
    }

    #[must_use]
    #[inline(always)]
    const fn contains(self, flag: u8) -> bool {
        self.bits & flag != 0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionBudget {
    limits: ExecutionLimits,
    counters: ExecutionCounters,
    flags: BudgetFlags,
}

impl ExecutionBudget {
    #[must_use]
    pub fn new(
        execution_unit_limit: u64,
        memory_limit_bytes: usize,
        max_call_depth: usize,
    ) -> Self {
        Self::with_limits(ExecutionLimits::new(
            execution_unit_limit,
            memory_limit_bytes,
            max_call_depth,
        ))
    }

    #[must_use]
    pub fn with_limits(limits: ExecutionLimits) -> Self {
        Self {
            limits,
            counters: ExecutionCounters::default(),
            flags: BudgetFlags::from_limits(&limits),
        }
    }

    #[must_use]
    pub fn unbounded() -> Self {
        Self::with_limits(ExecutionLimits::unbounded())
    }

    #[must_use]
    pub fn limits(&self) -> ExecutionLimits {
        self.limits
    }

    #[must_use]
    pub fn counters(&self) -> ExecutionCounters {
        self.counters
    }

    #[must_use]
    pub fn execution_units_consumed(&self) -> u64 {
        self.counters.execution_units_consumed()
    }

    #[must_use]
    pub fn memory_bytes_allocated(&self) -> usize {
        self.counters.memory_bytes_allocated()
    }

    #[must_use]
    pub fn current_call_depth(&self) -> usize {
        self.counters.current_call_depth()
    }

    #[must_use]
    pub fn host_calls(&self) -> u64 {
        self.counters.host_calls()
    }

    #[must_use]
    pub fn collection_limits(&self) -> CollectionLimits {
        self.limits.collection_limits
    }

    #[must_use]
    pub fn with_collection_limits(mut self, limits: CollectionLimits) -> Self {
        self.limits = self.limits.with_collection_limits(limits);
        self.flags = BudgetFlags::from_limits(&self.limits);
        self
    }

    #[must_use]
    pub fn with_host_call_limit(mut self, limit: u64) -> Self {
        self.limits = self.limits.with_host_call_limit(limit);
        self.flags = BudgetFlags::from_limits(&self.limits);
        self
    }

    pub fn charge_execution_units(&mut self, units: u64) -> VmResult<()> {
        if !self.charges_execution_units() {
            return Ok(());
        }
        let next = self.counters.execution_units_consumed.saturating_add(units);
        if next > self.limits.execution_unit_limit {
            return Err(VmError::new(VmErrorKind::BudgetExceeded {
                budget: ExecutionBudgetKind::ExecutionUnits,
                limit: self.limits.execution_unit_limit,
            }));
        }
        self.counters.execution_units_consumed = next;
        Ok(())
    }

    pub fn charge_host_call(&mut self) -> VmResult<()> {
        if !self.limits_host_calls() {
            return Ok(());
        }
        let next = self.counters.host_calls.saturating_add(1);
        if next > self.limits.host_call_limit {
            return Err(VmError::new(VmErrorKind::BudgetExceeded {
                budget: ExecutionBudgetKind::HostCalls,
                limit: self.limits.host_call_limit,
            }));
        }
        self.counters.host_calls = next;
        Ok(())
    }

    #[must_use]
    #[inline(always)]
    pub(crate) fn charges_execution_units(&self) -> bool {
        self.flags.contains(BudgetFlags::EXECUTION_UNITS)
    }

    #[must_use]
    #[inline(always)]
    pub(crate) fn charges_memory(&self) -> bool {
        self.flags.contains(BudgetFlags::MEMORY)
    }

    #[must_use]
    #[inline(always)]
    pub(crate) fn limits_call_depth(&self) -> bool {
        self.flags.contains(BudgetFlags::CALL_DEPTH)
    }

    #[must_use]
    #[inline(always)]
    pub(crate) fn limits_collections(&self) -> bool {
        self.flags.contains(BudgetFlags::COLLECTION_LIMITS)
    }

    #[must_use]
    #[inline(always)]
    pub(crate) fn limits_host_calls(&self) -> bool {
        self.flags.contains(BudgetFlags::HOST_CALLS)
    }

    #[must_use]
    #[inline(always)]
    pub(crate) fn tracks_collection_growth(&self) -> bool {
        self.charges_memory() || self.limits_collections()
    }

    pub fn charge_memory_bytes(&mut self, bytes: usize) -> VmResult<()> {
        if !self.charges_memory() {
            return Ok(());
        }
        let next = self.counters.memory_bytes_allocated.saturating_add(bytes);
        if next > self.limits.memory_limit_bytes {
            return Err(VmError::new(VmErrorKind::BudgetExceeded {
                budget: ExecutionBudgetKind::MemoryBytes,
                limit: u64::try_from(self.limits.memory_limit_bytes).unwrap_or(u64::MAX),
            }));
        }
        self.counters.memory_bytes_allocated = next;
        Ok(())
    }

    pub(crate) fn charge_memory(&mut self, bytes: usize) -> VmResult<()> {
        self.charge_memory_bytes(bytes)
    }

    pub(crate) fn release_memory(&mut self, bytes: usize) {
        if self.charges_memory() {
            self.counters.memory_bytes_allocated =
                self.counters.memory_bytes_allocated.saturating_sub(bytes);
        }
    }

    pub(crate) fn enter_call(&mut self) -> VmResult<()> {
        if !self.limits_call_depth() {
            return Ok(());
        }
        if self.counters.current_call_depth >= self.limits.max_call_depth {
            return Err(VmError::new(VmErrorKind::BudgetExceeded {
                budget: ExecutionBudgetKind::CallDepth,
                limit: u64::try_from(self.limits.max_call_depth).unwrap_or(u64::MAX),
            }));
        }
        self.counters.current_call_depth = self.counters.current_call_depth.saturating_add(1);
        Ok(())
    }

    pub(crate) fn exit_call(&mut self) {
        if self.limits_call_depth() {
            self.counters.current_call_depth = self.counters.current_call_depth.saturating_sub(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CollectionLimits, ExecutionBudget};

    #[test]
    fn unbounded_budget_disables_all_runtime_flags() {
        let mut budget = ExecutionBudget::unbounded();

        assert!(!budget.charges_execution_units());
        assert!(!budget.charges_memory());
        assert!(!budget.limits_call_depth());
        assert!(!budget.limits_collections());
        assert!(!budget.limits_host_calls());
        assert!(!budget.tracks_collection_growth());

        budget.charge_execution_units(10).expect("unbounded charge");
        budget.charge_memory_bytes(128).expect("unbounded memory");
        budget.charge_host_call().expect("unbounded host call");
        budget.enter_call().expect("unbounded call depth");
        budget.exit_call();

        assert_eq!(budget.execution_units_consumed(), 0);
        assert_eq!(budget.memory_bytes_allocated(), 0);
        assert_eq!(budget.current_call_depth(), 0);
        assert_eq!(budget.host_calls(), 0);
    }

    #[test]
    fn finite_limits_enable_independent_budget_flags() {
        let execution_units_only = ExecutionBudget::new(10, usize::MAX, usize::MAX);
        assert!(execution_units_only.charges_execution_units());
        assert!(!execution_units_only.charges_memory());
        assert!(!execution_units_only.limits_call_depth());

        let memory_only = ExecutionBudget::new(u64::MAX, 1024, usize::MAX);
        assert!(!memory_only.charges_execution_units());
        assert!(memory_only.charges_memory());
        assert!(!memory_only.limits_call_depth());

        let call_depth_only = ExecutionBudget::new(u64::MAX, usize::MAX, 4);
        assert!(!call_depth_only.charges_execution_units());
        assert!(!call_depth_only.charges_memory());
        assert!(call_depth_only.limits_call_depth());
    }

    #[test]
    fn collection_limits_refresh_flags_without_memory_accounting() {
        let budget = ExecutionBudget::unbounded().with_collection_limits(CollectionLimits {
            max_array_len: 1,
            max_map_entries: usize::MAX,
            max_set_len: usize::MAX,
        });

        assert!(!budget.charges_memory());
        assert!(budget.limits_collections());
        assert!(budget.tracks_collection_growth());
        assert_eq!(budget.memory_bytes_allocated(), 0);
    }

    #[test]
    fn host_call_limit_is_independent_and_fails_before_increment() {
        let mut budget = ExecutionBudget::unbounded().with_host_call_limit(1);

        budget.charge_host_call().expect("first host call fits");
        let error = budget
            .charge_host_call()
            .expect_err("second host call exceeds limit");

        assert!(matches!(
            error.kind(),
            crate::VmErrorKind::BudgetExceeded {
                budget: super::ExecutionBudgetKind::HostCalls,
                limit: 1,
            }
        ));
        assert_eq!(budget.host_calls(), 1);
    }

    #[test]
    fn call_depth_counter_updates_only_when_limit_is_active() {
        let mut budget = ExecutionBudget::new(u64::MAX, usize::MAX, 1);

        budget.enter_call().expect("first call fits");
        assert_eq!(budget.current_call_depth(), 1);
        assert!(budget.enter_call().is_err());
        budget.exit_call();
        assert_eq!(budget.current_call_depth(), 0);
    }
}
