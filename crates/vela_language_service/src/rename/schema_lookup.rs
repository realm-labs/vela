use vela_hir::body::HirPathKind;

use crate::{
    LanguageServiceDatabases, LineIndex, QueryContext, hir_path_sites,
    schema_function_sites::{resolve_names, scoped_path},
};

pub(super) fn changes_lookup(db: &LanguageServiceDatabases, target: &str, new_name: &str) -> bool {
    if target.rsplit("::").next() == Some(new_name) {
        return false;
    }
    let renamed = target.rsplit_once("::").map_or_else(
        || new_name.to_owned(),
        |(prefix, _)| format!("{prefix}::{new_name}"),
    );
    let before: Vec<_> = db
        .schema_db()
        .facts()
        .functions()
        .map(|function| function.name)
        .collect();
    let after: Vec<_> = before
        .iter()
        .map(|name| {
            if name == target {
                renamed.clone()
            } else {
                name.clone()
            }
        })
        .collect();
    let graph = db.hir_db().graph();
    for source in db.source_db().records().values() {
        let lines = LineIndex::new(source.text());
        for path in graph
            .paths_in_source(source.source_id())
            .filter(|path| matches!(path.kind, HirPathKind::Value | HirPathKind::Callee))
        {
            let Some(site) = hir_path_sites::site(path) else {
                continue;
            };
            let Some(query) = QueryContext::from_databases(
                db,
                source.document_id(),
                lines.position(site.segment_range.start),
            ) else {
                continue;
            };
            let Some(mut expanded) = scoped_path(db, &query, site.path, site.segment_range) else {
                continue;
            };
            let original = resolve_names(&before, &expanded.join("::"));
            let expected = if original == Some(target) {
                *expanded.last_mut().expect("resolved path") = new_name.to_owned();
                Some(renamed.as_str())
            } else {
                original
            };
            if resolve_names(&after, &expanded.join("::")) != expected {
                return true;
            }
        }
    }
    // Imports require an exact registered path, including imports with no uses.
    graph
        .module_ids()
        .filter_map(|module| graph.imports(module))
        .flatten()
        .filter(|import| import.resolution.is_none())
        .any(|import| {
            let path = import.path.join("::");
            let original = before
                .iter()
                .find(|name| **name == path)
                .map(String::as_str);
            let (path, expected) = if original == Some(target) {
                (renamed.as_str(), Some(renamed.as_str()))
            } else {
                (path.as_str(), original)
            };
            after
                .iter()
                .find(|name| name.as_str() == path)
                .map(String::as_str)
                != expected
        })
}
