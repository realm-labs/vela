use std::collections::BTreeMap;

use vela_common::Span;

use crate::{
    ids::{HirBodyId, HirDeclId, HirExprId, HirLocalId},
    type_hint::HirTypeHint,
};

mod name_candidates;
mod syntax_binding;

pub(crate) use syntax_binding::{
    SyntaxExpressionBindingInput, SyntaxFunctionBindingInput, bind_syntax_expression_body,
    bind_syntax_function,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalBinding {
    pub id: HirLocalId,
    pub name: String,
    pub kind: LocalBindingKind,
    pub type_hint: Option<HirTypeHint>,
    pub span: Span,
    pub scope_span: Option<Span>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalBindingKind {
    Parameter,
    Let,
    For,
    LambdaParameter,
    Pattern,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BindingResolution {
    Local(HirLocalId),
    Declaration(HirDeclId),
    Import(String),
    QualifiedPath(Vec<String>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceLexicalCapability {
    Base,
    Pinned,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConstructorResolution {
    Declaration(HirDeclId),
    Dynamic(Vec<String>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ImportBinding {
    pub name: String,
    pub declaration: Option<HirDeclId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BindingMap {
    pub declaration: HirDeclId,
    body: HirBodyId,
    pub(crate) locals: BTreeMap<HirLocalId, LocalBinding>,
    pub(crate) locals_by_name: BTreeMap<String, Vec<HirLocalId>>,
    pub(crate) resolutions: BTreeMap<HirExprId, BindingResolution>,
    pub(crate) pattern_resolutions: BTreeMap<Vec<String>, BindingResolution>,
    pub(crate) pending_constructor_paths: BTreeMap<HirExprId, Vec<String>>,
    pub(crate) pending_pattern_paths: BTreeMap<Vec<String>, Vec<String>>,
    pub(crate) service_capabilities: BTreeMap<HirExprId, ServiceLexicalCapability>,
}

impl BindingMap {
    #[must_use]
    pub const fn body(&self) -> HirBodyId {
        self.body
    }

    #[must_use]
    pub fn local(&self, local: HirLocalId) -> Option<&LocalBinding> {
        self.locals.get(&local)
    }

    pub fn locals(&self) -> impl Iterator<Item = &LocalBinding> {
        self.locals.values()
    }

    /// Projects a declaration source range to its stable local identity.
    ///
    /// Editor callers should use the returned ID for semantic work and keep
    /// the source range only for protocol projection.
    #[must_use]
    pub fn local_containing_source_range(&self, start: usize, end: usize) -> Option<HirLocalId> {
        self.locals
            .values()
            .filter(|binding| {
                usize::try_from(binding.span.start)
                    .is_ok_and(|binding_start| binding_start <= start)
                    && usize::try_from(binding.span.end).is_ok_and(|binding_end| end <= binding_end)
            })
            .min_by_key(|binding| binding.span.end.saturating_sub(binding.span.start))
            .map(|binding| binding.id)
    }

    #[must_use]
    pub fn locals_named(&self, name: &str) -> &[HirLocalId] {
        self.locals_by_name
            .get(name)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    #[must_use]
    pub fn resolution(&self, expression: HirExprId) -> Option<&BindingResolution> {
        self.resolutions.get(&expression)
    }

    pub fn resolutions(&self) -> impl Iterator<Item = (HirExprId, &BindingResolution)> {
        self.resolutions
            .iter()
            .map(|(expression, resolution)| (*expression, resolution))
    }

    #[must_use]
    pub fn service_capability(&self, expression: HirExprId) -> Option<ServiceLexicalCapability> {
        self.service_capabilities.get(&expression).copied()
    }

    pub fn service_capabilities(
        &self,
    ) -> impl Iterator<Item = (HirExprId, ServiceLexicalCapability)> + '_ {
        self.service_capabilities
            .iter()
            .map(|(expression, capability)| (*expression, *capability))
    }

    #[must_use]
    pub fn pattern_resolution(&self, path: &[String]) -> Option<&BindingResolution> {
        self.pattern_resolutions.get(path)
    }

    pub fn pattern_resolutions(&self) -> impl Iterator<Item = (&[String], &BindingResolution)> {
        self.pattern_resolutions
            .iter()
            .map(|(path, resolution)| (path.as_slice(), resolution))
    }

    #[must_use]
    pub fn constructor_resolution(&self, expression: HirExprId) -> Option<ConstructorResolution> {
        match self.resolution(expression) {
            Some(BindingResolution::Declaration(declaration)) => {
                Some(ConstructorResolution::Declaration(*declaration))
            }
            Some(
                BindingResolution::Local(_)
                | BindingResolution::Import(_)
                | BindingResolution::QualifiedPath(_),
            ) => None,
            None => self
                .pending_constructor_paths
                .get(&expression)
                .cloned()
                .map(ConstructorResolution::Dynamic),
        }
    }

    #[must_use]
    pub fn pattern_constructor_resolution(&self, path: &[String]) -> Option<ConstructorResolution> {
        match self.pattern_resolution(path) {
            Some(BindingResolution::Declaration(declaration)) => {
                Some(ConstructorResolution::Declaration(*declaration))
            }
            Some(
                BindingResolution::Local(_)
                | BindingResolution::Import(_)
                | BindingResolution::QualifiedPath(_),
            ) => None,
            None => self
                .pending_pattern_paths
                .get(path)
                .cloned()
                .map(ConstructorResolution::Dynamic),
        }
    }

    pub(crate) fn resolve_import_declarations(&mut self, imports: &BTreeMap<String, HirDeclId>) {
        for resolution in self.resolutions.values_mut() {
            if let BindingResolution::Import(name) = resolution
                && let Some(declaration) = imports.get(name).copied()
            {
                *resolution = BindingResolution::Declaration(declaration);
            }
        }
        for resolution in self.pattern_resolutions.values_mut() {
            if let BindingResolution::Import(name) = resolution
                && let Some(declaration) = imports.get(name).copied()
            {
                *resolution = BindingResolution::Declaration(declaration);
            }
        }
    }

    pub(crate) fn resolve_qualified_declarations(
        &mut self,
        declarations: &BTreeMap<Vec<String>, HirDeclId>,
    ) {
        for resolution in self.resolutions.values_mut() {
            if let BindingResolution::QualifiedPath(path) = resolution
                && let Some(declaration) = declarations.get(path).copied()
            {
                *resolution = BindingResolution::Declaration(declaration);
            }
        }
        for resolution in self.pattern_resolutions.values_mut() {
            if let BindingResolution::QualifiedPath(path) = resolution
                && let Some(declaration) = declarations.get(path).copied()
            {
                *resolution = BindingResolution::Declaration(declaration);
            }
        }
        let resolved = self
            .pending_constructor_paths
            .iter()
            .filter_map(|(expression, path)| {
                constructor_declaration(path, declarations)
                    .map(|declaration| (*expression, declaration))
            })
            .collect::<Vec<_>>();
        for (expression, declaration) in resolved {
            self.pending_constructor_paths.remove(&expression);
            self.resolutions
                .insert(expression, BindingResolution::Declaration(declaration));
        }
        let resolved = self
            .pending_pattern_paths
            .keys()
            .filter_map(|path| {
                constructor_declaration(path, declarations)
                    .map(|declaration| (path.clone(), declaration))
            })
            .collect::<Vec<_>>();
        for (path, declaration) in resolved {
            self.pending_pattern_paths.remove(&path);
            self.pattern_resolutions
                .insert(path, BindingResolution::Declaration(declaration));
        }
    }
}

fn constructor_declaration(
    path: &[String],
    declarations: &BTreeMap<Vec<String>, HirDeclId>,
) -> Option<HirDeclId> {
    declarations.get(path).copied().or_else(|| {
        let (_, owner) = path.split_last()?;
        declarations.get(owner).copied()
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PathUsage {
    Value,
    Callee,
    FieldBase,
    CalleeFieldBase(u8),
    AssignmentTarget,
}
