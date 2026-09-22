use vela_hir::body::HirPathKind;

use crate::{
    LanguageServiceDatabases, LineIndex, QueryContext, hir_path_sites,
    schema_variant_sites::{names, resolve_names, scoped_path},
};

pub(super) fn changes_lookup(
    db: &LanguageServiceDatabases,
    owner: &str,
    variant: &str,
    new_name: &str,
) -> bool {
    if variant == new_name {
        return false;
    }
    let target = (owner.to_owned(), variant.to_owned());
    let renamed = (owner.to_owned(), new_name.to_owned());
    let before = names(db);
    let after: Vec<_> = before
        .iter()
        .map(|name| {
            if name == &target {
                renamed.clone()
            } else {
                name.clone()
            }
        })
        .collect();
    let graph = db.hir_db().graph();
    let mut target_path: Vec<_> = owner.split("::").map(str::to_owned).collect();
    target_path.push(variant.to_owned());
    for source in db.source_db().records().values() {
        let lines = LineIndex::new(source.text());
        for path in graph.paths_in_source(source.source_id()).filter(|path| {
            hir_path_sites::is_expression_path(path.kind) || path.kind == HirPathKind::Pattern
        }) {
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
            let original = resolve_names(&before, &expanded);
            let expected = if original == Some(&target) {
                *expanded.last_mut().expect("resolved variant path") = new_name.to_owned();
                Some(&renamed)
            } else {
                // Renaming an unaliased import introduces a new local binding.
                // Original local/source bindings have already been excluded.
                if site.path.first().is_some_and(|name| name == new_name)
                    && query
                        .module_key()
                        .and_then(|key| graph.module_id(key))
                        .and_then(|module| graph.imports(module))
                        .is_some_and(|imports| {
                            imports.iter().any(|import| {
                                import.resolution.is_none()
                                    && import.alias.is_none()
                                    && import.path == target_path
                            })
                        })
                {
                    expanded = target_path.clone();
                    *expanded.last_mut().expect("imported variant path") = new_name.to_owned();
                    expanded.extend_from_slice(&site.path[1..]);
                }
                original
            };
            if resolve_names(&after, &expanded) != expected {
                return true;
            }
        }
    }
    graph
        .module_ids()
        .filter_map(|module| graph.imports(module))
        .flatten()
        .filter(|import| import.resolution.is_none())
        .any(|import| {
            let Some((name, parent)) = import.path.split_last() else {
                return false;
            };
            let identity = (parent.join("::"), name.clone());
            let original = before.iter().find(|candidate| **candidate == identity);
            let (identity, expected) = if original == Some(&target) {
                (&renamed, Some(&renamed))
            } else {
                (&identity, original)
            };
            after.iter().find(|candidate| *candidate == identity) != expected
        })
}
