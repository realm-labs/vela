use vela_syntax::ast::{AstNode, SyntaxSourceFile, SyntaxStatement, SyntaxStatementKind};
use vela_syntax::{TextRange as SyntaxTextRange, TextSize};

pub(super) fn is_inside_item(source: &SyntaxSourceFile, offset: usize) -> bool {
    let Some(offset) = syntax_offset(offset) else {
        return false;
    };
    source
        .items()
        .any(|item| range_contains_offset(item.text_range(), offset))
}

pub(super) fn is_statement_context(source: &SyntaxSourceFile, offset: usize) -> bool {
    let Some(offset) = syntax_offset(offset) else {
        return false;
    };
    source
        .functions()
        .filter_map(|function| function.body())
        .flat_map(|body| body.statements())
        .any(|statement| is_statement_start(&statement, offset))
}

fn is_statement_start(statement: &SyntaxStatement, offset: TextSize) -> bool {
    let statement_range = statement.syntax().text_range();
    if !range_contains_offset(statement_range, offset) {
        return false;
    }
    match statement.statement_kind() {
        SyntaxStatementKind::Let => offset <= statement_start(statement) + TextSize::from(4),
        SyntaxStatementKind::Return
        | SyntaxStatementKind::Break
        | SyntaxStatementKind::Continue => true,
        SyntaxStatementKind::Expr => statement
            .as_expr()
            .and_then(|statement| statement.expression())
            .is_some_and(|expression| {
                offset <= expression.syntax().text_range().start() + TextSize::from(1)
            }),
        SyntaxStatementKind::For
        | SyntaxStatementKind::If
        | SyntaxStatementKind::Match
        | SyntaxStatementKind::Block => offset <= statement_start(statement) + TextSize::from(1),
    }
}

fn statement_start(statement: &SyntaxStatement) -> TextSize {
    statement.syntax().text_range().start()
}

fn range_contains_offset(range: SyntaxTextRange, offset: TextSize) -> bool {
    range.start() <= offset && offset < range.end()
}

fn syntax_offset(offset: usize) -> Option<TextSize> {
    let offset = u32::try_from(offset).ok()?;
    Some(TextSize::from(offset))
}
