use std::collections::BTreeMap;
use std::sync::Arc;

use vela_bytecode::{CodeObject, Program};

use crate::{FunctionSymbolId, HotReloadAbi, ProgramVersionId};

#[derive(Clone, Debug, PartialEq)]
pub struct ProgramVersion {
    pub id: ProgramVersionId,
    pub(crate) functions: BTreeMap<FunctionSymbolId, Arc<CodeObject>>,
    pub(crate) abi: HotReloadAbi,
}

impl ProgramVersion {
    #[must_use]
    pub fn from_program(id: ProgramVersionId, program: Program) -> Self {
        Self::from_program_with_abi(id, program, HotReloadAbi::empty())
    }

    #[must_use]
    pub fn from_program_with_abi(
        id: ProgramVersionId,
        program: Program,
        abi: HotReloadAbi,
    ) -> Self {
        let functions = program
            .functions
            .into_iter()
            .map(|(name, code)| (FunctionSymbolId::new(name), Arc::new(code)))
            .collect();
        Self { id, functions, abi }
    }

    #[must_use]
    pub fn function(&self, name: &str) -> Option<Arc<CodeObject>> {
        self.functions.get(&FunctionSymbolId::new(name)).cloned()
    }

    #[must_use]
    pub fn abi(&self) -> &HotReloadAbi {
        &self.abi
    }

    #[must_use]
    pub fn to_program(&self) -> Program {
        let mut program = Program::new();
        for function in self.functions.values() {
            program.insert_function((**function).clone());
        }
        program
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct HotUpdate {
    pub(crate) functions: BTreeMap<FunctionSymbolId, Arc<CodeObject>>,
    pub(crate) abi: HotReloadAbi,
}

impl HotUpdate {
    pub(crate) fn new(
        functions: BTreeMap<FunctionSymbolId, Arc<CodeObject>>,
        abi: HotReloadAbi,
    ) -> Self {
        Self { functions, abi }
    }
}
