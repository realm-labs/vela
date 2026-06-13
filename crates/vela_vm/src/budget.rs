use crate::{VmError, VmErrorKind, VmResult};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionBudgetKind {
    Instructions,
    MemoryBytes,
    CallDepth,
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
    pub instruction_limit: u64,
    pub memory_limit_bytes: usize,
    pub max_call_depth: usize,
    pub collection_limits: CollectionLimits,
}

impl ExecutionLimits {
    #[must_use]
    pub const fn new(
        instruction_limit: u64,
        memory_limit_bytes: usize,
        max_call_depth: usize,
    ) -> Self {
        Self {
            instruction_limit,
            memory_limit_bytes,
            max_call_depth,
            collection_limits: CollectionLimits::unbounded(),
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
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ExecutionCounters {
    instructions_executed: u64,
    memory_bytes_allocated: usize,
    current_call_depth: usize,
}

impl ExecutionCounters {
    #[must_use]
    pub fn instructions_executed(self) -> u64 {
        self.instructions_executed
    }

    #[must_use]
    pub fn memory_bytes_allocated(self) -> usize {
        self.memory_bytes_allocated
    }

    #[must_use]
    pub fn current_call_depth(self) -> usize {
        self.current_call_depth
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BudgetFlags {
    bits: u8,
}

impl BudgetFlags {
    const INSTRUCTIONS: u8 = 0b0001;
    const MEMORY: u8 = 0b0010;
    const CALL_DEPTH: u8 = 0b0100;
    const COLLECTION_LIMITS: u8 = 0b1000;

    #[must_use]
    const fn from_limits(limits: &ExecutionLimits) -> Self {
        let mut bits = 0;
        if limits.instruction_limit != u64::MAX {
            bits |= Self::INSTRUCTIONS;
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
    pub fn new(instruction_limit: u64, memory_limit_bytes: usize, max_call_depth: usize) -> Self {
        Self::with_limits(ExecutionLimits::new(
            instruction_limit,
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
    pub fn instructions_executed(&self) -> u64 {
        self.counters.instructions_executed()
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
    pub fn collection_limits(&self) -> CollectionLimits {
        self.limits.collection_limits
    }

    #[must_use]
    pub fn with_collection_limits(mut self, limits: CollectionLimits) -> Self {
        self.limits = self.limits.with_collection_limits(limits);
        self.flags = BudgetFlags::from_limits(&self.limits);
        self
    }

    pub fn charge_instructions(&mut self, instructions: u64) -> VmResult<()> {
        if !self.charges_instructions() {
            return Ok(());
        }
        let next = self
            .counters
            .instructions_executed
            .saturating_add(instructions);
        if next > self.limits.instruction_limit {
            return Err(VmError::new(VmErrorKind::BudgetExceeded {
                budget: ExecutionBudgetKind::Instructions,
                limit: self.limits.instruction_limit,
            }));
        }
        self.counters.instructions_executed = next;
        Ok(())
    }

    pub(crate) fn charge_instruction(&mut self) -> VmResult<()> {
        self.charge_instructions(1)?;
        Ok(())
    }

    #[must_use]
    #[inline(always)]
    pub(crate) fn charges_instructions(&self) -> bool {
        self.flags.contains(BudgetFlags::INSTRUCTIONS)
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

        assert!(!budget.charges_instructions());
        assert!(!budget.charges_memory());
        assert!(!budget.limits_call_depth());
        assert!(!budget.limits_collections());
        assert!(!budget.tracks_collection_growth());

        budget.charge_instructions(10).expect("unbounded charge");
        budget.charge_memory_bytes(128).expect("unbounded memory");
        budget.enter_call().expect("unbounded call depth");
        budget.exit_call();

        assert_eq!(budget.instructions_executed(), 0);
        assert_eq!(budget.memory_bytes_allocated(), 0);
        assert_eq!(budget.current_call_depth(), 0);
    }

    #[test]
    fn finite_limits_enable_independent_budget_flags() {
        let instruction_only = ExecutionBudget::new(10, usize::MAX, usize::MAX);
        assert!(instruction_only.charges_instructions());
        assert!(!instruction_only.charges_memory());
        assert!(!instruction_only.limits_call_depth());

        let memory_only = ExecutionBudget::new(u64::MAX, 1024, usize::MAX);
        assert!(!memory_only.charges_instructions());
        assert!(memory_only.charges_memory());
        assert!(!memory_only.limits_call_depth());

        let call_depth_only = ExecutionBudget::new(u64::MAX, usize::MAX, 4);
        assert!(!call_depth_only.charges_instructions());
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
    fn call_depth_counter_updates_only_when_limit_is_active() {
        let mut budget = ExecutionBudget::new(u64::MAX, usize::MAX, 1);

        budget.enter_call().expect("first call fits");
        assert_eq!(budget.current_call_depth(), 1);
        assert!(budget.enter_call().is_err());
        budget.exit_call();
        assert_eq!(budget.current_call_depth(), 0);
    }
}
