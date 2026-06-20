use vela_syntax::ast::{AstNode, SyntaxFieldExpr, SyntaxSourceFile};
use vela_syntax::{TextRange as SyntaxTextRange, TextSize};

use crate::TextRange;

pub(super) fn member_receiver_for_source(
    source: &SyntaxSourceFile,
    offset: usize,
) -> Option<TextRange> {
    let offset = syntax_offset(offset)?;
    source
        .syntax()
        .descendants()
        .filter_map(SyntaxFieldExpr::cast)
        .find_map(|field| member_receiver_for_field(&field, offset))
}

fn member_receiver_for_field(field: &SyntaxFieldExpr, offset: TextSize) -> Option<TextRange> {
    let name = field.name_token()?;
    if !range_contains_offset(name.text_range(), offset) {
        return None;
    }
    field
        .receiver()
        .and_then(|receiver| text_range(receiver.syntax().text_range()))
}

fn text_range(range: SyntaxTextRange) -> Option<TextRange> {
    Some(TextRange::new(
        text_size_to_usize(range.start()),
        text_size_to_usize(range.end()),
    ))
}

fn text_size_to_usize(size: TextSize) -> usize {
    u32::from(size) as usize
}

fn range_contains_offset(range: SyntaxTextRange, offset: TextSize) -> bool {
    range.start() <= offset && offset <= range.end()
}

fn syntax_offset(offset: usize) -> Option<TextSize> {
    let offset = u32::try_from(offset).ok()?;
    Some(TextSize::from(offset))
}
