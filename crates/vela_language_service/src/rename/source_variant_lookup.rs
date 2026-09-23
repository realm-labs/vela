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
    let target_path = graph.declaration(owner).map(|declaration| {
        let mut path = crate::symbol_ref::qualified_source_declaration_path(graph, declaration);
        path.push(variant.to_owned());
        path
    });

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
            let Some(mut expanded) = query.expand_import_path(site.path) else {
                continue;
            };
            let original = resolve(graph, module, &before, &expanded);
            let expected = if original == Some(&target) {
                *expanded.last_mut().expect("resolved variant path") = new_name.to_owned();
                Some(&renamed)
            } else {
                // A renamed direct import also changes its unaliased local binding.
                if site.path.first().is_some_and(|name| name == new_name)
                    && let Some(target_path) = &target_path
                    && graph.imports(module).is_some_and(|imports| {
                        imports
                            .iter()
                            .any(|import| import.alias.is_none() && import.path == *target_path)
                    })
                {
                    expanded = target_path.clone();
                    *expanded.last_mut().expect("imported variant path") = new_name.to_owned();
                    expanded.extend_from_slice(&site.path[1..]);
                }
                original
            };
            if resolve(graph, module, &after, &expanded) != expected {
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
