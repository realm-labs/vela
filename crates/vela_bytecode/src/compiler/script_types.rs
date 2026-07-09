use std::collections::HashMap;

use vela_common::{SourceId, Span};
use vela_hir::body::{HirPathKind, HirPathOwner};
use vela_hir::ids::{HirExprId, HirLocalId};
use vela_hir::type_hint::HirTypeHint;
use vela_syntax::ast::SyntaxExpression;

use super::body_payloads::CompilerExpressionPayload;
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

fn expression_script_fact_from_payload_syntax(
    payload: &CompilerExpressionPayload<'_>,
    type_symbol_at_span: &impl Fn(Span) -> Option<String>,
    local_fact_at_span: &impl Fn(Span) -> Option<ScriptTypeFact>,
    local_fact_named: &impl Fn(&str) -> Option<ScriptTypeFact>,
    hir_call_fact_at_span: &impl Fn(Span) -> Option<ScriptTypeFact>,
) -> Option<ScriptTypeFact> {
    if let Some(path) = payload.syntax_record_path_segments() {
        if let Some((enum_path, variant)) = enum_variant_path(&path) {
            let type_name = payload
                .syntax_span()
                .and_then(type_symbol_at_span)
                .unwrap_or(enum_path);
            return Some(ScriptTypeFact::enum_variant(type_name, variant));
        }
        let type_name = payload
            .syntax_span()
            .and_then(type_symbol_at_span)
            .unwrap_or_else(|| path.join("::"));
        return Some(ScriptTypeFact::new(type_name));
    }

    if let Some(fact) = payload.syntax_span().and_then(hir_call_fact_at_span) {
        return Some(fact);
    }

    if payload.syntax_is_self() {
        return payload
            .syntax_span()
            .and_then(local_fact_at_span)
            .or_else(|| local_fact_named("self"));
    }

    if let Some(path) = payload.syntax_path_segments() {
        if let Some(fact) = path
            .first()
            .and_then(|name| (path.len() == 1).then(|| local_fact_named(name)).flatten())
        {
            return Some(fact);
        }
        return payload.syntax_span().and_then(local_fact_at_span);
    }

    None
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
    fn type_symbol_for_expression(&self, expression: HirExprId) -> Option<String> {
        let Some(vela_hir::binding::BindingResolution::Declaration(declaration)) =
            self.bindings.resolution(expression)
        else {
            return None;
        };
        self.facts.type_symbols.get(declaration).cloned()
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

    pub(super) fn script_fact_for_syntax_expression(
        &self,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> Option<ScriptTypeFact> {
        let payload =
            CompilerExpressionPayload::from_syntax(Some(source), Some(expression.clone()));
        expression_script_fact_from_payload_syntax(
            &payload,
            &|span| self.type_symbol_at_span(span),
            &|span| {
                self.local_at_span(span)
                    .and_then(|local| self.script_types.local_fact(local))
                    .or_else(|| self.global_type_at_span(span).map(ScriptTypeFact::new))
            },
            &|name| {
                self.script_types
                    .name_fact(name)
                    .or_else(|| self.global_type_named(name).map(ScriptTypeFact::new))
            },
            &|span| {
                let call = self.expression_at_span(span)?;
                self.script_fact_for_hir_call(call)
            },
        )
    }
}
