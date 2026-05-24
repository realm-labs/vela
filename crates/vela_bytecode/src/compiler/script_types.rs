use std::collections::HashMap;

use vela_common::Span;
use vela_hir::{BindingMap, BindingResolution, HirLocalId};
use vela_syntax::{Expr, ExprKind};

use super::patterns::enum_variant_path;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct ScriptTypeFlow {
    locals: HashMap<HirLocalId, String>,
    names: HashMap<String, String>,
}

impl ScriptTypeFlow {
    pub(super) fn local_at_span(&self, bindings: &BindingMap, span: Span) -> Option<String> {
        let BindingResolution::Local(local) = bindings.resolution_at_span(span)? else {
            return None;
        };
        self.locals.get(local).cloned()
    }

    pub(super) fn name(&self, name: &str) -> Option<String> {
        self.names.get(name).cloned()
    }

    pub(super) fn set_name(&mut self, name: impl Into<String>, type_name: Option<String>) {
        match type_name {
            Some(type_name) => {
                self.names.insert(name.into(), type_name);
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
        let name = name.into();
        match type_name {
            Some(type_name) => {
                self.locals.insert(local, type_name.clone());
                self.names.insert(name, type_name);
            }
            None => {
                self.locals.remove(&local);
                self.names.remove(&name);
            }
        }
    }

    pub(super) fn remove_local(&mut self, local: HirLocalId, name: &str) {
        self.locals.remove(&local);
        self.names.remove(name);
    }
}

pub(super) fn expression_script_type(
    expr: &Expr,
    type_symbol_at_span: impl Fn(Span) -> Option<String>,
    local_type_at_span: impl Fn(Span) -> Option<String>,
    local_type_named: impl Fn(&str) -> Option<String>,
) -> Option<String> {
    match &expr.kind {
        ExprKind::Record { path, .. } => {
            if let Some(type_name) = type_symbol_at_span(expr.span) {
                return Some(type_name);
            }
            if let Some((enum_path, _)) = enum_variant_path(path) {
                return Some(enum_path);
            }
            Some(path.join("."))
        }
        ExprKind::Path(path) => local_type_at_span(expr.span).or_else(|| {
            path.as_slice()
                .first()
                .and_then(|name| (path.len() == 1).then(|| local_type_named(name)).flatten())
        }),
        ExprKind::SelfValue => local_type_at_span(expr.span).or_else(|| local_type_named("self")),
        _ => None,
    }
}
