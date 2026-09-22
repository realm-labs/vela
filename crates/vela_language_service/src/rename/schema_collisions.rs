use vela_hir::ids::ModuleId;

use crate::{
    LanguageServiceDatabases, LineIndex, QueryContext, ReferenceKind, schema_function_sites,
};

pub(super) fn function_name_is_captured(
    db: &LanguageServiceDatabases,
    name: &str,
    new_name: &str,
) -> bool {
    if name.rsplit("::").next() == Some(new_name) {
        return false;
    }
    let graph = db.hir_db().graph();
    if let Some(span) = db.schema_db().source_locations().function_span(name)
        && let Some(declaration) = graph
            .declarations()
            .find(|declaration| declaration.name_span == span)
        && module_name_exists(db, declaration.module, new_name)
    {
        return true;
    }
    db.source_db().records().values().any(|source| {
        schema_function_sites::sites(db, source)
            .into_iter()
            .filter(|site| site.name == name && site.edit_range.is_some())
            .any(|site| {
                if site.kind == ReferenceKind::Import {
                    // Explicit aliases keep their binding name when the path changes.
                    return site.edit_range == Some(site.range)
                        && graph.module_ids().any(|module| {
                            graph.imports(module).is_some_and(|imports| {
                                imports.iter().any(|import| {
                                    import.span.source == source.source_id()
                                        && import
                                            .path_spans
                                            .last()
                                            .copied()
                                            .and_then(super::span_text_range)
                                            == Some(site.range)
                                })
                            }) && module_name_exists(db, module, new_name)
                        });
                }
                if !graph.paths_in_source(source.source_id()).any(|path| {
                    path.path.len() == 1
                        && crate::hir_path_sites::site(path)
                            .is_some_and(|path| path.segment_range == site.range)
                }) {
                    return false;
                }
                let Some(query) = QueryContext::from_databases(
                    db,
                    source.document_id(),
                    LineIndex::new(source.text()).position(site.range.start),
                ) else {
                    return false;
                };
                super::collisions::local_name_captures(graph, &query, new_name)
                    || query
                        .module_key()
                        .and_then(|key| graph.module_id(key))
                        .is_some_and(|module| module_name_exists(db, module, new_name))
            })
    })
}

fn module_name_exists(db: &LanguageServiceDatabases, module: ModuleId, name: &str) -> bool {
    let graph = db.hir_db().graph();
    graph
        .module(module)
        .is_some_and(|declarations| declarations.get(name).is_some())
        || graph.imports(module).is_some_and(|imports| {
            imports.iter().any(|import| {
                import
                    .alias
                    .as_ref()
                    .or_else(|| import.path.last())
                    .is_some_and(|binding| binding == name)
            })
        })
}
