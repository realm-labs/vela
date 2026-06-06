use std::collections::BTreeMap;
use std::sync::Arc;

use vela_bytecode::{
    CodeObject, Program,
    script_methods::{ScriptMethod, ScriptMethodTable},
};
use vela_common::MethodId;
use vela_hir::module_graph::ModuleGraph;

use crate::abi::HotReloadAbi;
use crate::profile::{FunctionProfile, ProgramProfile};
use crate::report::AcceptedHotReloadChanges;
use crate::symbol::{FunctionSymbolId, ProgramVersionId};

#[derive(Clone, Debug, PartialEq)]
pub struct ProgramVersion {
    pub id: ProgramVersionId,
    pub(crate) functions: BTreeMap<FunctionSymbolId, Arc<CodeObject>>,
    pub(crate) script_methods: ScriptMethodTable,
    pub(crate) script_metadata: Option<ModuleGraph>,
    pub(crate) abi: HotReloadAbi,
    pub(crate) profile: ProgramProfile,
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
        let profile = ProgramProfile::from_functions(&functions);
        Self {
            id,
            functions,
            script_methods,
            script_metadata,
            abi,
            profile,
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
    pub fn script_method(&self, type_name: &str, method: &str) -> Option<&ScriptMethod> {
        self.script_methods.get(type_name, method)
    }

    #[must_use]
    pub fn script_method_by_id(
        &self,
        type_name: &str,
        method_id: MethodId,
    ) -> Option<&ScriptMethod> {
        self.script_methods.get_by_id(type_name, method_id)
    }

    #[must_use]
    pub fn script_method_function(&self, type_name: &str, method: &str) -> Option<Arc<CodeObject>> {
        let method = self.script_method(type_name, method)?;
        self.function(&method.function)
    }

    #[must_use]
    pub fn script_method_function_by_id(
        &self,
        type_name: &str,
        method_id: MethodId,
    ) -> Option<Arc<CodeObject>> {
        let method = self.script_method_by_id(type_name, method_id)?;
        self.function(&method.function)
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
    pub fn profile(&self) -> &ProgramProfile {
        &self.profile
    }

    #[must_use]
    pub fn function_profile(&self, name: &str) -> Option<&FunctionProfile> {
        self.profile.function(name)
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

    #[must_use]
    pub fn function(&self, name: &str) -> Option<Arc<CodeObject>> {
        self.functions.get(&FunctionSymbolId::new(name)).cloned()
    }

    pub fn function_names(&self) -> impl Iterator<Item = &str> {
        self.functions.keys().map(|name| name.0.as_str())
    }

    pub fn changed_function_names(&self) -> impl Iterator<Item = &str> {
        self.changes
            .changed_functions
            .iter()
            .map(|name| name.0.as_str())
    }

    #[must_use]
    pub fn changed_modules(&self) -> &[String] {
        &self.changes.changed_modules
    }

    #[must_use]
    pub fn impacted_modules(&self) -> &[String] {
        &self.changes.impacted_modules
    }

    #[must_use]
    pub fn script_methods(&self) -> &ScriptMethodTable {
        &self.script_methods
    }

    #[must_use]
    pub fn script_method(&self, type_name: &str, method: &str) -> Option<&ScriptMethod> {
        self.script_methods.get(type_name, method)
    }

    #[must_use]
    pub fn script_method_by_id(
        &self,
        type_name: &str,
        method_id: MethodId,
    ) -> Option<&ScriptMethod> {
        self.script_methods.get_by_id(type_name, method_id)
    }

    #[must_use]
    pub fn script_method_function(&self, type_name: &str, method: &str) -> Option<Arc<CodeObject>> {
        let method = self.script_method(type_name, method)?;
        self.function(&method.function)
    }

    #[must_use]
    pub fn script_method_function_by_id(
        &self,
        type_name: &str,
        method_id: MethodId,
    ) -> Option<Arc<CodeObject>> {
        let method = self.script_method_by_id(type_name, method_id)?;
        self.function(&method.function)
    }

    #[must_use]
    pub fn script_metadata(&self) -> Option<&ModuleGraph> {
        self.script_metadata.as_ref()
    }
}
