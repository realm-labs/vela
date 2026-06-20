use vela_syntax::ast::{AstNode, SyntaxRecordExpr, SyntaxSourceFile};
use vela_syntax::{Parse as SyntaxParse, TextRange as SyntaxTextRange, TextSize};

use crate::TextRange;

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct RecordFieldSite {
    pub(super) path: Vec<String>,
    pub(super) name: String,
    pub(super) name_range: TextRange,
}

pub(super) fn record_field_sites(parse: &SyntaxParse<SyntaxSourceFile>) -> Vec<RecordFieldSite> {
    let source = parse.tree();
    source
        .syntax()
        .descendants()
        .filter_map(SyntaxRecordExpr::cast)
        .flat_map(record_field_sites_for_expr)
        .collect()
}

fn record_field_sites_for_expr(expr: SyntaxRecordExpr) -> Vec<RecordFieldSite> {
    let path = expr.path_segments();
    expr.fields()
        .into_iter()
        .filter_map(|field| {
            let label = field.label_token()?;
            Some(RecordFieldSite {
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
