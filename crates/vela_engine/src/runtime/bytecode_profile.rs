use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

use vela_bytecode::{
    DebugNameId, ExecutableGenerationId, InstructionOffset, ProfileLayout, ScriptFunctionHandle,
};
use vela_vm::VmBytecodeProfiler;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BytecodeProfileSnapshot {
    generation: ExecutableGenerationId,
    functions: Vec<FunctionBytecodeProfile>,
}

impl BytecodeProfileSnapshot {
    #[must_use]
    pub const fn generation(&self) -> ExecutableGenerationId {
        self.generation
    }

    #[must_use]
    pub fn functions(&self) -> &[FunctionBytecodeProfile] {
        &self.functions
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionBytecodeProfile {
    handle: ScriptFunctionHandle,
    debug_name: DebugNameId,
    instruction_hits: Vec<u64>,
}

impl FunctionBytecodeProfile {
    #[must_use]
    pub const fn handle(&self) -> ScriptFunctionHandle {
        self.handle
    }

    #[must_use]
    pub const fn debug_name(&self) -> DebugNameId {
        self.debug_name
    }

    #[must_use]
    pub fn instruction_hits(&self) -> &[u64] {
        &self.instruction_hits
    }
}

#[derive(Debug)]
pub(crate) struct GenerationBytecodeProfile {
    functions: Box<[FunctionCounters]>,
    function_index: BTreeMap<DebugNameId, usize>,
}

#[derive(Debug)]
struct FunctionCounters {
    handle: ScriptFunctionHandle,
    debug_name: DebugNameId,
    instruction_hits: Box<[AtomicU64]>,
}

impl GenerationBytecodeProfile {
    pub(super) fn for_layout(layout: &ProfileLayout) -> Self {
        let functions = layout
            .functions()
            .iter()
            .map(|function| FunctionCounters {
                handle: function.handle,
                debug_name: function.debug_name,
                instruction_hits: (0..function.instruction_count)
                    .map(|_| AtomicU64::new(0))
                    .collect(),
            })
            .collect::<Box<[_]>>();
        let function_index = functions
            .iter()
            .enumerate()
            .map(|(index, function)| (function.debug_name, index))
            .collect();
        Self {
            functions,
            function_index,
        }
    }

    pub(super) fn snapshot(&self, generation: ExecutableGenerationId) -> BytecodeProfileSnapshot {
        BytecodeProfileSnapshot {
            generation,
            functions: self
                .functions
                .iter()
                .map(|function| FunctionBytecodeProfile {
                    handle: function.handle,
                    debug_name: function.debug_name,
                    instruction_hits: function
                        .instruction_hits
                        .iter()
                        .map(|count| count.load(Ordering::Relaxed))
                        .collect(),
                })
                .collect(),
        }
    }

    pub(super) fn reset(&self) {
        for counter in self
            .functions
            .iter()
            .flat_map(|function| function.instruction_hits.iter())
        {
            counter.store(0, Ordering::Relaxed);
        }
    }

    #[cfg(test)]
    pub(super) fn set_instruction_hit_count(
        &self,
        function: DebugNameId,
        offset: InstructionOffset,
        value: u64,
    ) -> bool {
        let Some(index) = self.function_index.get(&function).copied() else {
            return false;
        };
        let Some(count) = self
            .functions
            .get(index)
            .and_then(|function| function.instruction_hits.get(offset.0))
        else {
            return false;
        };
        count.store(value, Ordering::Relaxed);
        true
    }
}

impl VmBytecodeProfiler for GenerationBytecodeProfile {
    fn record_instruction(&self, function: DebugNameId, offset: InstructionOffset) {
        let Some(index) = self.function_index.get(&function).copied() else {
            return;
        };
        let Some(count) = self
            .functions
            .get(index)
            .and_then(|function| function.instruction_hits.get(offset.0))
        else {
            return;
        };
        let _ = count.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            Some(current.saturating_add(1))
        });
    }
}
