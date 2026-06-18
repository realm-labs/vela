use vela_common::SourceId;
use vela_hir::module_graph::{ModuleGraph, ModulePath};

use crate::{LanguageServiceDatabases, SymbolRef, TextRange};

use super::{
    Reference, ReferenceKind, ReferenceToken, diagnostic_range, span_text_range, token_text,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct ImportModuleTarget {
    path: Vec<String>,
}

pub(super) fn import_module_target(
    graph: &ModuleGraph,
    source_id: SourceId,
    text: &str,
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
            let Some(segment_index) = import_segment_index(text, import.span, &import.path, token)
            else {
                continue;
            };
            if segment_index + 1 >= import.path.len() {
                continue;
            }
            let path = import.path[..=segment_index].to_vec();
            if graph
                .module_id(&ModulePath::new(path.iter().cloned()))
                .is_some()
            {
                return Some(ImportModuleTarget { path });
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
            let Some(source) = databases
                .source_db()
                .records()
                .values()
                .find(|record| record.source_id() == import.span.source)
            else {
                continue;
            };
            let Some(range) = import_segment_range(
                source.text(),
                import.span,
                &import.path,
                target.path.len() - 1,
            ) else {
                continue;
            };
            references.push(Reference {
                document_id: source.document_id().clone(),
                range: diagnostic_range(source.text(), range),
                kind: ReferenceKind::Import,
                symbol: SymbolRef::Source(target.path.join("::")),
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
    text: &str,
    span: vela_common::Span,
    path: &[String],
    token: &ReferenceToken,
) -> Option<usize> {
    let name = token_text(text, token.range)?;
    let index = path.iter().position(|segment| segment == name)?;
    let range = import_segment_range(text, span, path, index)?;
    (range.start <= token.range.start && token.range.end <= range.end).then_some(index)
}

fn import_segment_range(
    text: &str,
    span: vela_common::Span,
    path: &[String],
    segment_index: usize,
) -> Option<TextRange> {
    let import_range = span_text_range(span)?;
    let import_text = text.get(import_range.start..import_range.end)?;
    let mut search_start = 0;
    for (index, segment) in path.iter().enumerate() {
        let relative = import_text.get(search_start..)?.find(segment)? + search_start;
        let start = import_range.start + relative;
        let end = start + segment.len();
        if index == segment_index {
            return Some(TextRange::new(start, end));
        }
        search_start = relative + segment.len();
    }
    None
}
