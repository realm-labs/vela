use vela_syntax::SyntaxKind;
use vela_syntax::ast::{
    IntegerSuffix, Literal, SyntaxExpression, SyntaxStatement, SyntaxStatementKind, UnaryOp,
};

use crate::compiler::body_payloads::CompilerBodyPayload;

pub(super) fn syntax_statement_requires_body_block_lookup(statement: &SyntaxStatement) -> bool {
    match statement.statement_kind() {
        SyntaxStatementKind::Let => statement.as_let().is_none_or(|let_statement| {
            if let_statement.name_text().is_none() {
                return true;
            }
            let Some(initializer) = let_statement.initializer() else {
                return false;
            };
            if syntax_expression_is_simple_value(&initializer) {
                return false;
            }
            let_statement.type_hint().is_some()
                || !syntax_expression_is_simple_negated_number(&initializer)
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
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return expression_syntax_path_or_self(&inner);
    }
    let path = expression.as_path()?;
    if path.is_self() {
        Some(vec!["self".to_owned()])
    } else {
        Some(path.path_segments())
    }
}

pub(super) fn expression_syntax_literal(expression: &SyntaxExpression) -> Option<Literal> {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return expression_syntax_literal(&inner);
    }
    expression.as_literal()?.literal()
}

pub(super) fn expression_syntax_negated_number_literal(
    expression: &SyntaxExpression,
) -> Option<Literal> {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return expression_syntax_negated_number_literal(&inner);
    }
    let unary = expression.as_unary()?;
    (unary.operator() == Some(UnaryOp::Negate)).then_some(())?;
    let operand = unary.expression()?;
    expression_syntax_negatable_number_literal(&operand)
}

fn syntax_expression_is_simple_literal(expression: &SyntaxExpression) -> bool {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return syntax_expression_is_simple_literal(&inner);
    }
    let Some(literal) = expression.as_literal() else {
        return false;
    };
    !matches!(literal.token_kind(), Some(SyntaxKind::InterpolatedString))
        && literal.literal().is_some()
}

fn syntax_expression_is_simple_path(expression: &SyntaxExpression) -> bool {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return syntax_expression_is_simple_path(&inner);
    }
    expression
        .as_path()
        .is_some_and(|path| path.is_self() || !path.path_segments().is_empty())
}

fn syntax_expression_is_simple_block(expression: &SyntaxExpression) -> bool {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return syntax_expression_is_simple_block(&inner);
    }
    expression
        .as_block()
        .is_some_and(|block| !CompilerBodyPayload::requires_body_block_lookup(&block))
}

fn syntax_expression_is_simple_negated_number(expression: &SyntaxExpression) -> bool {
    expression_syntax_negated_number_literal(expression).is_some()
}

fn expression_syntax_negatable_number_literal(expression: &SyntaxExpression) -> Option<Literal> {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return expression_syntax_negatable_number_literal(&inner);
    }
    let literal = expression
        .as_literal()
        .and_then(|literal| literal.literal())?;
    match literal {
        Literal::Integer(ref value)
            if matches!(
                value.suffix,
                Some(
                    IntegerSuffix::U8
                        | IntegerSuffix::U16
                        | IntegerSuffix::U32
                        | IntegerSuffix::U64
                )
            ) =>
        {
            None
        }
        Literal::Integer(_) | Literal::Float(_) => Some(literal),
        _ => None,
    }
}

fn syntax_expression_is_simple_value(expression: &SyntaxExpression) -> bool {
    syntax_expression_is_simple_literal(expression)
        || syntax_expression_is_simple_path(expression)
        || syntax_expression_is_simple_block(expression)
        || syntax_expression_is_simple_negated_number(expression)
}
