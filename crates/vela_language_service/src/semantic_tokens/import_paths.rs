use vela_common::Span;
use vela_hir::module_graph::{ImportResolution, ModuleGraph};

use crate::TextRange;

use super::{
    SemanticTokenClassification, SemanticTokenModifiers, SemanticTokenType,
    declaration_use_classification, span_contains_range, token_range, token_text,
};

pub(super) fn classification(
    graph: &ModuleGraph,
    text: &str,
    name: &str,
    range: TextRange,
    span: Span,
) -> Option<SemanticTokenClassification> {
    for module in graph.module_ids() {
        let Some(imports) = graph.imports(module) else {
            continue;
        };
        for import in imports {
            if import.span.source != span.source || !import.span.contains(span.start) {
                continue;
            }
            let Some(segment_index) = segment_index(text, import.span, &import.path, name, range)
            else {
                continue;
            };
            if segment_index + 1 < import.path.len() {
                return Some(SemanticTokenClassification::new(
                    SemanticTokenType::Module,
                    SemanticTokenModifiers::NONE,
                ));
            }
            let Some(ImportResolution::Declaration(declaration)) = import.resolution else {
                return Some(SemanticTokenClassification::new(
                    SemanticTokenType::Variable,
                    SemanticTokenModifiers::UNRESOLVED,
                ));
            };
            return graph
                .declaration(declaration)
                .map(declaration_use_classification);
        }
    }
    None
}

fn segment_index(
    text: &str,
    span: Span,
    path: &[String],
    name: &str,
    range: TextRange,
) -> Option<usize> {
    (token_text(text, range)? == name).then_some(())?;
    let import_range = token_range(span)?;
    if !span_contains_range(span, range) {
        return None;
    }
    let import_text = text.get(import_range.start..import_range.end)?;
    let mut search_start = 0;
    for (index, segment) in path.iter().enumerate() {
        let relative = import_text.get(search_start..)?.find(segment)? + search_start;
        let start = import_range.start + relative;
        let end = start + segment.len();
        if start == range.start && end == range.end {
            return Some(index);
        }
        search_start = relative + segment.len();
    }
    None
}
