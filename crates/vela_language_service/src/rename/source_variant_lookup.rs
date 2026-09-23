use vela_hir::{
    body::HirPathKind,
    ids::{HirDeclId, ModuleId},
    module_graph::{DeclarationKind, ModuleGraph, Visibility},
};

use crate::{LanguageServiceDatabases, LineIndex, QueryContext, hir_path_sites};

type VariantName = (HirDeclId, String);

pub(super) fn changes_lookup(
    db: &LanguageServiceDatabases,
    owner: HirDeclId,
    variant: &str,
    new_name: &str,
) -> bool {
    if variant == new_name {
        return false;
    }
    let graph = db.hir_db().graph();
    let target = (owner, variant.to_owned());
    let renamed = (owner, new_name.to_owned());
    let before: Vec<_> = graph
        .declarations()
        .filter(|declaration| declaration.kind == DeclarationKind::Enum)
        .flat_map(|declaration| {
            graph
                .enum_shape(declaration.id)
                .into_iter()
                .flat_map(move |shape| {
                    shape
                        .variants
                        .iter()
                        .map(move |variant| (declaration.id, variant.name.clone()))
                })
        })
        .collect();
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
            let Some(module) = query.module_key().and_then(|key| graph.module_id(key)) else {
                continue;
            };
            let original = query
                .expand_import_path(site.path)
                .as_deref()
                .and_then(|path| resolve(graph, module, &before, path));
            let mut edited_path = site.path.to_vec();
            let expected = if original == Some(&target) {
                let retained_alias = site.path.len() == 1
                    && graph.imports(module).is_some_and(|imports| {
                        imports.iter().any(|import| {
                            import.alias.as_ref() == site.path.first()
                                && resolve(graph, module, &before, &import.path) == Some(&target)
                        })
                    });
                if !retained_alias {
                    *edited_path.last_mut().expect("resolved variant path") = new_name.to_owned();
                }
                Some(&renamed)
            } else {
                original
            };
            let actual = expand_after(
                graph,
                module,
                &query,
                &edited_path,
                &before,
                &target,
                new_name,
            )
            .as_deref()
            .and_then(|path| resolve(graph, module, &after, path));
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
            let mut path = import.path.clone();
            let original = resolve(graph, import.module, &before, &path);
            let expected = if original == Some(&target) {
                *path.last_mut().expect("resolved variant import") = new_name.to_owned();
                Some(&renamed)
            } else {
                original
            };
            resolve(graph, import.module, &after, &path) != expected
        })
}

fn expand_after(
    graph: &ModuleGraph,
    module: ModuleId,
    query: &QueryContext<'_>,
    path: &[String],
    before: &[VariantName],
    target: &VariantName,
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
        let target_import = resolve(graph, module, before, &import.path) == Some(target);
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
    if resolve(graph, module, before, &import.path) == Some(target) {
        *expanded.last_mut().expect("target variant import") = new_name.to_owned();
    }
    expanded.extend_from_slice(&path[1..]);
    Some(expanded)
}

fn resolve<'a>(
    graph: &ModuleGraph,
    module: ModuleId,
    names: &'a [VariantName],
    path: &[String],
) -> Option<&'a VariantName> {
    let (name, parent) = path.split_last()?;
    let owner = graph.resolve_visible_declaration_path(module, parent, DeclarationKind::Enum)?;
    if owner.module != module && owner.visibility != Visibility::Public {
        return None;
    }
    names
        .iter()
        .find(|(id, variant)| *id == owner.id && variant == name)
}
