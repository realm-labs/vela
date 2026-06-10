use std::collections::HashMap;

use vela_common::Span;
use vela_hir::binding::{BindingMap, BindingResolution};
use vela_hir::ids::HirLocalId;
use vela_hir::type_hint::HirTypeHint;
use vela_syntax::ast::{BinaryOp, Expr, ExprKind, Literal};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct ValueTypeFlow {
    locals: HashMap<HirLocalId, String>,
    names: HashMap<String, String>,
}

impl ValueTypeFlow {
    pub(super) fn local_at_span(&self, bindings: &BindingMap, span: Span) -> Option<String> {
        let BindingResolution::Local(local) = bindings.resolution_at_span(span)? else {
            return None;
        };
        self.local(*local)
    }

    pub(super) fn local(&self, local: HirLocalId) -> Option<String> {
        self.locals.get(&local).cloned()
    }

    pub(super) fn name(&self, name: &str) -> Option<String> {
        self.names.get(name).cloned()
    }

    pub(super) fn set_name(&mut self, name: impl Into<String>, type_name: Option<String>) {
        let name = name.into();
        match type_name {
            Some(type_name) => {
                self.names.insert(name, type_name);
            }
            None => {
                self.names.remove(&name);
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
}

pub(super) fn expression_value_type(
    expr: &Expr,
    local_type_at_span: impl Fn(Span) -> Option<String>,
    local_type_named: impl Fn(&str) -> Option<String>,
) -> Option<String> {
    match &expr.kind {
        ExprKind::Literal(Literal::Null) => Some("null".to_owned()),
        ExprKind::Literal(Literal::Bool(_)) => Some("bool".to_owned()),
        ExprKind::Literal(Literal::Int(_)) => Some("int".to_owned()),
        ExprKind::Literal(Literal::Float(_)) => Some("float".to_owned()),
        ExprKind::Literal(Literal::String(_)) => Some("string".to_owned()),
        ExprKind::Array(_) => Some("array".to_owned()),
        ExprKind::Map(_) => Some("map".to_owned()),
        ExprKind::Lambda { .. } => Some("closure".to_owned()),
        ExprKind::Binary {
            op: BinaryOp::Range,
            ..
        } => Some("range".to_owned()),
        ExprKind::Path(path) => local_type_at_span(expr.span).or_else(|| {
            path.as_slice()
                .first()
                .and_then(|name| (path.len() == 1).then(|| local_type_named(name)).flatten())
        }),
        ExprKind::SelfValue => local_type_at_span(expr.span).or_else(|| local_type_named("self")),
        _ => None,
    }
}

pub(super) fn type_hint_value_type(hint: &HirTypeHint) -> Option<String> {
    match hint.display().as_str() {
        "null" | "bool" | "int" | "float" | "string" | "array" | "map" | "set" | "range"
        | "function" | "closure" | "Option" | "Result" => Some(hint.display()),
        _ => None,
    }
}

impl super::Compiler<'_, '_> {
    pub(super) fn value_type_for_expr(&self, expr: &Expr) -> Option<String> {
        expression_value_type(
            expr,
            |span| self.value_types.local_at_span(self.bindings, span),
            |name| self.value_types.name(name),
        )
        .or_else(|| self.record_field_value_type_for_expr(expr))
    }
}
