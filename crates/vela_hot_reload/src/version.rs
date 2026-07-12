use std::sync::Arc;

use vela_bytecode::{
    LinkedArtifact, LinkedProgram, ProgramImage, UnlinkedCodeObject,
    script_methods::{ScriptMethod, ScriptMethodTable},
};
use vela_def::MethodId;
use vela_hir::module_graph::ModuleGraph;

use crate::abi::HotReloadAbi;
use crate::profile::{FunctionProfile, ProgramProfile};
use crate::report::AcceptedHotReloadChanges;
use crate::symbol::ProgramVersionId;

#[derive(Clone, Debug)]
pub struct RestrictedJitInput<'a> {
    pub generation: vela_bytecode::ExecutableGenerationId,
    pub handle: vela_bytecode::ScriptFunctionHandle,
    pub linked: &'a vela_bytecode::LinkedCodeObject,
    pub mir_owner: &'a vela_mir::OwnedVerifiedMirProgram,
    pub mir_function: vela_mir::MirFunctionId,
    pub eligibility: vela_mir::MirJitEligibility,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgramVersion {
    pub id: ProgramVersionId,
    pub(crate) abi: HotReloadAbi,
    pub(crate) artifact: Arc<LinkedArtifact>,
}

impl ProgramVersion {
    pub(crate) fn from_linked_artifact(
        id: ProgramVersionId,
        abi: HotReloadAbi,
        artifact: Arc<LinkedArtifact>,
    ) -> Option<Self> {
        artifact.verified_mir()?;
        Some(Self { id, abi, artifact })
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
        self.artifact
            .verified_mir()
            .expect("ProgramVersion is constructed only from a MIR-bound artifact")
    }

    #[must_use]
    pub fn executable_generation_id(&self) -> vela_bytecode::ExecutableGenerationId {
        self.artifact.generation()
    }

    #[must_use]
    pub fn restricted_jit_input(
        &self,
        handle: vela_bytecode::ScriptFunctionHandle,
    ) -> Option<RestrictedJitInput<'_>> {
        let layout = self.artifact.mir_executable(handle)?;
        let mir_owner = self.verified_mir().root(layout.root)?.as_ref();
        let linked = self.artifact.program().function(handle)?;
        Some(RestrictedJitInput {
            generation: self.artifact.generation(),
            handle,
            linked,
            mir_owner,
            mir_function: layout.function,
            eligibility: vela_mir::restricted_jit_eligibility(mir_owner, layout.function),
        })
    }

    #[must_use]
    pub fn restricted_entry_jit_input(&self, name: &str) -> Option<RestrictedJitInput<'_>> {
        let handle = self.artifact.program().entry_point_by_name(name)?;
        self.restricted_jit_input(handle)
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
    pub(crate) artifact: Arc<LinkedArtifact>,
}

impl HotUpdate {
    pub(crate) fn new(
        abi: HotReloadAbi,
        changes: AcceptedHotReloadChanges,
        artifact: Arc<LinkedArtifact>,
    ) -> Self {
        Self {
            abi,
            changes,
            artifact,
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
    pub fn linked_artifact(&self) -> &Arc<LinkedArtifact> {
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
