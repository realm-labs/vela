use vela_common::Span;
use vela_hir::body::{HirPath, HirPathKind};
use vela_hir::module_graph::{Declaration, ImportResolution, ModuleGraph};

use crate::TextRange;

pub(crate) fn binding_maps(
    graph: &ModuleGraph,
) -> impl Iterator<Item = &vela_hir::binding::BindingMap> {
    let mut seen = std::collections::BTreeSet::new();
    graph
        .bodies()
        .filter_map(|body| graph.bindings_for_body(body.id))
        .filter(move |bindings| seen.insert(bindings.body()))
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PathSite<'a> {
    pub(crate) path: &'a [String],
    pub(crate) segment_range: TextRange,
}

pub(crate) fn site(path: &HirPath) -> Option<PathSite<'_>> {
    Some(PathSite {
        path: path.path.as_slice(),
        segment_range: text_range_for_span(path.segment_origin.span)?,
    })
}

pub(crate) const fn is_expression_path(kind: HirPathKind) -> bool {
    matches!(
        kind,
        HirPathKind::Value | HirPathKind::Callee | HirPathKind::Constructor
    )
}

pub(crate) fn text_range_for_span(span: Span) -> Option<TextRange> {
    Some(TextRange::new(
        usize::try_from(span.start).ok()?,
        usize::try_from(span.end).ok()?,
    ))
}

pub(crate) fn resolved_use_range(graph: &ModuleGraph, span: Span) -> Option<TextRange> {
    let range = text_range_for_span(span)?;
    Some(
        graph
            .paths_in_source(span.source)
            .filter(|path| is_expression_path(path.kind))
            .filter_map(site)
            .filter(|site| {
                range.start <= site.segment_range.start && site.segment_range.end == range.end
            })
            .map(|site| site.segment_range)
            .max_by_key(|segment| segment.start)
            .unwrap_or(range),
    )
}

pub(crate) fn imported_declaration(
    graph: &ModuleGraph,
    source: vela_common::SourceId,
    range: TextRange,
) -> Option<&Declaration> {
    graph
        .module_ids()
        .filter_map(|module| graph.imports(module))
        .flatten()
        .find_map(|import| {
            if import.span.source != source
                || import
                    .path_spans
                    .last()
                    .copied()
                    .and_then(text_range_for_span)
                    != Some(range)
            {
                return None;
            }
            let ImportResolution::Declaration(declaration) = import.resolution?;
            graph.declaration(declaration)
        })
}
