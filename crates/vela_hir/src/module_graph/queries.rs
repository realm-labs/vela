use std::collections::BTreeSet;

use vela_common::{Diagnostic, SourceId, Span};
use vela_package::{ModuleKey, ModulePath, PackageId};

use crate::attributes::HirAttribute;
use crate::binding::BindingMap;
use crate::body::{HirBody, HirField, HirIndex, HirPath, HirPathKind};
use crate::ids::{HirBodyId, HirDeclId, HirExprId, HirLocalId, HirNodeId, HirPatternId, ModuleId};
use crate::type_hint::{
    ConstMetadata, EnumShape, FunctionSignature, GlobalMetadata, ImplMetadata, StructShape,
    TraitShape,
};

use super::{
    Declaration, DeclarationIndex, DeclarationKind, Import, ImportResolution, ModuleGraph,
};

impl ModuleGraph {
    #[must_use]
    pub fn module(&self, module: ModuleId) -> Option<&DeclarationIndex> {
        self.modules
            .get(usize::try_from(module.get()).ok()?)
            .map(|module| &module.declarations)
    }

    #[must_use]
    pub fn module_path(&self, module: ModuleId) -> Option<&ModulePath> {
        self.module_key(module).map(|key| &key.path)
    }

    #[must_use]
    pub fn module_package(&self, module: ModuleId) -> Option<&PackageId> {
        self.module_key(module).map(|key| &key.package)
    }

    #[must_use]
    pub fn source_package(&self, source: SourceId) -> Option<&PackageId> {
        self.modules
            .iter()
            .find(|module| module.source == source)
            .map(|module| &module.key.package)
    }

    #[must_use]
    pub fn module_key(&self, module: ModuleId) -> Option<&ModuleKey> {
        self.modules
            .get(usize::try_from(module.get()).ok()?)
            .map(|module| &module.key)
    }

    #[must_use]
    pub fn module_id(&self, key: &ModuleKey) -> Option<ModuleId> {
        self.module_by_key.get(key).copied()
    }

    pub fn module_ids(&self) -> impl Iterator<Item = ModuleId> + '_ {
        self.modules.iter().map(|module| module.id)
    }

    #[must_use]
    pub fn module_source_hash(&self, module: ModuleId) -> Option<u64> {
        self.modules
            .get(usize::try_from(module.get()).ok()?)
            .and_then(|module| module.source_hash)
    }

    #[must_use]
    pub fn declaration(&self, declaration: HirDeclId) -> Option<&Declaration> {
        self.declarations.get(&declaration)
    }

    /// Returns a declaration's canonical source symbol without a package
    /// prefix, including its module qualification when one exists.
    #[must_use]
    pub fn qualified_declaration_name(&self, declaration: HirDeclId) -> Option<String> {
        let declaration = self.declaration(declaration)?;
        let path = self.module_path(declaration.module)?;
        if path.segments().is_empty() {
            Some(declaration.name.clone())
        } else {
            Some(format!("{}::{}", path.join(), declaration.name))
        }
    }

    #[must_use]
    pub fn const_metadata(&self, declaration: HirDeclId) -> Option<&ConstMetadata> {
        self.const_metadata.get(&declaration)
    }

    #[must_use]
    pub fn global_metadata(&self, declaration: HirDeclId) -> Option<&GlobalMetadata> {
        self.global_metadata.get(&declaration)
    }

    #[must_use]
    pub fn declaration_attrs(&self, declaration: HirDeclId) -> &[HirAttribute] {
        self.declaration_attrs
            .get(&declaration)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    #[must_use]
    pub fn bindings(&self, declaration: HirDeclId) -> Option<&BindingMap> {
        self.bindings.get(&declaration)
    }

    #[must_use]
    pub fn const_initializer_bindings(&self, declaration: HirDeclId) -> Option<&BindingMap> {
        self.const_initializer_bindings.get(&declaration)
    }

    #[must_use]
    pub fn schema_field_default_bindings(&self, body: HirBodyId) -> Option<&BindingMap> {
        self.schema_field_default_bindings.get(&body)
    }

    /// Returns the canonical binding map that owns a HIR body.
    ///
    /// Lambda and parameter-default bodies share the binding generation of
    /// their nearest enclosing executable body. Keeping this lookup on the
    /// module graph prevents downstream consumers from rebuilding owner joins.
    #[must_use]
    pub fn bindings_for_body(&self, body: HirBodyId) -> Option<&BindingMap> {
        match self.body(body)?.owner {
            crate::body::HirBodyOwner::Declaration(declaration) => self.bindings(declaration),
            crate::body::HirBodyOwner::ConstInitializer(declaration) => {
                self.const_initializer_bindings(declaration)
            }
            crate::body::HirBodyOwner::SchemaFieldDefault(_) => {
                self.schema_field_default_bindings(body)
            }
            crate::body::HirBodyOwner::TraitDefaultMethod(method) => {
                self.trait_default_method_bindings(method)
            }
            crate::body::HirBodyOwner::ImplMethod(method) => self.impl_method_bindings(method),
            crate::body::HirBodyOwner::Lambda { parent, .. }
            | crate::body::HirBodyOwner::ParameterDefault { parent, .. } => {
                self.bindings_for_body(parent)
            }
        }
    }

    #[must_use]
    pub fn body(&self, body: HirBodyId) -> Option<&HirBody> {
        self.bodies.get(&body)
    }

    pub fn bodies(&self) -> impl Iterator<Item = &HirBody> {
        self.bodies.values()
    }

    pub fn bodies_in_source(&self, source: SourceId) -> impl Iterator<Item = &HirBody> {
        self.body_ids_by_source
            .get(&source)
            .into_iter()
            .flatten()
            .filter_map(|body| self.bodies.get(body))
    }

    #[must_use]
    pub fn body_containing_offset(&self, source: SourceId, offset: u32) -> Option<&HirBody> {
        self.bodies_in_source(source)
            .filter(|body| body.origin.span.contains(offset))
            .min_by_key(|body| body.origin.span.len())
    }

    pub fn body_and_ancestors(&self, body: HirBodyId) -> impl Iterator<Item = &HirBody> {
        std::iter::successors(self.body(body), |body| {
            let parent = match &body.owner {
                crate::body::HirBodyOwner::Lambda { parent, .. }
                | crate::body::HirBodyOwner::ParameterDefault { parent, .. } => *parent,
                crate::body::HirBodyOwner::Declaration(_)
                | crate::body::HirBodyOwner::ConstInitializer(_)
                | crate::body::HirBodyOwner::SchemaFieldDefault(_)
                | crate::body::HirBodyOwner::TraitDefaultMethod(_)
                | crate::body::HirBodyOwner::ImplMethod(_) => return None,
            };
            self.body(parent)
        })
    }

    #[must_use]
    pub fn local_binding(&self, local: HirLocalId) -> Option<&crate::binding::LocalBinding> {
        self.bindings
            .values()
            .chain(self.const_initializer_bindings.values())
            .chain(self.schema_field_default_bindings.values())
            .chain(self.trait_default_method_bindings.values())
            .chain(self.impl_method_bindings.values())
            .find_map(|bindings| bindings.local(local))
    }

    #[must_use]
    pub fn expression_at_span(&self, span: Span) -> Option<HirExprId> {
        self.bodies
            .values()
            .flat_map(|body| body.expressions.values())
            .find_map(|expression| (expression.origin.span == span).then_some(expression.id))
    }

    #[must_use]
    pub fn pattern_at_span(&self, span: Span) -> Option<HirPatternId> {
        self.bodies
            .values()
            .flat_map(|body| body.patterns.values())
            .find_map(|pattern| (pattern.origin.span == span).then_some(pattern.id))
    }

    #[must_use]
    pub fn expression_span(&self, expression: HirExprId) -> Option<Span> {
        self.bodies
            .values()
            .find_map(|body| body.expressions.get(&expression))
            .map(|expression| expression.origin.span)
    }

    #[must_use]
    pub fn pattern_span(&self, pattern: HirPatternId) -> Option<Span> {
        self.bodies
            .values()
            .find_map(|body| body.patterns.get(&pattern))
            .map(|pattern| pattern.origin.span)
    }

    #[must_use]
    pub fn call_callee(&self, expression: HirExprId) -> Option<HirExprId> {
        self.bodies
            .values()
            .find_map(|body| body.call(expression))
            .map(|call| call.callee)
    }

    #[must_use]
    pub fn index_for_expression(&self, expression: HirExprId) -> Option<&HirIndex> {
        self.bodies.values().find_map(|body| body.index(expression))
    }

    #[must_use]
    pub fn field_at_member_span(&self, span: Span) -> Option<&HirField> {
        self.bodies
            .values()
            .flat_map(|body| body.fields().map(|(_, field)| field))
            .find(|field| field.member_origin.span == span)
    }

    pub fn fields_in_source(&self, source: SourceId) -> impl Iterator<Item = &HirField> + '_ {
        self.bodies_in_source(source)
            .flat_map(|body| body.fields().map(|(_, field)| field))
    }

    pub fn member_calls_in_source(&self, source: SourceId) -> impl Iterator<Item = &HirField> + '_ {
        self.bodies_in_source(source).flat_map(move |body| {
            body.calls().filter_map(move |(_, call)| {
                body.field(call.callee)
                    .filter(|field| field.member_origin.source == source)
            })
        })
    }

    pub fn paths_in_source(&self, source: SourceId) -> impl Iterator<Item = &HirPath> + '_ {
        self.bodies_in_source(source)
            .flat_map(|body| body.paths.values())
    }

    pub fn paths_in_source_by_kind(
        &self,
        source: SourceId,
        kind: HirPathKind,
    ) -> impl Iterator<Item = &HirPath> + '_ {
        self.paths_in_source(source)
            .filter(move |path| path.kind == kind)
    }

    #[must_use]
    pub fn expression_containing_span(&self, span: Span) -> Option<HirExprId> {
        self.bodies
            .values()
            .flat_map(|body| body.expressions.values())
            .filter(|expression| {
                expression.origin.span.source == span.source
                    && expression.origin.span.start <= span.start
                    && span.end <= expression.origin.span.end
            })
            .min_by_key(|expression| {
                expression
                    .origin
                    .span
                    .end
                    .saturating_sub(expression.origin.span.start)
            })
            .map(|expression| expression.id)
    }

    #[must_use]
    pub fn function_body(&self, declaration: HirDeclId) -> Option<&HirBody> {
        self.function_bodies
            .get(&declaration)
            .and_then(|body| self.body(*body))
    }

    #[must_use]
    pub fn const_initializer_body(&self, declaration: HirDeclId) -> Option<&HirBody> {
        self.const_initializer_bodies
            .get(&declaration)
            .and_then(|body| self.body(*body))
    }

    #[must_use]
    pub fn trait_default_method_body(&self, method: HirNodeId) -> Option<&HirBody> {
        self.trait_default_method_bodies
            .get(&method)
            .and_then(|body| self.body(*body))
    }

    #[must_use]
    pub fn impl_method_body(&self, method: HirNodeId) -> Option<&HirBody> {
        self.impl_method_bodies
            .get(&method)
            .and_then(|body| self.body(*body))
    }

    #[must_use]
    pub fn function_signature(&self, declaration: HirDeclId) -> Option<&FunctionSignature> {
        self.function_signatures.get(&declaration)
    }

    #[must_use]
    pub fn struct_shape(&self, declaration: HirDeclId) -> Option<&StructShape> {
        self.struct_shapes.get(&declaration)
    }

    #[must_use]
    pub fn enum_shape(&self, declaration: HirDeclId) -> Option<&EnumShape> {
        self.enum_shapes.get(&declaration)
    }

    #[must_use]
    pub fn trait_shape(&self, declaration: HirDeclId) -> Option<&TraitShape> {
        self.trait_shapes.get(&declaration)
    }

    pub fn declarations(&self) -> impl Iterator<Item = &Declaration> {
        self.declarations.values()
    }

    #[must_use]
    pub fn declarations_by_name(&self, name: &str) -> Vec<&Declaration> {
        self.declarations_by_name
            .get(name)
            .into_iter()
            .flat_map(|declarations| declarations.iter())
            .filter_map(|declaration| self.declarations.get(declaration))
            .collect()
    }

    #[must_use]
    pub fn declarations_by_name_prefix(&self, prefix: &str) -> Vec<&Declaration> {
        if prefix.is_empty() {
            return self.declarations.values().collect();
        }

        self.declarations_by_name
            .range(prefix.to_owned()..)
            .take_while(|(name, _)| name.starts_with(prefix))
            .flat_map(|(_, declarations)| declarations.iter())
            .filter_map(|declaration| self.declarations.get(declaration))
            .collect()
    }

    #[must_use]
    pub fn declarations_by_kind(&self, kind: DeclarationKind) -> Vec<&Declaration> {
        self.declarations_by_kind
            .get(&kind)
            .into_iter()
            .flat_map(|declarations| declarations.iter())
            .filter_map(|declaration| self.declarations.get(declaration))
            .collect()
    }

    #[must_use]
    pub fn declaration_by_type_path(
        &self,
        path: &[String],
        current_module: &ModuleKey,
        kind: DeclarationKind,
    ) -> Option<&Declaration> {
        let (name, module_segments) = path.split_last()?;
        let module_key = if module_segments.is_empty() {
            current_module.clone()
        } else {
            self.import_module_key_from(current_module, module_segments)
        };
        let module = self.module_id(&module_key)?;
        let declaration = self.module(module)?.get(name)?;
        self.declaration(declaration)
            .filter(|declaration| declaration.kind == kind)
    }

    #[must_use]
    pub fn resolve_visible_declaration_path(
        &self,
        requesting_module: ModuleId,
        path: &[String],
        kind: DeclarationKind,
    ) -> Option<&Declaration> {
        if let [name] = path {
            if let Some(declaration) = self
                .module(requesting_module)
                .and_then(|declarations| declarations.get(name))
                .and_then(|declaration| self.declaration(declaration))
                .filter(|declaration| declaration.kind == kind)
            {
                return Some(declaration);
            }
            return self.imports(requesting_module)?.iter().find_map(|import| {
                let binding = super::names::import_binding_name(import)?;
                if binding != *name {
                    return None;
                }
                let ImportResolution::Declaration(declaration) = import.resolution?;
                self.declaration(declaration)
                    .filter(|declaration| declaration.kind == kind)
            });
        }
        let current = self.module_key(requesting_module)?;
        self.declaration_by_type_path(path, current, kind)
    }

    #[must_use]
    pub fn resolve_module_path(
        &self,
        current_module: &ModuleKey,
        path: &[String],
    ) -> Option<ModuleKey> {
        Some(self.import_module_key_from(current_module, path))
    }

    #[must_use]
    pub fn declarations_by_path_base(
        &self,
        package: &PackageId,
        base: &str,
        kind: DeclarationKind,
    ) -> Vec<&Declaration> {
        let path = ModulePath::from_qualified(base);
        if path.segments().len() > 1 {
            return self
                .declaration_by_type_path(
                    path.segments(),
                    &ModuleKey::new(package.clone(), ModulePath::root()),
                    kind,
                )
                .into_iter()
                .collect();
        }
        self.declarations_by_name(base)
            .into_iter()
            .filter(|declaration| declaration.kind == kind)
            .collect()
    }

    #[must_use]
    pub fn declarations_in_module(&self, module: ModuleId) -> Vec<&Declaration> {
        let Ok(index) = usize::try_from(module.get()) else {
            return Vec::new();
        };
        let Some(module) = self.modules.get(index) else {
            return Vec::new();
        };
        module
            .declarations
            .names()
            .filter_map(|name| module.declarations.get(name))
            .filter_map(|declaration| self.declarations.get(&declaration))
            .collect()
    }

    #[must_use]
    pub fn module_child_segments(&self, base: &ModuleKey) -> Vec<&str> {
        self.module_children
            .get(base)
            .map(|children| children.iter().map(String::as_str).collect())
            .unwrap_or_default()
    }

    #[must_use]
    pub fn module_completion_labels(&self) -> Vec<String> {
        let mut labels = BTreeSet::new();
        let packages = self
            .modules
            .iter()
            .map(|module| module.key.package.clone())
            .collect::<BTreeSet<_>>();
        for package in packages {
            self.collect_module_completion_labels(
                &ModuleKey::new(package, ModulePath::root()),
                String::new(),
                &mut labels,
            );
        }
        labels.into_iter().collect()
    }

    #[must_use]
    pub fn impl_metadata(&self, declaration: HirDeclId) -> Option<&ImplMetadata> {
        self.impl_metadata.get(&declaration)
    }

    #[must_use]
    pub fn trait_default_method_bindings(&self, method: HirNodeId) -> Option<&BindingMap> {
        self.trait_default_method_bindings.get(&method)
    }

    #[must_use]
    pub fn impl_method_bindings(&self, method: HirNodeId) -> Option<&BindingMap> {
        self.impl_method_bindings.get(&method)
    }

    #[must_use]
    pub fn imports(&self, module: ModuleId) -> Option<&[Import]> {
        self.modules
            .get(usize::try_from(module.get()).ok()?)
            .map(|module| module.imports.as_slice())
    }

    pub fn dependent_modules(
        &self,
        roots: impl IntoIterator<Item = ModuleId>,
    ) -> BTreeSet<ModuleId> {
        let mut impacted = roots.into_iter().collect::<BTreeSet<_>>();
        let mut pending = impacted.iter().copied().collect::<Vec<_>>();

        while let Some(changed) = pending.pop() {
            for module in &self.modules {
                if impacted.contains(&module.id) {
                    continue;
                }
                if self.module_imports_module(module, changed) {
                    impacted.insert(module.id);
                    pending.push(module.id);
                }
            }
        }

        impacted
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}
