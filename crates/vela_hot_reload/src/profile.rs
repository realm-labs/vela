use std::collections::BTreeMap;
use std::sync::Arc;

use vela_bytecode::{InstructionOffset, UnlinkedCodeObject};

use crate::symbol::FunctionSymbolId;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProgramProfile {
    functions: BTreeMap<FunctionSymbolId, FunctionProfile>,
}

impl ProgramProfile {
    pub(crate) fn from_functions(
        functions: &BTreeMap<FunctionSymbolId, Arc<UnlinkedCodeObject>>,
    ) -> Self {
        let functions = functions
            .iter()
            .map(|(name, code)| (name.clone(), FunctionProfile::from_code(code)))
            .collect();
        Self { functions }
    }

    #[must_use]
    pub fn function(&self, name: &str) -> Option<&FunctionProfile> {
        self.functions.get(&FunctionSymbolId::new(name))
    }

    #[must_use]
    pub fn function_by_id(&self, name: &FunctionSymbolId) -> Option<&FunctionProfile> {
        self.functions.get(name)
    }

    pub fn function_names(&self) -> impl Iterator<Item = &str> {
        self.functions.keys().map(|name| name.0.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionProfile {
    instruction_offsets: Vec<InstructionOffset>,
}

impl FunctionProfile {
    fn from_code(code: &UnlinkedCodeObject) -> Self {
        let instruction_offsets = (0..code.instructions.len())
            .map(InstructionOffset)
            .collect();
        Self {
            instruction_offsets,
        }
    }

    #[must_use]
    pub fn instruction_count(&self) -> usize {
        self.instruction_offsets.len()
    }

    #[must_use]
    pub fn instruction_offsets(&self) -> &[InstructionOffset] {
        &self.instruction_offsets
    }

    #[must_use]
    pub fn contains_offset(&self, offset: InstructionOffset) -> bool {
        offset.0 < self.instruction_offsets.len()
    }
}
