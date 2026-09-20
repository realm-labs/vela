use std::collections::{BTreeMap, BTreeSet};

use vela_hir::binding::{BindingMap, BindingResolution, LocalBindingKind};
use vela_hir::body::{HirBody, HirBodyOwner, HirExprKind, HirScope};
use vela_hir::ids::{HirLocalId, HirScopeId};
use vela_hir::module_graph::ModuleGraph;

pub(super) fn conflicts(
    graph: &ModuleGraph,
    bindings: &BindingMap,
    local: HirLocalId,
    name: &str,
) -> bool {
    let Some(target) = bindings.local(local) else {
        return true;
    };
    if target.name == name {
        return false;
    }
    let bodies = graph
        .bodies()
        .filter(|body| {
            graph
                .bindings_for_body(body.id)
                .is_some_and(|map| map.local(local).is_some())
        })
        .collect::<Vec<_>>();
    let scopes: BTreeMap<_, _> = bodies
        .iter()
        .flat_map(|body| body.scopes.values().map(|scope| (scope.id, (*body, scope))))
        .collect();
    let Some((_, target_scope)) = scopes
        .values()
        .find(|(_, scope)| scope.locals.contains(&local))
    else {
        return true;
    };
    // Preserve the existing same-scope collision contract, even when sequential
    // shadowing would happen to leave all current uses unchanged.
    if target_scope
        .locals
        .iter()
        .any(|id| *id != local && bindings.local(*id).is_some_and(|other| other.name == name))
    {
        return true;
    }
    for body in bodies {
        let unresolved_reads = body
            .unresolved_references
            .iter()
            .map(|reference| reference.expression)
            .chain(
                body.expressions
                    .values()
                    .filter_map(|expression| match &expression.kind {
                        HirExprKind::Field(field) => Some(field.receiver),
                        _ => None,
                    }),
            )
            .collect::<BTreeSet<_>>();
        for expression in body.expressions.values() {
            let HirExprKind::Path(path) = expression.kind else {
                continue;
            };
            let Some(head) = body.paths.get(&path).and_then(|path| path.path.first()) else {
                continue;
            };
            let resolution = bindings.resolution(expression.id);
            let owned = resolution == Some(&BindingResolution::Local(local));
            if !owned && head != name {
                continue;
            }
            // Bare Map keys are literal keys, not unresolved identifier reads.
            // Contextual task/service capabilities likewise bypass local lookup.
            if (resolution.is_none() && !unresolved_reads.contains(&expression.id))
                || bindings.task_capability(expression.id).is_some()
                || bindings.service_capability(expression.id).is_some()
            {
                continue;
            }
            let resolved = renamed_lookup(
                graph,
                bindings,
                &scopes,
                expression.scope,
                expression.origin.span.start,
                local,
                name,
            );
            if (owned && resolved != Some(local)) || (!owned && resolved == Some(local)) {
                return true;
            }
        }
    }
    false
}

// Follow actual HIR scope ancestry, including the scope at a closure's creation
// site. Source intervals alone cannot distinguish sibling scopes or defaults.
fn renamed_lookup(
    graph: &ModuleGraph,
    bindings: &BindingMap,
    scopes: &BTreeMap<HirScopeId, (&HirBody, &HirScope)>,
    mut scope_id: HirScopeId,
    offset: u32,
    renamed: HirLocalId,
    name: &str,
) -> Option<HirLocalId> {
    for _ in 0..=scopes.len() {
        let (body, scope) = scopes.get(&scope_id)?;
        for id in scope.locals.iter().rev() {
            let Some(local) = bindings.local(*id) else {
                continue;
            };
            let effective_name = if *id == renamed { name } else { &local.name };
            if effective_name != name {
                continue;
            }
            // Let initializers are bound before their bindings are installed.
            // For/match scopes only become active after iterable/pattern binding;
            // parameters are all installed before their defaults are lowered.
            if local.kind == LocalBindingKind::Let
                && local.scope_span.unwrap_or(local.span).end > offset
            {
                continue;
            }
            return Some(*id);
        }
        scope_id = if let Some(parent) = scope.parent {
            parent
        } else {
            match body.owner {
                HirBodyOwner::Lambda { parent, expression } => {
                    graph.body(parent)?.expressions.get(&expression)?.scope
                }
                HirBodyOwner::ParameterDefault { parent, .. } => graph.body(parent)?.root_scope,
                _ => return None,
            }
        };
    }
    None
}
