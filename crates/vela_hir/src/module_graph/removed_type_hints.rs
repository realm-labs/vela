use vela_common::{Diagnostic, Span};

use crate::ids::ModuleId;
use crate::type_hint::is_removed_host_type_hint_name;

use super::{DeclarationKind, ModuleGraph};

pub(super) fn diagnostics(graph: &ModuleGraph) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for module in &graph.modules {
        for hint in graph
            .source_type_hints
            .get(&module.id)
            .into_iter()
            .flatten()
        {
            if let Some(path) = unresolved_removed_path(graph, module.id, &hint.path) {
                diagnostics.push(removed_hint(&path, hint.span));
            }
        }
        for import in &module.imports {
            if let Some(path) = unresolved_removed_path(graph, module.id, &import.path) {
                diagnostics.push(removed_hint(&path, import.span));
            }
        }
    }
    diagnostics
}

fn unresolved_removed_path(
    graph: &ModuleGraph,
    module: ModuleId,
    path: &[String],
) -> Option<Vec<String>> {
    let mut path = path.to_vec();
    // Resolve declarations before checking legacy spellings, including imported
    // aliases and module aliases. Bound expansion for malformed/cyclic imports.
    for _ in 0..=graph.imports(module).map_or(0, <[_]>::len) {
        if [
            DeclarationKind::Struct,
            DeclarationKind::Enum,
            DeclarationKind::Trait,
        ]
        .into_iter()
        .any(|kind| {
            graph
                .resolve_visible_declaration_path(module, &path, kind)
                .is_some()
        }) {
            return None;
        }
        let expanded = graph.expand_import_path(module, &path)?;
        if expanded == path {
            break;
        }
        path = expanded;
    }
    path.last()
        .is_some_and(|name| is_removed_host_type_hint_name(name))
        .then_some(path)
}

fn removed_hint(path: &[String], span: Span) -> Diagnostic {
    Diagnostic::error(format!(
        "borrowed-host type hint `{}` was removed; use script-owned values",
        path.join("::")
    ))
    .with_code("hir::removed_host_type_hint")
    .with_span(span)
}
