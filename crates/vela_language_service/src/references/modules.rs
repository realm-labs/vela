use vela_common::SourceId;
use vela_hir::module_graph::ModuleGraph;

use crate::{LanguageServiceDatabases, symbol_ref::source_module_symbol};

use super::{Reference, ReferenceKind, ReferenceToken, diagnostic_range, span_text_range};

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct ImportModuleTarget {
    path: Vec<String>,
    key: vela_package::ModuleKey,
}

pub(super) fn import_module_target(
    graph: &ModuleGraph,
    source_id: SourceId,
    token: &ReferenceToken,
) -> Option<ImportModuleTarget> {
    for module in graph.module_ids() {
        let Some(imports) = graph.imports(module) else {
            continue;
        };
        for import in imports {
            if import.span.source != source_id {
                continue;
            }
            let Some(segment_index) = import_segment_index(import, source_id, token) else {
                continue;
            };
            if segment_index + 1 >= import.path.len() {
                continue;
            }
            let path = import.path[..=segment_index].to_vec();
            if let Some(key) = graph
                .module_key(module)
                .and_then(|current| graph.resolve_module_path(current, &path))
                .filter(|key| graph.module_id(key).is_some())
            {
                return Some(ImportModuleTarget { path, key });
            }
        }
    }
    None
}

pub(super) fn import_module_references(
    databases: &LanguageServiceDatabases,
    target: &ImportModuleTarget,
) -> Vec<Reference> {
    let graph = databases.hir_db().graph();
    let mut references = Vec::new();

    for module in graph.module_ids() {
        let Some(imports) = graph.imports(module) else {
            continue;
        };
        for import in imports {
            if import.path.len() <= target.path.len()
                || !import.path.starts_with(target.path.as_slice())
            {
                continue;
            }
            let Some(current) = graph.module_key(module) else {
                continue;
            };
            let Some(key) = graph.resolve_module_path(current, &target.path) else {
                continue;
            };
            if key != target.key {
                continue;
            }
            let Some(source) = databases
                .source_db()
                .records()
                .values()
                .find(|record| record.source_id() == import.span.source)
            else {
                continue;
            };
            let Some(range) = import
                .path_spans
                .get(target.path.len() - 1)
                .and_then(|span| span_text_range(*span))
            else {
                continue;
            };
            references.push(Reference {
                document_id: source.document_id().clone(),
                range: diagnostic_range(source.text(), range),
                kind: ReferenceKind::Import,
                symbol: source_module_symbol(&target.key),
            });
        }
    }

    references.sort_by_key(|reference| {
        let start = reference.range.start();
        (
            reference.document_id.as_str().to_owned(),
            start.line,
            start.character,
        )
    });
    references
}

fn import_segment_index(
    import: &vela_hir::module_graph::Import,
    source_id: SourceId,
    token: &ReferenceToken,
) -> Option<usize> {
    import.path_spans.iter().position(|span| {
        span.source == source_id
            && span_text_range(*span).is_some_and(|range| {
                range.start <= token.range.start && token.range.end <= range.end
            })
    })
}
