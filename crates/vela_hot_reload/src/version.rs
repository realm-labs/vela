use std::collections::BTreeMap;
use std::sync::Arc;

use vela_bytecode::{CodeObject, Program, script_methods::ScriptMethodTable};
use vela_hir::module_graph::ModuleGraph;

use crate::abi::HotReloadAbi;
use crate::report::AcceptedHotReloadChanges;
use crate::symbol::{FunctionSymbolId, ProgramVersionId};

#[derive(Clone, Debug, PartialEq)]
pub struct ProgramVersion {
    pub id: ProgramVersionId,
    pub(crate) functions: BTreeMap<FunctionSymbolId, Arc<CodeObject>>,
    pub(crate) script_methods: ScriptMethodTable,
    pub(crate) script_metadata: Option<ModuleGraph>,
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
        let script_methods = program.script_methods().clone();
        let script_metadata = program.script_metadata().cloned();
        let functions = program
            .functions
            .into_iter()
            .map(|(name, code)| (FunctionSymbolId::new(name), Arc::new(code)))
            .collect();
        Self {
            id,
            functions,
            script_methods,
            script_metadata,
            abi,
        }
    }

    #[must_use]
    pub fn function(&self, name: &str) -> Option<Arc<CodeObject>> {
        self.functions.get(&FunctionSymbolId::new(name)).cloned()
    }

    pub fn function_names(&self) -> impl Iterator<Item = &str> {
        self.functions.keys().map(|name| name.0.as_str())
    }

    #[must_use]
    pub fn script_methods(&self) -> &ScriptMethodTable {
        &self.script_methods
    }

    #[must_use]
    pub fn script_metadata(&self) -> Option<&ModuleGraph> {
        self.script_metadata.as_ref()
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
        program.set_script_methods(self.script_methods.clone());
        if let Some(graph) = &self.script_metadata {
            program.set_script_metadata(graph.clone());
        }
        program
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct HotUpdate {
    pub(crate) functions: BTreeMap<FunctionSymbolId, Arc<CodeObject>>,
    pub(crate) script_methods: ScriptMethodTable,
    pub(crate) script_metadata: Option<ModuleGraph>,
    pub(crate) abi: HotReloadAbi,
    pub(crate) changes: AcceptedHotReloadChanges,
}

impl HotUpdate {
    pub(crate) fn new(
        functions: BTreeMap<FunctionSymbolId, Arc<CodeObject>>,
        script_methods: ScriptMethodTable,
        script_metadata: Option<ModuleGraph>,
        abi: HotReloadAbi,
        changes: AcceptedHotReloadChanges,
    ) -> Self {
        Self {
            functions,
            script_methods,
            script_metadata,
            abi,
            changes,
        }
    }
}
