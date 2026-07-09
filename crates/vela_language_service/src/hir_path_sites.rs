use vela_common::Span;
use vela_hir::body::{HirPath, HirPathKind};

use crate::TextRange;

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
