use std::cell::RefCell;
use std::collections::BTreeMap;

use vela_bytecode::{DebugNameId, InstructionOffset};
use vela_vm::VmBytecodeProfiler;

use super::image::RuntimeImage;

#[derive(Debug, Default)]
pub(super) struct RuntimeBytecodeProfile {
    functions: RefCell<Vec<FunctionCounters>>,
    function_index: BTreeMap<DebugNameId, usize>,
}

#[derive(Debug)]
struct FunctionCounters {
    instruction_hits: Vec<u64>,
}

impl RuntimeBytecodeProfile {
    pub(super) fn for_image(image: &RuntimeImage) -> Self {
        let program = image.linked_program();
        let functions = program
            .functions()
            .map(|(_, code)| {
                (
                    code.debug_name,
                    FunctionCounters {
                        instruction_hits: vec![0; code.instructions.len()],
                    },
                )
            })
            .collect::<Vec<_>>();
        let function_index = functions
            .iter()
            .enumerate()
            .map(|(index, (debug_name, _))| (*debug_name, index))
            .collect();
        let functions = functions
            .into_iter()
            .map(|(_, counters)| counters)
            .collect();
        Self {
            functions: RefCell::new(functions),
            function_index,
        }
    }

    pub(super) fn clear_for_image(&mut self, image: &RuntimeImage) {
        *self = Self::for_image(image);
    }

    #[cfg(test)]
    pub(super) fn instruction_hit_count(
        &self,
        function: DebugNameId,
        offset: InstructionOffset,
    ) -> Option<u64> {
        let index = *self.function_index.get(&function)?;
        self.functions
            .borrow()
            .get(index)?
            .instruction_hits
            .get(offset.0)
            .copied()
    }
}

impl VmBytecodeProfiler for RuntimeBytecodeProfile {
    fn record_instruction(&self, function: DebugNameId, offset: InstructionOffset) {
        let Some(index) = self.function_index.get(&function).copied() else {
            return;
        };
        let mut functions = self.functions.borrow_mut();
        let Some(counters) = functions.get_mut(index) else {
            return;
        };
        if let Some(count) = counters.instruction_hits.get_mut(offset.0) {
            *count = count.saturating_add(1);
        }
    }
}
