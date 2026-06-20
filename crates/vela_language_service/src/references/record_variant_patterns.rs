use vela_syntax::ast::{AstNode, SyntaxRecordPattern, SyntaxSourceFile};
use vela_syntax::{Parse as SyntaxParse, TextRange as SyntaxTextRange, TextSize};

use crate::TextRange;

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct RecordPatternFieldSite {
    pub(super) path: Vec<String>,
    pub(super) name: String,
    pub(super) name_range: TextRange,
}

pub(super) fn record_pattern_field_sites(
    parse: &SyntaxParse<SyntaxSourceFile>,
) -> Vec<RecordPatternFieldSite> {
    let source = parse.tree();
    source
        .syntax()
        .descendants()
        .filter_map(SyntaxRecordPattern::cast)
        .flat_map(record_pattern_field_sites_for_pattern)
        .collect()
}

fn record_pattern_field_sites_for_pattern(
    pattern: SyntaxRecordPattern,
) -> Vec<RecordPatternFieldSite> {
    let path = pattern.path_segments();
    pattern
        .fields()
        .filter_map(|field| {
            let label = field.label_token()?;
            Some(RecordPatternFieldSite {
                path: path.clone(),
                name: label.text().to_owned(),
                name_range: text_range(label.text_range()),
            })
        })
        .collect()
}

fn text_range(range: SyntaxTextRange) -> TextRange {
    TextRange::new(
        text_size_to_usize(range.start()),
        text_size_to_usize(range.end()),
    )
}

fn text_size_to_usize(size: TextSize) -> usize {
    u32::from(size) as usize
}
