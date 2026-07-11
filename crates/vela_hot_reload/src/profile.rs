use std::collections::BTreeMap;
use vela_bytecode::{InstructionOffset, LinkedArtifact};

use crate::symbol::FunctionSymbolId;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProgramProfile {
    functions: BTreeMap<FunctionSymbolId, FunctionProfile>,
}

impl ProgramProfile {
    pub(crate) fn from_artifact(artifact: &LinkedArtifact) -> Self {
        let functions = artifact
            .profile_layout()
            .functions()
            .iter()
            .map(|layout| {
                let name = artifact.debug_name(layout.debug_name);
                (
                    FunctionSymbolId::new(name),
                    FunctionProfile::from_instruction_count(layout.instruction_count),
                )
            })
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
    fn from_instruction_count(instruction_count: usize) -> Self {
        let instruction_offsets = (0..instruction_count).map(InstructionOffset).collect();
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
