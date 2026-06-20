use std::collections::BTreeMap;

use vela_common::Span;

use crate::{
    ids::{HirDeclId, HirExprId, HirLocalId},
    type_hint::HirTypeHint,
};

mod name_candidates;
mod syntax_binding;

pub(crate) use syntax_binding::{SyntaxFunctionBindingInput, bind_syntax_function};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalBinding {
    pub id: HirLocalId,
    pub name: String,
    pub kind: LocalBindingKind,
    pub type_hint: Option<HirTypeHint>,
    pub span: Span,
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
pub struct ExprInfo {
    pub id: HirExprId,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BindingResolution {
    Local(HirLocalId),
    Declaration(HirDeclId),
    Import(String),
    QualifiedPath(Vec<String>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ImportBinding {
    pub name: String,
    pub declaration: Option<HirDeclId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BindingMap {
    pub declaration: HirDeclId,
    pub(crate) locals: BTreeMap<HirLocalId, LocalBinding>,
    pub(crate) locals_by_name: BTreeMap<String, Vec<HirLocalId>>,
    pub(crate) expressions: BTreeMap<HirExprId, ExprInfo>,
    pub(crate) resolutions: BTreeMap<HirExprId, BindingResolution>,
    pub(crate) pattern_resolutions: BTreeMap<Vec<String>, BindingResolution>,
}

impl BindingMap {
    #[must_use]
    pub fn local(&self, local: HirLocalId) -> Option<&LocalBinding> {
        self.locals.get(&local)
    }

    pub fn locals(&self) -> impl Iterator<Item = &LocalBinding> {
        self.locals.values()
    }

    #[must_use]
    pub fn locals_named(&self, name: &str) -> &[HirLocalId] {
        self.locals_by_name
            .get(name)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    #[must_use]
    pub fn local_named_at(
        &self,
        name: &str,
        kind: LocalBindingKind,
        span: Span,
    ) -> Option<HirLocalId> {
        self.locals_named(name).iter().copied().find(|local| {
            self.local(*local)
                .is_some_and(|binding| binding.kind == kind && binding.span == span)
        })
    }

    #[must_use]
    pub fn expression(&self, expression: HirExprId) -> Option<&ExprInfo> {
        self.expressions.get(&expression)
    }

    #[must_use]
    pub fn expression_count(&self) -> usize {
        self.expressions.len()
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
    pub fn resolution_at_span(&self, span: Span) -> Option<&BindingResolution> {
        let expression = self
            .expressions
            .iter()
            .find_map(|(id, expression)| (expression.span == span).then_some(*id))?;
        self.resolution(expression)
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
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PathUsage {
    Value,
    Callee,
    FieldBase,
    AssignmentTarget,
}
