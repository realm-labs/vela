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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionBudget {
    pub instruction_limit: u64,
    pub memory_limit_bytes: usize,
    pub max_call_depth: usize,
    collection_limits: CollectionLimits,
    instructions_executed: u64,
    memory_bytes_allocated: usize,
    current_call_depth: usize,
}

impl ExecutionBudget {
    #[must_use]
    pub fn new(instruction_limit: u64, memory_limit_bytes: usize, max_call_depth: usize) -> Self {
        Self {
            instruction_limit,
            memory_limit_bytes,
            max_call_depth,
            collection_limits: CollectionLimits::unbounded(),
            instructions_executed: 0,
            memory_bytes_allocated: 0,
            current_call_depth: 0,
        }
    }

    #[must_use]
    pub fn unbounded() -> Self {
        Self::new(u64::MAX, usize::MAX, usize::MAX)
    }

    #[must_use]
    pub fn instructions_executed(&self) -> u64 {
        self.instructions_executed
    }

    #[must_use]
    pub fn memory_bytes_allocated(&self) -> usize {
        self.memory_bytes_allocated
    }

    #[must_use]
    pub fn current_call_depth(&self) -> usize {
        self.current_call_depth
    }

    #[must_use]
    pub fn collection_limits(&self) -> CollectionLimits {
        self.collection_limits
    }

    #[must_use]
    pub fn with_collection_limits(mut self, limits: CollectionLimits) -> Self {
        self.collection_limits = limits;
        self
    }

    pub fn charge_instructions(&mut self, instructions: u64) -> VmResult<()> {
        let next = self.instructions_executed.saturating_add(instructions);
        if next > self.instruction_limit {
            return Err(VmError::new(VmErrorKind::BudgetExceeded {
                budget: ExecutionBudgetKind::Instructions,
                limit: self.instruction_limit,
            }));
        }
        self.instructions_executed = next;
        Ok(())
    }

    pub(crate) fn charge_instruction(&mut self) -> VmResult<()> {
        self.charge_instructions(1)?;
        Ok(())
    }

    #[must_use]
    pub(crate) fn charges_instructions(&self) -> bool {
        self.instruction_limit != u64::MAX
    }

    pub fn charge_memory_bytes(&mut self, bytes: usize) -> VmResult<()> {
        let next = self.memory_bytes_allocated.saturating_add(bytes);
        if next > self.memory_limit_bytes {
            return Err(VmError::new(VmErrorKind::BudgetExceeded {
                budget: ExecutionBudgetKind::MemoryBytes,
                limit: u64::try_from(self.memory_limit_bytes).unwrap_or(u64::MAX),
            }));
        }
        self.memory_bytes_allocated = next;
        Ok(())
    }

    pub(crate) fn charge_memory(&mut self, bytes: usize) -> VmResult<()> {
        self.charge_memory_bytes(bytes)
    }

    pub(crate) fn release_memory(&mut self, bytes: usize) {
        self.memory_bytes_allocated = self.memory_bytes_allocated.saturating_sub(bytes);
    }

    pub(crate) fn enter_call(&mut self) -> VmResult<()> {
        if self.current_call_depth >= self.max_call_depth {
            return Err(VmError::new(VmErrorKind::BudgetExceeded {
                budget: ExecutionBudgetKind::CallDepth,
                limit: u64::try_from(self.max_call_depth).unwrap_or(u64::MAX),
            }));
        }
        self.current_call_depth = self.current_call_depth.saturating_add(1);
        Ok(())
    }

    pub(crate) fn exit_call(&mut self) {
        self.current_call_depth = self.current_call_depth.saturating_sub(1);
    }
}
