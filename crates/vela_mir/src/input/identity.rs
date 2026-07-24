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
    pub fn service_function_for_node(&self, node: HirNodeId) -> Option<FunctionId> {
        self.service_functions_by_node.get(&node).copied()
    }

    #[must_use]
    pub fn method_for_node(
        &self,
        node: HirNodeId,
        owner: TypeId,
    ) -> Option<MethodExecutableTarget> {
        self.methods_by_node
            .get(&node)?
            .iter()
            .find(|target| target.owner == owner)
            .copied()
    }

    pub fn methods_for_node(&self, node: HirNodeId) -> &[MethodExecutableTarget] {
        self.methods_by_node.get(&node).map_or(&[], Vec::as_slice)
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
    pub fn insert_service_function(
        &mut self,
        node: HirNodeId,
        body: HirBodyId,
        descriptor: CompileFunctionDescriptor,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        if self.snapshot.service_functions_by_node.contains_key(&node) {
            return Err(inconsistent(
                origin,
                format!("duplicate service function node {node:?}"),
            ));
        }
        let function = descriptor.id;
        if self.snapshot.function_descriptor(function).is_some()
            || self.snapshot.functions.contains_key(&function)
        {
            return Err(inconsistent(
                origin,
                format!("duplicate service function #{}", function.get()),
            ));
        }
        self.insert_function_descriptor(descriptor, origin)?;
        self.insert_function(body, CompileFunctionIdentity::Function(function), origin)?;
        self.snapshot
            .service_functions_by_node
            .insert(node, function);
        Ok(())
    }

    pub fn insert_script_function(
        &mut self,
        declaration: HirDeclId,
        body: HirBodyId,
        descriptor: CompileFunctionDescriptor,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        let function = descriptor.id;
        if self.snapshot.functions.contains_key(&function) {
            return Err(inconsistent(
                origin,
                format!("duplicate script function root #{}", function.get()),
            ));
        }
        self.insert_script_function_descriptor(declaration, descriptor, origin)?;
        self.insert_function(body, CompileFunctionIdentity::Function(function), origin)
    }

    pub fn insert_script_function_descriptor(
        &mut self,
        declaration: HirDeclId,
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
        if self.snapshot.function_descriptor(function).is_some() {
            return Err(inconsistent(
                origin,
                format!("duplicate script function #{}", function.get()),
            ));
        }
        self.insert_function_descriptor(descriptor, origin)?;
        self.snapshot
            .functions_by_declaration
            .insert(declaration, function);
        self.snapshot
            .origins
            .function_declarations
            .insert(declaration, origin);
        Ok(())
    }

    pub fn insert_script_method(
        &mut self,
        body: HirBodyId,
        target: MethodExecutableTarget,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        if self.snapshot.functions.contains_key(&target.function)
            || self.snapshot.functions.values().any(|existing| {
                matches!(
                    existing.identity,
                    CompileFunctionIdentity::Method(method)
                        if method.owner == target.owner && method.method == target.method
                )
            })
        {
            return Err(inconsistent(
                origin,
                format!("duplicate script method root #{}", target.function.get()),
            ));
        }
        self.insert_script_method_target(target, origin)?;
        self.insert_function(body, CompileFunctionIdentity::Method(target), origin)?;
        Ok(())
    }

    pub fn insert_script_method_target(
        &mut self,
        target: MethodExecutableTarget,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        let methods = self
            .snapshot
            .methods_by_node
            .entry(target.node)
            .or_default();
        if methods.iter().any(|existing| {
            existing.owner == target.owner
                && (existing.method == target.method || existing.function == target.function)
        }) {
            return Err(inconsistent(
                origin,
                format!(
                    "duplicate script method node {:?} for owner #{}",
                    target.node,
                    target.owner.get()
                ),
            ));
        }
        methods.push(target);
        self.snapshot.origins.method_targets.insert(target, origin);
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
        self.snapshot
            .origins
            .type_declarations
            .insert(declaration, origin);
        Ok(())
    }
}

fn inconsistent(origin: MirSourceOrigin, message: String) -> MirBuildError {
    MirBuildError::InconsistentInput { origin, message }
}
