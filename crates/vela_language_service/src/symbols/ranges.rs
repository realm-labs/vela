//! CST outline extents keyed by existing HIR name/item offsets. HIR spans keep
//! their semantic/name ownership; outlining does not rewrite those contracts.
use crate::{DiagnosticRange, LineIndex, SourceRecord, TextRange};
use std::collections::BTreeMap;
use vela_common::Span;
use vela_syntax::{
    SyntaxKind, SyntaxNode,
    ast::{
        AstNode, SyntaxConstItem, SyntaxEnumItem, SyntaxEnumVariant, SyntaxFunctionItem,
        SyntaxImplItem, SyntaxImplMethod, SyntaxParam, SyntaxStateItem, SyntaxStructField,
        SyntaxStructItem, SyntaxTraitItem, SyntaxTraitMethod,
    },
};

pub(super) struct SymbolSource<'a> {
    pub(super) record: &'a SourceRecord,
    index: LineIndex,
    ranges: BTreeMap<usize, (TextRange, TextRange)>,
}

impl<'a> SymbolSource<'a> {
    pub(super) fn new(record: &'a SourceRecord, root: SyntaxNode) -> Self {
        let mut ranges = BTreeMap::new();
        for node in root.descendants() {
            let Some(selection) = selection(&node) else {
                continue;
            };
            let syntax = node.text_range();
            let start = usize::from(syntax.start());
            let end = usize::from(syntax.end());
            let text = &record.text()[start..end];
            let leading = text.len() - text.trim_start().len();
            let extent = TextRange::new(start + leading, start + text.trim_end().len());
            ranges.insert(start, (extent, selection));
            ranges.insert(selection.start, (extent, selection));
        }
        Self {
            record,
            index: LineIndex::new(record.text()),
            ranges,
        }
    }

    pub(super) fn ranges(&self, span: Span) -> Option<(DiagnosticRange, DiagnosticRange)> {
        if span.source != self.record.source_id() {
            return None;
        }
        let (extent, selection) = self.ranges.get(&usize::try_from(span.start).ok()?)?;
        let convert = |range: &TextRange| {
            DiagnosticRange::new(
                self.index.position(range.start),
                self.index.position(range.end),
            )
        };
        Some((convert(extent), convert(selection)))
    }
}

fn selection(node: &SyntaxNode) -> Option<TextRange> {
    macro_rules! name {
        ($ty:ty) => {
            <$ty>::cast(node.clone())?.name_token()?.text_range()
        };
    }
    let range = match node.kind() {
        SyntaxKind::ConstItem => name!(SyntaxConstItem),
        SyntaxKind::StateItem => name!(SyntaxStateItem),
        SyntaxKind::FunctionItem => name!(SyntaxFunctionItem),
        SyntaxKind::StructItem => name!(SyntaxStructItem),
        SyntaxKind::EnumItem => name!(SyntaxEnumItem),
        SyntaxKind::TraitItem => name!(SyntaxTraitItem),
        SyntaxKind::StructField => name!(SyntaxStructField),
        SyntaxKind::EnumVariant => name!(SyntaxEnumVariant),
        SyntaxKind::Param => name!(SyntaxParam),
        SyntaxKind::TraitMethod => name!(SyntaxTraitMethod),
        SyntaxKind::ImplMethod => name!(SyntaxImplMethod),
        SyntaxKind::ImplItem => {
            let item = SyntaxImplItem::cast(node.clone())?;
            let start = item.impl_token()?.text_range().start();
            let end = item.target_path_tokens().last()?.text_range().end();
            return Some(TextRange::new(usize::from(start), usize::from(end)));
        }
        _ => return None,
    };
    Some(TextRange::new(
        usize::from(range.start()),
        usize::from(range.end()),
    ))
}
