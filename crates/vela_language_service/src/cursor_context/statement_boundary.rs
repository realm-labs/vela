use vela_syntax::ast::{AstNode, SyntaxSourceFile, SyntaxStatement};
use vela_syntax::{TextRange as SyntaxTextRange, TextSize};

pub(super) fn is_inside_item(source: &SyntaxSourceFile, offset: usize) -> bool {
    let Some(offset) = syntax_offset(offset) else {
        return false;
    };
    source.items().any(|item| {
        let start = item
            .syntax()
            .first_token()
            .map(|token| token.text_range().start())
            .unwrap_or(item.text_range().start());
        start < offset && offset < item.text_range().end()
    })
}

pub(super) fn is_statement_context(source: &SyntaxSourceFile, offset: usize) -> bool {
    let Some(offset) = syntax_offset(offset) else {
        return false;
    };
    source
        .syntax()
        .descendants()
        .filter_map(SyntaxStatement::cast)
        .any(|statement| is_statement_start(&statement, offset))
}

pub(super) fn is_statement_prefix(prefix: &str) -> bool {
    ["let", "return", "for", "if", "match", "break", "continue"]
        .iter()
        .any(|keyword| keyword.starts_with(prefix))
}

fn is_statement_start(statement: &SyntaxStatement, offset: TextSize) -> bool {
    let statement_range = statement.syntax().text_range();
    if !range_contains_offset(statement_range, offset) {
        return false;
    }
    statement
        .syntax()
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| !token.kind().is_trivia())
        .is_some_and(|token| offset <= token.text_range().start())
}

fn range_contains_offset(range: SyntaxTextRange, offset: TextSize) -> bool {
    range.start() <= offset && offset < range.end()
}

fn syntax_offset(offset: usize) -> Option<TextSize> {
    let offset = u32::try_from(offset).ok()?;
    Some(TextSize::from(offset))
}
