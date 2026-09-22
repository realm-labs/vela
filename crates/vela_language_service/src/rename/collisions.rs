use super::{
    BindingResolution, Declaration, LanguageServiceDatabases, LineIndex, ModuleGraph, QueryContext,
    span_text_range, token_text,
};

// A bare use rewritten to a visible local's name would silently change owner.
// Qualified paths and retained import aliases do not undergo that lookup.
pub(super) fn declaration_use_is_captured(
    databases: &LanguageServiceDatabases,
    declaration: &Declaration,
    new_name: &str,
) -> bool {
    let graph = databases.hir_db().graph();
    let mut seen = std::collections::BTreeSet::new();
    graph.bodies().any(|body| {
        let Some(bindings) = graph.bindings_for_body(body.id) else {
            return false;
        };
        bindings.resolutions().any(|(expression, resolution)| {
            if *resolution != BindingResolution::Declaration(declaration.id)
                || !seen.insert(expression)
            {
                return false;
            }
            let Some(span) = graph.expression_span(expression) else {
                return false;
            };
            let Some(source) = databases.source_record_for_rename(span.source) else {
                return false;
            };
            let Some(range) = span_text_range(span) else {
                return false;
            };
            if token_text(source.text(), range) != Some(declaration.name.as_str()) {
                return false;
            }
            QueryContext::from_databases(
                databases,
                source.document_id(),
                LineIndex::new(source.text()).position(range.start),
            )
            .is_some_and(|query| local_name_captures(graph, &query, new_name))
        })
    })
}

pub(super) fn local_name_captures(
    graph: &ModuleGraph,
    query: &QueryContext<'_>,
    name: &str,
) -> bool {
    query
        .local_bindings_before_cursor()
        .any(|local| local.name == name)
        || default_parameter_captures(graph, query, name)
}

fn default_parameter_captures(graph: &ModuleGraph, query: &QueryContext<'_>, name: &str) -> bool {
    let Some(body) = query.body() else {
        return false;
    };
    // The canonical binder declares all parameters before lowering defaults.
    // Completion's source-order filter is insufficient for rename safety here.
    graph.body_and_ancestors(body.id).any(|body| {
        let vela_hir::body::HirBodyOwner::ParameterDefault { parent, .. } = body.owner else {
            return false;
        };
        graph.body(parent).is_some_and(|owner| {
            owner.scopes[&owner.root_scope].locals.iter().any(|id| {
                graph.local_binding(*id).is_some_and(|local| {
                    local.kind == vela_hir::binding::LocalBindingKind::Parameter
                        && local.name == name
                })
            })
        })
    })
}
