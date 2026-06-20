use vela_syntax::ast::{AstNode, SyntaxRecordExpr, SyntaxSourceFile};
use vela_syntax::{TextRange as SyntaxTextRange, TextSize};

pub(super) fn is_record_expression_field_context(source: &SyntaxSourceFile, offset: usize) -> bool {
    let Some(offset) = syntax_offset(offset) else {
        return false;
    };
    source
        .syntax()
        .descendants()
        .filter_map(SyntaxRecordExpr::cast)
        .any(|expr| range_contains_offset(expr.syntax().text_range(), offset))
}

fn range_contains_offset(range: SyntaxTextRange, offset: TextSize) -> bool {
    range.start() <= offset && offset < range.end()
}

fn syntax_offset(offset: usize) -> Option<TextSize> {
    let offset = u32::try_from(offset).ok()?;
    Some(TextSize::from(offset))
}
