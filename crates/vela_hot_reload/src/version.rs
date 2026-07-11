use std::sync::Arc;

use vela_bytecode::{
    LinkedArtifact, LinkedProgram, ProgramImage, UnlinkedCodeObject,
    compiler::CompiledProgram,
    script_methods::{ScriptMethod, ScriptMethodTable},
};
use vela_def::MethodId;
use vela_hir::module_graph::ModuleGraph;

use crate::abi::HotReloadAbi;
use crate::profile::{FunctionProfile, ProgramProfile};
use crate::report::AcceptedHotReloadChanges;
use crate::symbol::ProgramVersionId;

#[derive(Clone, Debug, PartialEq)]
pub struct ProgramVersion {
    pub id: ProgramVersionId,
    pub(crate) abi: HotReloadAbi,
    pub(crate) artifact: Arc<LinkedArtifact>,
    pub(crate) verified_mir: Arc<vela_mir::OwnedVerifiedMirBundle>,
}

impl ProgramVersion {
    #[must_use]
    pub fn from_linked_program(
        id: ProgramVersionId,
        program: CompiledProgram,
        artifact: LinkedArtifact,
    ) -> Self {
        Self::from_linked_program_with_abi(id, program, HotReloadAbi::empty(), artifact)
    }

    #[must_use]
    pub fn from_linked_program_with_abi(
        id: ProgramVersionId,
        program: CompiledProgram,
        abi: HotReloadAbi,
        artifact: LinkedArtifact,
    ) -> Self {
        let (_, verified_mir) = program.into_parts();
        Self {
            id,
            abi,
            artifact: Arc::new(artifact),
            verified_mir,
        }
    }

    #[must_use]
    pub fn function(&self, name: &str) -> Option<Arc<UnlinkedCodeObject>> {
        self.artifact
            .image()
            .function_by_name(name)
            .cloned()
            .map(Arc::new)
    }

    pub fn function_names(&self) -> impl Iterator<Item = &str> {
        self.artifact.image().entry_function_names()
    }

    #[must_use]
    pub fn script_methods(&self) -> &ScriptMethodTable {
        self.artifact.image().script_methods()
    }

    #[must_use]
    pub fn global_names(&self) -> &[String] {
        self.artifact.image().global_names()
    }

    #[must_use]
    pub fn script_method(&self, type_name: &str, method: &str) -> Option<&ScriptMethod> {
        self.artifact
            .image()
            .script_methods()
            .get(type_name, method)
    }

    #[must_use]
    pub fn script_method_by_id(
        &self,
        type_name: &str,
        method_id: MethodId,
    ) -> Option<&ScriptMethod> {
        self.artifact
            .image()
            .script_methods()
            .get_by_id(type_name, method_id)
    }

    #[must_use]
    pub fn script_method_function(
        &self,
        type_name: &str,
        method: &str,
    ) -> Option<Arc<UnlinkedCodeObject>> {
        let method = self.script_method(type_name, method)?;
        self.function(&method.function)
    }

    #[must_use]
    pub fn script_method_function_by_id(
        &self,
        type_name: &str,
        method_id: MethodId,
    ) -> Option<Arc<UnlinkedCodeObject>> {
        let method = self.script_method_by_id(type_name, method_id)?;
        self.function(&method.function)
    }

    #[must_use]
    pub fn script_metadata(&self) -> Option<&ModuleGraph> {
        self.artifact.image().script_metadata()
    }

    #[must_use]
    pub fn abi(&self) -> &HotReloadAbi {
        &self.abi
    }

    #[must_use]
    pub fn profile(&self) -> ProgramProfile {
        ProgramProfile::from_artifact(&self.artifact)
    }

    #[must_use]
    pub fn program_image(&self) -> &ProgramImage {
        self.artifact.image()
    }

    #[must_use]
    pub fn linked_program(&self) -> &LinkedProgram {
        self.artifact.program()
    }

    #[must_use]
    pub fn linked_artifact(&self) -> &Arc<LinkedArtifact> {
        &self.artifact
    }

    #[must_use]
    pub fn verified_mir(&self) -> &Arc<vela_mir::OwnedVerifiedMirBundle> {
        &self.verified_mir
    }

    #[must_use]
    pub fn executable_generation_id(&self) -> vela_bytecode::ExecutableGenerationId {
        self.artifact.generation()
    }

    #[must_use]
    pub fn function_profile(&self, name: &str) -> Option<FunctionProfile> {
        self.profile().function(name).cloned()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct HotUpdate {
    pub(crate) abi: HotReloadAbi,
    pub(crate) changes: AcceptedHotReloadChanges,
    pub(crate) artifact: LinkedArtifact,
    pub(crate) verified_mir: Arc<vela_mir::OwnedVerifiedMirBundle>,
}

impl HotUpdate {
    pub(crate) fn new(
        abi: HotReloadAbi,
        changes: AcceptedHotReloadChanges,
        artifact: LinkedArtifact,
        verified_mir: Arc<vela_mir::OwnedVerifiedMirBundle>,
    ) -> Self {
        Self {
            abi,
            changes,
            artifact,
            verified_mir,
        }
    }

    #[must_use]
    pub fn function(&self, name: &str) -> Option<Arc<UnlinkedCodeObject>> {
        self.artifact
            .image()
            .function_by_name(name)
            .cloned()
            .map(Arc::new)
    }

    pub fn function_names(&self) -> impl Iterator<Item = &str> {
        self.artifact.image().entry_function_names()
    }

    pub fn changed_function_names(&self) -> impl Iterator<Item = &str> {
        self.changes
            .changed_functions
            .iter()
            .map(|name| name.0.as_str())
    }

    #[must_use]
    pub fn linked_program(&self) -> &LinkedProgram {
        self.artifact.program()
    }

    #[must_use]
    pub fn linked_artifact(&self) -> &LinkedArtifact {
        &self.artifact
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
        self.artifact.image().script_methods()
    }

    #[must_use]
    pub fn script_method(&self, type_name: &str, method: &str) -> Option<&ScriptMethod> {
        self.script_methods().get(type_name, method)
    }

    #[must_use]
    pub fn script_method_by_id(
        &self,
        type_name: &str,
        method_id: MethodId,
    ) -> Option<&ScriptMethod> {
        self.script_methods().get_by_id(type_name, method_id)
    }

    #[must_use]
    pub fn script_method_function(
        &self,
        type_name: &str,
        method: &str,
    ) -> Option<Arc<UnlinkedCodeObject>> {
        let method = self.script_method(type_name, method)?;
        self.function(&method.function)
    }

    #[must_use]
    pub fn script_method_function_by_id(
        &self,
        type_name: &str,
        method_id: MethodId,
    ) -> Option<Arc<UnlinkedCodeObject>> {
        let method = self.script_method_by_id(type_name, method_id)?;
        self.function(&method.function)
    }

    #[must_use]
    pub fn script_metadata(&self) -> Option<&ModuleGraph> {
        self.artifact.image().script_metadata()
    }
}
