use std::collections::BTreeSet;

use vela_common::{SourceId, Span};
use vela_hir::{
    binding::{BindingMap, BindingResolution},
    module_graph::ModuleGraph,
};

use crate::TextRange;

use super::{SemanticTokenClassification, SemanticTokenModifiers, SemanticTokenType, token_range};

pub(super) type IdentifierRanges = BTreeSet<(usize, usize)>;

pub(super) fn classification(
    graph: &ModuleGraph,
    bindings: &BindingMap,
    span: Span,
    unresolved_identifiers: &IdentifierRanges,
    range: TextRange,
) -> Option<SemanticTokenClassification> {
    let resolution = graph
        .expression_containing_span(span)
        .and_then(|expression| bindings.resolution(expression));
    if unresolved_identifiers.contains(&(range.start, range.end))
        || matches!(
            resolution,
            Some(BindingResolution::Import(_) | BindingResolution::QualifiedPath(_))
        )
    {
        return Some(SemanticTokenClassification::new(
            SemanticTokenType::UnresolvedReference,
            SemanticTokenModifiers::UNRESOLVED,
        ));
    }
    None
}

pub(super) fn ranges(graph: &ModuleGraph, source_id: SourceId) -> IdentifierRanges {
    graph
        .bodies()
        .flat_map(|body| body.unresolved_references.iter())
        .map(|reference| reference.origin.span)
        .filter(|span| span.source == source_id)
        .filter_map(token_range)
        .map(|range| (range.start, range.end))
        .collect()
}
