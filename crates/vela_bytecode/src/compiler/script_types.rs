use std::collections::HashMap;

use vela_common::Span;
use vela_hir::binding::{BindingMap, BindingResolution};
use vela_hir::ids::HirLocalId;
use vela_hir::type_hint::HirTypeHint;
use vela_syntax::ast::{Expr, ExprKind};

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
    pub(super) fn local_at_span(&self, bindings: &BindingMap, span: Span) -> Option<String> {
        self.local_fact_at_span(bindings, span)
            .map(|fact| fact.type_name)
    }

    pub(super) fn local_fact_at_span(
        &self,
        bindings: &BindingMap,
        span: Span,
    ) -> Option<ScriptTypeFact> {
        let BindingResolution::Local(local) = bindings.resolution_at_span(span)? else {
            return None;
        };
        self.local_fact(*local)
    }

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

pub(super) fn expression_script_fact_with_payload(
    expr: &Expr,
    payload: Option<&CompilerExpressionPayload<'_>>,
    type_symbol_at_span: impl Fn(Span) -> Option<String>,
    local_fact_at_span: impl Fn(Span) -> Option<ScriptTypeFact>,
    local_fact_named: impl Fn(&str) -> Option<ScriptTypeFact>,
) -> Option<ScriptTypeFact> {
    if let Some(fact) = payload.and_then(|payload| {
        expression_script_fact_from_payload(
            payload,
            &type_symbol_at_span,
            &local_fact_at_span,
            &local_fact_named,
        )
    }) {
        return Some(fact);
    }

    match &expr.kind {
        ExprKind::Record { path, .. } => {
            let cst_path = payload.and_then(CompilerExpressionPayload::record_path_segments);
            let lookup_path = cst_path.as_deref().unwrap_or(path);
            if let Some((enum_path, variant)) = enum_variant_path(lookup_path) {
                let type_name = type_symbol_at_span(expr.span).unwrap_or(enum_path);
                return Some(ScriptTypeFact::enum_variant(type_name, variant));
            }
            let type_name =
                type_symbol_at_span(expr.span).unwrap_or_else(|| lookup_path.join("::"));
            Some(ScriptTypeFact::new(type_name))
        }
        ExprKind::Call { callee, .. } => {
            let ExprKind::Path(path) = &callee.kind else {
                return None;
            };
            let callee_payload = payload.and_then(CompilerExpressionPayload::call_callee_payload);
            let cst_path = callee_payload
                .as_ref()
                .and_then(CompilerExpressionPayload::path_segments);
            let lookup_path = cst_path.as_deref().unwrap_or(path);
            let (_, variant) = enum_variant_path(lookup_path)?;
            let type_name = type_symbol_at_span(callee.span)?;
            Some(ScriptTypeFact::enum_variant(type_name, variant))
        }
        ExprKind::Path(path) => {
            let cst_path = payload.and_then(CompilerExpressionPayload::path_segments);
            cst_path
                .as_deref()
                .and_then(|lookup_path| {
                    lookup_path.first().and_then(|name| {
                        (lookup_path.len() == 1)
                            .then(|| local_fact_named(name))
                            .flatten()
                    })
                })
                .or_else(|| local_fact_at_span(expr.span))
                .or_else(|| {
                    path.first().and_then(|name| {
                        (path.len() == 1).then(|| local_fact_named(name)).flatten()
                    })
                })
        }
        ExprKind::SelfValue => local_fact_at_span(expr.span).or_else(|| local_fact_named("self")),
        _ => None,
    }
}

fn expression_script_fact_from_payload(
    payload: &CompilerExpressionPayload<'_>,
    type_symbol_at_span: &impl Fn(Span) -> Option<String>,
    local_fact_at_span: &impl Fn(Span) -> Option<ScriptTypeFact>,
    local_fact_named: &impl Fn(&str) -> Option<ScriptTypeFact>,
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

    if let Some(path) = payload.syntax_call_callee_path_segments() {
        let (_, variant) = enum_variant_path(&path)?;
        let type_name = payload
            .syntax_call_callee_span()
            .and_then(type_symbol_at_span)?;
        return Some(ScriptTypeFact::enum_variant(type_name, variant));
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

pub(super) fn expression_script_type_with_payload(
    expr: &Expr,
    payload: Option<&CompilerExpressionPayload<'_>>,
    type_symbol_at_span: impl Fn(Span) -> Option<String>,
    local_type_at_span: impl Fn(Span) -> Option<String>,
    local_type_named: impl Fn(&str) -> Option<String>,
) -> Option<String> {
    expression_script_fact_with_payload(
        expr,
        payload,
        type_symbol_at_span,
        |span| local_type_at_span(span).map(ScriptTypeFact::new),
        |name| local_type_named(name).map(ScriptTypeFact::new),
    )
    .map(|fact| fact.type_name)
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
    pub(super) fn script_type_for_expr(&self, expr: &Expr) -> Option<String> {
        self.script_type_for_expr_with_payload(expr, None)
    }

    pub(super) fn script_type_for_expr_with_payload(
        &self,
        expr: &Expr,
        payload: Option<&CompilerExpressionPayload<'_>>,
    ) -> Option<String> {
        expression_script_type_with_payload(
            expr,
            payload,
            |span| self.type_symbol_at_span(span),
            |span| {
                self.script_types
                    .local_at_span(self.bindings, span)
                    .or_else(|| self.global_type_at_span(span))
            },
            |name| {
                self.script_types
                    .name(name)
                    .or_else(|| self.global_type_named(name))
            },
        )
    }

    pub(super) fn script_fact_for_expr_with_payload(
        &self,
        expr: &Expr,
        payload: Option<&CompilerExpressionPayload<'_>>,
    ) -> Option<ScriptTypeFact> {
        expression_script_fact_with_payload(
            expr,
            payload,
            |span| self.type_symbol_at_span(span),
            |span| {
                self.script_types
                    .local_fact_at_span(self.bindings, span)
                    .or_else(|| self.global_type_at_span(span).map(ScriptTypeFact::new))
            },
            |name| {
                self.script_types
                    .name_fact(name)
                    .or_else(|| self.global_type_named(name).map(ScriptTypeFact::new))
            },
        )
    }
}
