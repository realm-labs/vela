use vela_syntax::SyntaxKind;
use vela_syntax::ast::{SyntaxExpression, SyntaxStatement, SyntaxStatementKind};

use crate::compiler::body_payloads::CompilerBodyPayload;

pub(super) fn syntax_statement_requires_body_block_lookup(statement: &SyntaxStatement) -> bool {
    match statement.statement_kind() {
        SyntaxStatementKind::Let => statement.as_let().is_none_or(|let_statement| {
            let_statement.name_text().is_none()
                || let_statement
                    .initializer()
                    .is_some_and(|expression| !syntax_expression_is_simple_value(&expression))
        }),
        SyntaxStatementKind::Break | SyntaxStatementKind::Continue => false,
        SyntaxStatementKind::Return => statement.as_return().is_none_or(|return_statement| {
            return_statement
                .expression()
                .is_some_and(|expression| !syntax_expression_is_simple_value(&expression))
        }),
        SyntaxStatementKind::Block => statement
            .as_block()
            .is_none_or(|block| CompilerBodyPayload::requires_body_block_lookup(&block)),
        _ => true,
    }
}

pub(super) fn expression_syntax_path_or_self(expression: &SyntaxExpression) -> Option<Vec<String>> {
    let path = expression.as_path()?;
    if path.is_self() {
        Some(vec!["self".to_owned()])
    } else {
        Some(path.path_segments())
    }
}

fn syntax_expression_is_simple_literal(expression: &SyntaxExpression) -> bool {
    let Some(literal) = expression.as_literal() else {
        return false;
    };
    !matches!(literal.token_kind(), Some(SyntaxKind::InterpolatedString))
        && literal.literal().is_some()
}

fn syntax_expression_is_simple_path(expression: &SyntaxExpression) -> bool {
    expression
        .as_path()
        .is_some_and(|path| path.is_self() || !path.path_segments().is_empty())
}

fn syntax_expression_is_simple_block(expression: &SyntaxExpression) -> bool {
    expression
        .as_block()
        .is_some_and(|block| !CompilerBodyPayload::requires_body_block_lookup(&block))
}

fn syntax_expression_is_simple_value(expression: &SyntaxExpression) -> bool {
    syntax_expression_is_simple_literal(expression)
        || syntax_expression_is_simple_path(expression)
        || syntax_expression_is_simple_block(expression)
}
