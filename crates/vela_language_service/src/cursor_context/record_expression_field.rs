use vela_syntax::ast::{AstNode, SyntaxRecordExpr, SyntaxRecordExprField, SyntaxSourceFile};
use vela_syntax::{TextRange as SyntaxTextRange, TextSize};

pub(super) fn is_record_expression_field_context(source: &SyntaxSourceFile, offset: usize) -> bool {
    let Some(offset) = syntax_offset(offset) else {
        return false;
    };
    source
        .syntax()
        .descendants()
        .filter_map(SyntaxRecordExpr::cast)
        .any(|expr| {
            expr.l_brace_token()
                .is_some_and(|brace| brace.text_range().end() <= offset)
                && (range_contains_offset(expr.syntax().text_range(), offset)
                    || (expr.r_brace_token().is_none()
                        && at_eof_after_whitespace(
                            source,
                            expr.syntax().text_range().end(),
                            offset,
                        )))
                && !expr
                    .fields()
                    .iter()
                    .any(|field| field_value_contains_offset(source, field, offset))
        })
}

pub(super) fn is_record_expression_value_context(source: &SyntaxSourceFile, offset: usize) -> bool {
    let Some(offset) = syntax_offset(offset) else {
        return false;
    };
    source
        .syntax()
        .descendants()
        .filter_map(SyntaxRecordExprField::cast)
        .any(|field| field_value_contains_offset(source, &field, offset))
}

fn field_value_contains_offset(
    source: &SyntaxSourceFile,
    field: &SyntaxRecordExprField,
    offset: TextSize,
) -> bool {
    field.colon_token().is_some_and(|colon| {
        colon.text_range().end() <= offset
            && (offset <= field.syntax().text_range().end()
                || at_eof_after_whitespace(source, field.syntax().text_range().end(), offset))
    })
}

fn at_eof_after_whitespace(source: &SyntaxSourceFile, end: TextSize, offset: TextSize) -> bool {
    offset == source.syntax().text_range().end()
        && end <= offset
        && source
            .syntax()
            .text()
            .slice(SyntaxTextRange::new(end, offset))
            .to_string()
            .trim()
            .is_empty()
}

fn range_contains_offset(range: SyntaxTextRange, offset: TextSize) -> bool {
    range.start() <= offset && offset < range.end()
}

fn syntax_offset(offset: usize) -> Option<TextSize> {
    let offset = u32::try_from(offset).ok()?;
    Some(TextSize::from(offset))
}
