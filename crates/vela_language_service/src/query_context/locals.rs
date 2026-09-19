use std::collections::BTreeMap;
use vela_hir::binding::{LocalBinding, LocalBindingKind};
use vela_hir::body::{HirBody, HirScope, HirStmtKind};

use super::QueryContext;

impl QueryContext<'_> {
    pub(super) fn visible_locals(&self) -> Vec<&LocalBinding> {
        let (Some(graph), Some(body), Some(bindings)) = (self.graph, self.body, self.bindings)
        else {
            return Vec::new();
        };
        let Ok(offset) = u32::try_from(self.cursor.replace_range().end) else {
            return Vec::new();
        };
        // The callee anchors incomplete calls whose recovery span ends at EOF.
        let probe = if body.origin.span.contains(offset) {
            offset
        } else {
            self.cursor
                .call_callee()
                .and_then(|range| u32::try_from(range.start).ok())
                .unwrap_or(offset)
        };
        let mut visible: BTreeMap<&str, &LocalBinding> = BTreeMap::new();
        let mut parameter_owner = None;
        for owner in graph.body_and_ancestors(body.id) {
            let default_parent = parameter_owner == Some(owner.id);
            if let vela_hir::body::HirBodyOwner::ParameterDefault { parent, .. } = owner.owner {
                parameter_owner = Some(parent);
            }
            for scope in owner.scopes.values().filter(|scope| {
                scope.origin.span.contains(probe)
                    || (default_parent && scope.id == owner.root_scope)
            }) {
                for id in &scope.locals {
                    let Some(local) = bindings.local(*id) else {
                        continue;
                    };
                    if !visible_from(owner, scope, local).is_some_and(|start| start <= offset) {
                        continue;
                    }
                    let entry = visible.entry(local.name.as_str()).or_insert(local);
                    if local.span.start > entry.span.start {
                        *entry = local;
                    }
                }
            }
        }
        visible.into_values().collect()
    }
}

fn visible_from(body: &HirBody, scope: &HirScope, local: &LocalBinding) -> Option<u32> {
    match local.kind {
        LocalBindingKind::For => body
            .statements
            .values()
            .find(|statement| Some(statement.origin.span) == local.scope_span)
            .and_then(|statement| match statement.kind {
                HirStmtKind::For {
                    body: Some(block), ..
                } => body.blocks.get(&block),
                _ => None,
            })
            .map(|block| block.origin.span.start),
        LocalBindingKind::Pattern => body
            .match_arms
            .values()
            .find(|arm| arm.scope == scope.id)
            .and_then(|arm| arm.pattern)
            .and_then(|pattern| body.patterns.get(&pattern))
            .and_then(|pattern| pattern.origin.span.end.checked_add(1)),
        LocalBindingKind::Let => Some(local.scope_span.unwrap_or(local.span).end),
        LocalBindingKind::Parameter | LocalBindingKind::LambdaParameter => Some(local.span.end),
    }
}
