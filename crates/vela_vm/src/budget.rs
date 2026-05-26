use crate::{VmError, VmErrorKind, VmResult};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionBudgetKind {
    Instructions,
    MemoryBytes,
    CallDepth,
    Patches,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionBudget {
    pub instruction_limit: u64,
    pub memory_limit_bytes: usize,
    pub max_call_depth: usize,
    pub max_patches: usize,
    instructions_executed: u64,
    memory_bytes_allocated: usize,
    current_call_depth: usize,
}

impl ExecutionBudget {
    #[must_use]
    pub fn new(
        instruction_limit: u64,
        memory_limit_bytes: usize,
        max_call_depth: usize,
        max_patches: usize,
    ) -> Self {
        Self {
            instruction_limit,
            memory_limit_bytes,
            max_call_depth,
            max_patches,
            instructions_executed: 0,
            memory_bytes_allocated: 0,
            current_call_depth: 0,
        }
    }

    #[must_use]
    pub fn unbounded() -> Self {
        Self::new(u64::MAX, usize::MAX, usize::MAX, usize::MAX)
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

    pub fn check_patch_count(&self, patch_count: usize) -> VmResult<()> {
        if patch_count > self.max_patches {
            Err(VmError::new(VmErrorKind::BudgetExceeded {
                budget: ExecutionBudgetKind::Patches,
                limit: u64::try_from(self.max_patches).unwrap_or(u64::MAX),
            }))
        } else {
            Ok(())
        }
    }

    pub fn reserve_patch(&self, current_patch_count: usize) -> VmResult<()> {
        self.check_patch_count(current_patch_count.saturating_add(1))
    }
}
