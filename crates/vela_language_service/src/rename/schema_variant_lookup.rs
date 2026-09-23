use vela_hir::{
    binding::BindingResolution,
    body::HirPathKind,
    ids::ModuleId,
    module_graph::{DeclarationKind, ModuleGraph},
};

use crate::{
    LanguageServiceDatabases, LineIndex, QueryContext, TextRange, hir_path_sites,
    query_context::binding_resolution_for_source_range,
    schema_variant_sites::{names, resolve_names, scoped_path, source_parent_exists},
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
            let original = scoped_path(db, &query, site.path, site.segment_range)
                .as_deref()
                .and_then(|path| resolve_names(&before, path));
            let mut edited_path = site.path.to_vec();
            let expected = if original == Some(&target) {
                let retained_alias = site.path.len() == 1
                    && query
                        .module_key()
                        .and_then(|key| graph.module_id(key))
                        .and_then(|module| graph.imports(module))
                        .is_some_and(|imports| {
                            imports.iter().any(|import| {
                                import.alias.as_ref() == site.path.first()
                                    && import.resolution.is_none()
                                    && resolve_names(&before, &import.path) == Some(&target)
                            })
                        });
                if !retained_alias {
                    *edited_path.last_mut().expect("resolved variant path") = new_name.to_owned();
                }
                Some(&renamed)
            } else {
                original
            };
            let actual = scoped_after(
                db,
                &query,
                &edited_path,
                site.segment_range,
                &before,
                &target,
                new_name,
            )
            .as_deref()
            .and_then(|path| resolve_names(&after, path));
            if actual != expected {
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

fn scoped_after(
    db: &LanguageServiceDatabases,
    query: &QueryContext<'_>,
    path: &[String],
    range: TextRange,
    before: &[(String, String)],
    target: &(String, String),
    new_name: &str,
) -> Option<Vec<String>> {
    let graph = db.hir_db().graph();
    if query
        .bindings()
        .and_then(|bindings| binding_resolution_for_source_range(graph, bindings, range))
        .is_some_and(|resolution| {
            matches!(
                resolution,
                BindingResolution::Local(_) | BindingResolution::Declaration(_)
            )
        })
    {
        return None;
    }
    let module = query.module_key().and_then(|key| graph.module_id(key))?;
    let expanded = expand_after(graph, module, query, path, before, target, new_name)?;
    if [
        DeclarationKind::Function,
        DeclarationKind::Const,
        DeclarationKind::State,
        DeclarationKind::Struct,
        DeclarationKind::Enum,
        DeclarationKind::Trait,
    ]
    .into_iter()
    .any(|kind| {
        graph
            .resolve_visible_declaration_path(module, &expanded, kind)
            .is_some()
    }) || source_parent_exists(db, module, &expanded)
    {
        return None;
    }
    Some(expanded)
}

fn expand_after(
    graph: &ModuleGraph,
    module: ModuleId,
    query: &QueryContext<'_>,
    path: &[String],
    before: &[(String, String)],
    target: &(String, String),
    new_name: &str,
) -> Option<Vec<String>> {
    let first = path.first()?;
    if query.visible_scope_names().contains(first) {
        return None;
    }
    if graph.module(module)?.get(first).is_some() {
        return Some(path.to_vec());
    }
    let mut imports = graph.imports(module)?.iter().filter(|import| {
        let target_import =
            import.resolution.is_none() && resolve_names(before, &import.path) == Some(target);
        import.alias.as_deref().unwrap_or_else(|| {
            if target_import {
                new_name
            } else {
                import.path.last().map(String::as_str).unwrap_or("")
            }
        }) == first
    });
    let Some(import) = imports.next() else {
        return Some(path.to_vec());
    };
    if imports.next().is_some() {
        return None;
    }
    let mut expanded = import.path.clone();
    if import.resolution.is_none() && resolve_names(before, &import.path) == Some(target) {
        *expanded.last_mut().expect("target variant import") = new_name.to_owned();
    }
    expanded.extend_from_slice(&path[1..]);
    Some(expanded)
}
