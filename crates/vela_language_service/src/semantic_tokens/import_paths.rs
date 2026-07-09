use vela_common::Span;
use vela_hir::module_graph::{Import, ImportResolution, ModuleGraph};

use crate::TextRange;

use super::{
    SemanticTokenClassification, SemanticTokenModifiers, SemanticTokenType,
    declaration_use_classification, span_contains_range,
};

pub(super) fn classification(
    graph: &ModuleGraph,
    _text: &str,
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
            let Some(segment_index) = segment_index(import, name, range) else {
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
                    SemanticTokenType::UnresolvedReference,
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

fn segment_index(import: &Import, name: &str, range: TextRange) -> Option<usize> {
    import
        .path
        .iter()
        .zip(&import.path_spans)
        .position(|(segment, span)| segment == name && span_contains_range(*span, range))
}
