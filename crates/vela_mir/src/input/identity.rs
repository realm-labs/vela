use vela_def::{FunctionId, TypeId};
use vela_hir::ids::{HirBodyId, HirDeclId, HirNodeId};

use crate::{
    CompileFunctionDescriptor, CompileFunctionIdentity, CompileFunctionTarget,
    CompileTypeDescriptor, MethodExecutableTarget, MirBuildError, MirSourceOrigin,
};

use super::{CompileTargetSnapshot, CompileTargetSnapshotBuilder};

impl CompileTargetSnapshot {
    #[must_use]
    pub fn function_for_declaration(&self, declaration: HirDeclId) -> Option<FunctionId> {
        self.functions_by_declaration.get(&declaration).copied()
    }

    #[must_use]
    pub fn method_for_node(&self, node: HirNodeId) -> Option<MethodExecutableTarget> {
        self.methods_by_node.get(&node).copied()
    }

    #[must_use]
    pub fn type_for_declaration(&self, declaration: HirDeclId) -> Option<TypeId> {
        self.types_by_declaration.get(&declaration).copied()
    }

    #[must_use]
    pub fn type_by_name(&self, canonical_name: &str) -> Option<&CompileTypeDescriptor> {
        self.types_by_name
            .get(canonical_name)
            .and_then(|id| self.type_descriptor(*id))
    }

    pub fn compilation_roots(&self) -> impl Iterator<Item = (FunctionId, &CompileFunctionTarget)> {
        self.functions.iter().map(|(id, target)| (*id, target))
    }
}

impl CompileTargetSnapshotBuilder {
    pub fn insert_script_function(
        &mut self,
        declaration: HirDeclId,
        body: HirBodyId,
        descriptor: CompileFunctionDescriptor,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        if self
            .snapshot
            .functions_by_declaration
            .contains_key(&declaration)
        {
            return Err(inconsistent(
                origin,
                format!("duplicate script function declaration {declaration:?}"),
            ));
        }
        let function = descriptor.id;
        if self.snapshot.functions.contains_key(&function)
            || self.snapshot.function_descriptor(function).is_some()
        {
            return Err(inconsistent(
                origin,
                format!("duplicate script function #{}", function.get()),
            ));
        }
        self.insert_function_descriptor(descriptor, origin)?;
        self.insert_function(body, CompileFunctionIdentity::Function(function), origin)?;
        self.snapshot
            .functions_by_declaration
            .insert(declaration, function);
        Ok(())
    }

    pub fn insert_script_method(
        &mut self,
        body: HirBodyId,
        target: MethodExecutableTarget,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        if self.snapshot.methods_by_node.contains_key(&target.node) {
            return Err(inconsistent(
                origin,
                format!("duplicate script method node {:?}", target.node),
            ));
        }
        self.insert_function(body, CompileFunctionIdentity::Method(target), origin)?;
        self.snapshot.methods_by_node.insert(target.node, target);
        Ok(())
    }

    pub fn insert_script_type(
        &mut self,
        declaration: HirDeclId,
        descriptor: CompileTypeDescriptor,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        if self
            .snapshot
            .types_by_declaration
            .contains_key(&declaration)
        {
            return Err(inconsistent(
                origin,
                format!("duplicate script type declaration {declaration:?}"),
            ));
        }
        let type_id = descriptor.id;
        self.insert_type_descriptor(descriptor, origin)?;
        self.snapshot
            .types_by_declaration
            .insert(declaration, type_id);
        Ok(())
    }
}

fn inconsistent(origin: MirSourceOrigin, message: String) -> MirBuildError {
    MirBuildError::InconsistentInput { origin, message }
}
