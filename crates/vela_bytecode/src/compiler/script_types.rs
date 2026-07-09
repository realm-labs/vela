use std::collections::HashMap;

use vela_common::{SourceId, Span};
use vela_hir::body::{HirPathKind, HirPathOwner};
use vela_hir::ids::{HirExprId, HirLocalId};
use vela_hir::type_hint::HirTypeHint;
use vela_syntax::ast::{AstNode, SyntaxExpression};

use super::patterns::enum_variant_path;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct ScriptTypeFlow {
    locals: HashMap<HirLocalId, ScriptTypeFact>,
    names: HashMap<String, ScriptTypeFact>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ScriptTypeFact {
    pub(super) type_name: String,
    pub(super) enum_variant: Option<String>,
}

impl ScriptTypeFact {
    pub(super) fn new(type_name: impl Into<String>) -> Self {
        Self {
            type_name: type_name.into(),
            enum_variant: None,
        }
    }

    pub(super) fn enum_variant(type_name: impl Into<String>, variant: impl Into<String>) -> Self {
        Self {
            type_name: type_name.into(),
            enum_variant: Some(variant.into()),
        }
    }
}

impl ScriptTypeFlow {
    pub(super) fn local(&self, local: HirLocalId) -> Option<String> {
        self.local_fact(local).map(|fact| fact.type_name)
    }

    pub(super) fn local_fact(&self, local: HirLocalId) -> Option<ScriptTypeFact> {
        self.locals.get(&local).cloned()
    }

    pub(super) fn name(&self, name: &str) -> Option<String> {
        self.name_fact(name).map(|fact| fact.type_name)
    }

    pub(super) fn name_fact(&self, name: &str) -> Option<ScriptTypeFact> {
        self.names.get(name).cloned()
    }

    pub(super) fn set_name(&mut self, name: impl Into<String>, type_name: Option<String>) {
        self.set_name_fact(name, type_name.map(ScriptTypeFact::new));
    }

    pub(super) fn set_name_fact(&mut self, name: impl Into<String>, fact: Option<ScriptTypeFact>) {
        match fact {
            Some(fact) => {
                self.names.insert(name.into(), fact);
            }
            None => {
                self.names.remove(&name.into());
            }
        }
    }

    pub(super) fn set_local(
        &mut self,
        local: HirLocalId,
        name: impl Into<String>,
        type_name: Option<String>,
    ) {
        self.set_local_fact(local, name, type_name.map(ScriptTypeFact::new));
    }

    pub(super) fn set_local_fact(
        &mut self,
        local: HirLocalId,
        name: impl Into<String>,
        fact: Option<ScriptTypeFact>,
    ) {
        let name = name.into();
        match fact {
            Some(fact) => {
                self.locals.insert(local, fact.clone());
                self.names.insert(name, fact);
            }
            None => {
                self.locals.remove(&local);
                self.names.remove(&name);
            }
        }
    }
}

pub(super) fn type_hint_script_type<'a>(
    hint: &HirTypeHint,
    type_names: impl IntoIterator<Item = &'a String>,
) -> Option<String> {
    let hinted = hint.display();
    let mut suffix_match = None;
    for type_name in type_names {
        if type_name == &hinted {
            return Some(type_name.clone());
        }
        if hint.path.len() == 1 && type_name.rsplit("::").next() == Some(hinted.as_str()) {
            if suffix_match.is_some() {
                return None;
            }
            suffix_match = Some(type_name.clone());
        }
    }
    suffix_match
}

impl super::Compiler<'_, '_> {
    pub(in crate::compiler) fn type_symbol_for_expression(
        &self,
        expression: HirExprId,
    ) -> Option<String> {
        let Some(vela_hir::binding::BindingResolution::Declaration(declaration)) =
            self.bindings.resolution(expression)
        else {
            return None;
        };
        self.facts.type_symbols.get(declaration).cloned()
    }

    pub(in crate::compiler) fn hir_constructor_path(
        &self,
        expression: HirExprId,
    ) -> Option<&[String]> {
        self.hir_bodies
            .iter()
            .flat_map(|body| body.paths.iter())
            .find(|path| {
                path.kind == HirPathKind::Constructor
                    && path.owner == HirPathOwner::Expression(expression)
            })
            .map(|path| path.path.as_slice())
    }

    fn hir_value_path(&self, expression: HirExprId) -> Option<&[String]> {
        self.hir_bodies
            .iter()
            .flat_map(|body| body.paths.iter())
            .find(|path| {
                path.kind == HirPathKind::Value
                    && path.owner == HirPathOwner::Expression(expression)
            })
            .map(|path| path.path.as_slice())
    }

    pub(in crate::compiler) fn script_fact_for_hir_constructor(
        &self,
        expression: HirExprId,
    ) -> Option<ScriptTypeFact> {
        let path = self.hir_constructor_path(expression)?;
        if let Some((enum_path, variant)) = enum_variant_path(path) {
            let type_name = self
                .type_symbol_for_expression(expression)
                .unwrap_or(enum_path);
            return Some(ScriptTypeFact::enum_variant(type_name, variant));
        }
        let type_name = self
            .type_symbol_for_expression(expression)
            .unwrap_or_else(|| path.join("::"));
        Some(ScriptTypeFact::new(type_name))
    }

    pub(in crate::compiler) fn script_fact_for_hir_call(
        &self,
        call: HirExprId,
    ) -> Option<ScriptTypeFact> {
        let callee = self.call_callee_expression(call)?;
        let path = self
            .hir_bodies
            .iter()
            .flat_map(|body| body.paths.iter())
            .find(|path| {
                path.kind == HirPathKind::Callee && path.owner == HirPathOwner::Expression(callee)
            })?;
        let (_, variant) = enum_variant_path(&path.path)?;
        let type_name = self.type_symbol_for_expression(callee)?;
        Some(ScriptTypeFact::enum_variant(type_name, variant))
    }

    fn script_fact_for_hir_expression(&self, expression: HirExprId) -> Option<ScriptTypeFact> {
        if let Some(fact) = self.script_fact_for_hir_constructor(expression) {
            return Some(fact);
        }
        if let Some(fact) = self.script_fact_for_hir_call(expression) {
            return Some(fact);
        }

        match self.bindings.resolution(expression) {
            Some(vela_hir::binding::BindingResolution::Local(local)) => {
                if let Some(fact) = self.script_types.local_fact(*local) {
                    return Some(fact);
                }
                if let Some(binding) = self.bindings.local(*local)
                    && let Some(fact) = self.script_types.name_fact(&binding.name)
                {
                    return Some(fact);
                }
            }
            Some(vela_hir::binding::BindingResolution::Declaration(declaration)) => {
                if let Some(type_name) = self.facts.global_type_symbols.get(declaration) {
                    return Some(ScriptTypeFact::new(type_name.clone()));
                }
            }
            _ => {}
        }

        let [name] = self.hir_value_path(expression)? else {
            return None;
        };
        self.script_types
            .name_fact(name)
            .or_else(|| self.global_type_named(name).map(ScriptTypeFact::new))
    }

    pub(super) fn script_fact_for_syntax_expression(
        &self,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> Option<ScriptTypeFact> {
        let expression = self.expression_at_span(syntax_expression_span(source, expression))?;
        self.script_fact_for_hir_expression(expression)
    }
}

fn syntax_expression_span(source: SourceId, expression: &SyntaxExpression) -> Span {
    let range = expression.syntax().text_range();
    Span::new(source, range.start().into(), range.end().into())
}
