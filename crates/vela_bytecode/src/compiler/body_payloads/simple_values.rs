#[cfg(test)]
use std::collections::BTreeMap;

#[cfg(test)]
use vela_common::SourceId;
#[cfg(test)]
use vela_syntax::SyntaxKind;
use vela_syntax::ast::{BinaryOp, IntegerSuffix, Literal, SyntaxExpression, UnaryOp};
#[cfg(test)]
use vela_syntax::ast::{
    FloatSuffix, SyntaxElseBranch, SyntaxExpressionKind, SyntaxIfExpr, SyntaxStatement,
    SyntaxStatementKind,
};

#[cfg(test)]
use crate::compiler::body_payloads::CompilerBodyPayload;
#[cfg(test)]
use crate::compiler::const_eval::evaluate_syntax_const_expr;

#[cfg(test)]
pub(super) fn syntax_statement_requires_body_block_lookup(
    statement: &SyntaxStatement,
    allow_unterminated_cst_expression: bool,
) -> bool {
    match statement.statement_kind() {
        SyntaxStatementKind::Let => statement.as_let().is_none_or(|let_statement| {
            if let_statement.name_text().is_none() {
                return true;
            }
            let Some(initializer) = let_statement.initializer() else {
                return false;
            };
            if let_statement.type_hint().is_some()
                && syntax_expression_is_constant_container(&initializer)
            {
                return false;
            }
            if syntax_expression_is_simple_value(&initializer) {
                return false;
            }
            if let_statement.type_hint().is_none()
                && syntax_expression_is_simple_path_field(&initializer)
            {
                return false;
            }
            if syntax_expression_is_simple_path_unary(&initializer) {
                return false;
            }
            if let_statement.type_hint().is_none()
                && syntax_expression_is_simple_path_binary(&initializer)
            {
                return false;
            }
            if let_statement.type_hint().is_none()
                && syntax_expression_is_simple_path_comparison(&initializer)
            {
                return false;
            }
            if let_statement.type_hint().is_none()
                && syntax_expression_is_simple_path_arithmetic(&initializer)
            {
                return false;
            }
            if let_statement.type_hint().is_none()
                && syntax_expression_is_simple_path_logical(&initializer)
            {
                return false;
            }
            if let_statement.type_hint().is_none() && syntax_expression_is_simple_try(&initializer)
            {
                return false;
            }
            if let_statement.type_hint().is_none()
                && syntax_expression_is_simple_container(&initializer)
            {
                return false;
            }
            if let_statement.type_hint().is_none()
                && syntax_expression_is_simple_path_numeric_comparison(&initializer)
            {
                return false;
            }
            if let_statement.type_hint().is_none()
                && syntax_expression_is_simple_path_numeric_equality(&initializer)
            {
                return false;
            }
            if syntax_expression_is_simple_path_numeric_arithmetic(&initializer) {
                return false;
            }
            let_statement.type_hint().is_some()
                || !syntax_expression_is_simple_negated_number(&initializer)
        }),
        SyntaxStatementKind::Break | SyntaxStatementKind::Continue => false,
        SyntaxStatementKind::Return => statement.as_return().is_none_or(|return_statement| {
            return_statement.expression().is_some_and(|expression| {
                !syntax_expression_is_simple_value(&expression)
                    && !syntax_expression_is_simple_path_field(&expression)
                    && !syntax_expression_is_simple_path_unary(&expression)
                    && !syntax_expression_is_simple_path_binary(&expression)
                    && !syntax_expression_is_simple_path_comparison(&expression)
                    && !syntax_expression_is_simple_path_arithmetic(&expression)
                    && !syntax_expression_is_simple_path_logical(&expression)
                    && !syntax_expression_is_simple_try(&expression)
                    && !syntax_expression_is_simple_container(&expression)
                    && !syntax_expression_is_simple_path_numeric_comparison(&expression)
                    && !syntax_expression_is_simple_path_numeric_equality(&expression)
                    && !syntax_expression_is_simple_path_numeric_arithmetic(&expression)
            })
        }),
        SyntaxStatementKind::Block => statement
            .as_block()
            .is_none_or(|block| CompilerBodyPayload::requires_body_block_lookup(&block)),
        SyntaxStatementKind::If => statement
            .as_if()
            .is_none_or(|if_expr| !syntax_if_statement_is_cst_lowerable(&if_expr)),
        SyntaxStatementKind::Expr => statement.as_expr().is_none_or(|expr_statement| {
            if expr_statement.semicolon_token().is_some() || allow_unterminated_cst_expression {
                return expr_statement.expression().is_none_or(|expression| {
                    !syntax_expression_statement_is_cst_lowerable(&expression)
                });
            }
            true
        }),
        _ => true,
    }
}

#[cfg(test)]
fn syntax_expression_statement_is_cst_lowerable(expression: &SyntaxExpression) -> bool {
    syntax_expression_is_inline_constant(expression)
        || syntax_expression_is_simple_path(expression)
        || syntax_expression_is_simple_range(expression)
        || syntax_expression_is_statement_block(expression)
        || syntax_expression_is_simple_path_field(expression)
        || syntax_expression_is_simple_path_unary(expression)
        || syntax_expression_is_simple_path_binary(expression)
        || syntax_expression_is_simple_path_comparison(expression)
        || syntax_expression_is_simple_path_arithmetic(expression)
        || syntax_expression_is_simple_path_logical(expression)
        || syntax_expression_is_simple_try(expression)
        || syntax_expression_is_simple_container(expression)
        || syntax_expression_is_simple_path_numeric_comparison(expression)
        || syntax_expression_is_simple_path_numeric_equality(expression)
        || syntax_expression_is_simple_path_numeric_arithmetic(expression)
}

pub(in crate::compiler) fn expression_syntax_path_or_self(
    expression: &SyntaxExpression,
) -> Option<Vec<String>> {
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

pub(in crate::compiler) fn expression_syntax_path_field(
    expression: &SyntaxExpression,
) -> Option<Vec<String>> {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return expression_syntax_path_field(&inner);
    }
    let field = expression.as_field()?;
    let receiver = field.receiver()?;
    let mut path = expression_syntax_path_or_self(&receiver)
        .or_else(|| expression_syntax_path_field(&receiver))?;
    path.push(field.name_text()?);
    Some(path)
}

pub(in crate::compiler) fn expression_syntax_path_or_field(
    expression: &SyntaxExpression,
) -> Option<Vec<String>> {
    expression_syntax_path_or_self(expression).or_else(|| expression_syntax_path_field(expression))
}

pub(in crate::compiler) fn expression_syntax_literal(
    expression: &SyntaxExpression,
) -> Option<Literal> {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return expression_syntax_literal(&inner);
    }
    expression.as_literal()?.literal()
}

pub(in crate::compiler) fn expression_syntax_negated_number_literal(
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

#[cfg(test)]
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

#[cfg(test)]
fn syntax_expression_is_simple_interpolated_string(expression: &SyntaxExpression) -> bool {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return syntax_expression_is_simple_interpolated_string(&inner);
    }
    let Some(literal) = expression.as_literal() else {
        return false;
    };
    if literal.token_kind() != Some(SyntaxKind::InterpolatedString) {
        return false;
    }
    let mut has_field_value = false;
    let all_supported = literal.interpolation_expressions().all(|expression| {
        has_field_value |= syntax_expression_contains_simple_field_value(&expression);
        syntax_expression_is_simple_interpolation_value(&expression)
    });
    all_supported && has_field_value
}

#[cfg(test)]
fn syntax_expression_is_simple_interpolation_value(expression: &SyntaxExpression) -> bool {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return syntax_expression_is_simple_interpolation_value(&inner);
    }
    syntax_expression_is_simple_value(expression)
        || syntax_expression_is_simple_path_field(expression)
        || syntax_expression_is_simple_path_unary(expression)
        || syntax_expression_is_simple_path_binary(expression)
        || syntax_expression_is_simple_path_comparison(expression)
        || syntax_expression_is_simple_path_arithmetic(expression)
        || syntax_expression_is_simple_path_logical(expression)
        || syntax_expression_is_simple_try(expression)
        || syntax_expression_is_simple_path_numeric_comparison(expression)
        || syntax_expression_is_simple_path_numeric_equality(expression)
        || syntax_expression_is_simple_path_numeric_arithmetic(expression)
}

#[cfg(test)]
fn syntax_expression_is_simple_if_value(expression: &SyntaxExpression) -> bool {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return syntax_expression_is_simple_if_value(&inner);
    }
    expression
        .as_if()
        .is_some_and(|if_expr| syntax_if_is_simple_value(&if_expr))
}

#[cfg(test)]
fn syntax_if_is_simple_value(if_expr: &SyntaxIfExpr) -> bool {
    let Some(condition) = if_expr.condition() else {
        return false;
    };
    if !syntax_expression_is_simple_interpolation_value(&condition)
        || !syntax_expression_contains_simple_field_value(&condition)
    {
        return false;
    }
    if if_expr
        .then_block()
        .is_none_or(|block| CompilerBodyPayload::requires_body_block_lookup(&block))
    {
        return false;
    }
    match if_expr.else_branch() {
        Some(SyntaxElseBranch::If(else_if)) => syntax_if_is_simple_value(&else_if),
        Some(SyntaxElseBranch::Block(block)) => {
            !CompilerBodyPayload::requires_body_block_lookup(&block)
        }
        None => true,
    }
}

#[cfg(test)]
fn syntax_if_statement_is_cst_lowerable(if_expr: &SyntaxIfExpr) -> bool {
    let Some(condition) = if_expr.condition() else {
        return false;
    };
    if !syntax_expression_is_simple_if_statement_condition(&condition) {
        return false;
    }
    if if_expr
        .then_block()
        .is_none_or(|block| CompilerBodyPayload::requires_body_block_lookup(&block))
    {
        return false;
    }
    match if_expr.else_branch() {
        Some(SyntaxElseBranch::If(else_if)) => syntax_if_statement_is_cst_lowerable(&else_if),
        Some(SyntaxElseBranch::Block(block)) => {
            !CompilerBodyPayload::requires_body_block_lookup(&block)
        }
        None => true,
    }
}

#[cfg(test)]
fn syntax_expression_is_simple_if_statement_condition(expression: &SyntaxExpression) -> bool {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return syntax_expression_is_simple_if_statement_condition(&inner);
    }
    expression.as_block().is_none() && syntax_expression_is_simple_interpolation_value(expression)
}

#[cfg(test)]
fn syntax_expression_is_simple_path(expression: &SyntaxExpression) -> bool {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return syntax_expression_is_simple_path(&inner);
    }
    expression
        .as_path()
        .is_some_and(|path| path.is_self() || !path.path_segments().is_empty())
}

#[cfg(test)]
fn syntax_expression_is_simple_path_or_field(expression: &SyntaxExpression) -> bool {
    expression_syntax_path_or_field(expression).is_some()
}

#[cfg(test)]
fn syntax_expression_is_simple_path_field(expression: &SyntaxExpression) -> bool {
    expression_syntax_path_field(expression).is_some()
}

#[cfg(test)]
fn syntax_expression_is_simple_empty_array(expression: &SyntaxExpression) -> bool {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return syntax_expression_is_simple_empty_array(&inner);
    }
    expression
        .as_array()
        .is_some_and(|array| array.expressions().next().is_none())
}

#[cfg(test)]
fn syntax_expression_is_constant_container(expression: &SyntaxExpression) -> bool {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return syntax_expression_is_constant_container(&inner);
    }
    matches!(
        expression.expression_kind(),
        SyntaxExpressionKind::Array | SyntaxExpressionKind::Map
    ) && evaluate_syntax_const_expr(SourceId::new(0), expression, &BTreeMap::new())
        .ok()
        .flatten()
        .is_some()
}

#[cfg(test)]
fn syntax_expression_is_inline_constant(expression: &SyntaxExpression) -> bool {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return syntax_expression_is_inline_constant(&inner);
    }
    let has_direct_block_value = match expression.expression_kind() {
        SyntaxExpressionKind::Array => expression.as_array().is_none_or(|array| {
            array
                .expressions()
                .any(|element| element.expression_kind() == SyntaxExpressionKind::Block)
        }),
        SyntaxExpressionKind::Map => expression.as_map().is_none_or(|map| {
            map.entries().any(|entry| {
                entry
                    .expressions()
                    .any(|value| value.expression_kind() == SyntaxExpressionKind::Block)
            })
        }),
        _ => false,
    };
    !has_direct_block_value
        && evaluate_syntax_const_expr(SourceId::new(0), expression, &BTreeMap::new())
            .ok()
            .flatten()
            .is_some()
}

pub(in crate::compiler) fn expression_syntax_range_operands(
    expression: &SyntaxExpression,
) -> Option<(SyntaxExpression, SyntaxExpression, bool)> {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return expression_syntax_range_operands(&inner);
    }
    let binary = expression.as_binary()?;
    let inclusive = match binary.operator()? {
        BinaryOp::Range => false,
        BinaryOp::RangeInclusive => true,
        _ => return None,
    };
    Some((binary.lhs()?, binary.rhs()?, inclusive))
}

#[cfg(test)]
fn syntax_expression_is_simple_range(expression: &SyntaxExpression) -> bool {
    let Some((lhs, rhs, _)) = expression_syntax_range_operands(expression) else {
        return false;
    };
    syntax_expression_is_simple_range_operand(&lhs)
        && syntax_expression_is_simple_range_operand(&rhs)
}

#[cfg(test)]
fn syntax_expression_is_simple_range_operand(expression: &SyntaxExpression) -> bool {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return syntax_expression_is_simple_range_operand(&inner);
    }
    syntax_expression_is_simple_literal(expression)
        || syntax_expression_is_simple_path_or_field(expression)
        || syntax_expression_is_simple_negated_number(expression)
        || syntax_expression_is_simple_constant_unary(expression)
        || syntax_expression_is_simple_constant_arithmetic(expression)
}

#[cfg(test)]
fn syntax_expression_is_simple_path_numeric_arithmetic(expression: &SyntaxExpression) -> bool {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return syntax_expression_is_simple_path_numeric_arithmetic(&inner);
    }
    let Some(binary) = expression.as_binary() else {
        return false;
    };
    if !matches!(
        binary.operator(),
        Some(BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem)
    ) {
        return false;
    }
    let Some(lhs) = binary.lhs() else {
        return false;
    };
    let Some(rhs) = binary.rhs() else {
        return false;
    };
    syntax_expression_has_path_and_numeric_literal_operands(&lhs, &rhs)
}

#[cfg(test)]
fn syntax_expression_is_simple_path_numeric_comparison(expression: &SyntaxExpression) -> bool {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return syntax_expression_is_simple_path_numeric_comparison(&inner);
    }
    let Some(binary) = expression.as_binary() else {
        return false;
    };
    if !matches!(
        binary.operator(),
        Some(BinaryOp::Less | BinaryOp::LessEqual | BinaryOp::Greater | BinaryOp::GreaterEqual)
    ) {
        return false;
    }
    let Some(lhs) = binary.lhs() else {
        return false;
    };
    let Some(rhs) = binary.rhs() else {
        return false;
    };
    syntax_expression_has_path_and_numeric_literal_operands(&lhs, &rhs)
}

#[cfg(test)]
fn syntax_expression_is_simple_path_numeric_equality(expression: &SyntaxExpression) -> bool {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return syntax_expression_is_simple_path_numeric_equality(&inner);
    }
    let Some(binary) = expression.as_binary() else {
        return false;
    };
    if !matches!(
        binary.operator(),
        Some(BinaryOp::Equal | BinaryOp::NotEqual)
    ) {
        return false;
    }
    let Some(lhs) = binary.lhs() else {
        return false;
    };
    let Some(rhs) = binary.rhs() else {
        return false;
    };
    syntax_expression_has_path_and_numeric_literal_operands(&lhs, &rhs)
}

#[cfg(test)]
fn syntax_expression_has_path_and_numeric_literal_operands(
    lhs: &SyntaxExpression,
    rhs: &SyntaxExpression,
) -> bool {
    (expression_syntax_path_or_field(lhs).is_some()
        && expression_syntax_numeric_literal_kind(rhs).is_some())
        || (expression_syntax_numeric_literal_kind(lhs).is_some()
            && expression_syntax_path_or_field(rhs).is_some())
}

#[cfg(test)]
fn syntax_expression_is_simple_path_binary(expression: &SyntaxExpression) -> bool {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return syntax_expression_is_simple_path_binary(&inner);
    }
    let Some(binary) = expression.as_binary() else {
        return false;
    };
    if !matches!(
        binary.operator(),
        Some(
            BinaryOp::Equal
                | BinaryOp::NotEqual
                | BinaryOp::IdentityEqual
                | BinaryOp::IdentityNotEqual
        )
    ) {
        return false;
    }
    let Some(lhs) = binary.lhs() else {
        return false;
    };
    let Some(rhs) = binary.rhs() else {
        return false;
    };
    syntax_expression_is_simple_path_or_field(&lhs)
        && syntax_expression_is_simple_path_or_field(&rhs)
}

#[cfg(test)]
fn syntax_expression_is_simple_path_comparison(expression: &SyntaxExpression) -> bool {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return syntax_expression_is_simple_path_comparison(&inner);
    }
    let Some(binary) = expression.as_binary() else {
        return false;
    };
    if !matches!(
        binary.operator(),
        Some(BinaryOp::Less | BinaryOp::LessEqual | BinaryOp::Greater | BinaryOp::GreaterEqual)
    ) {
        return false;
    }
    let Some(lhs) = binary.lhs() else {
        return false;
    };
    let Some(rhs) = binary.rhs() else {
        return false;
    };
    syntax_expression_is_simple_path_or_field(&lhs)
        && syntax_expression_is_simple_path_or_field(&rhs)
}

#[cfg(test)]
fn syntax_expression_is_simple_path_arithmetic(expression: &SyntaxExpression) -> bool {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return syntax_expression_is_simple_path_arithmetic(&inner);
    }
    let Some(binary) = expression.as_binary() else {
        return false;
    };
    if !matches!(
        binary.operator(),
        Some(BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem)
    ) {
        return false;
    }
    let Some(lhs) = binary.lhs() else {
        return false;
    };
    let Some(rhs) = binary.rhs() else {
        return false;
    };
    syntax_expression_is_simple_path_or_field(&lhs)
        && syntax_expression_is_simple_path_or_field(&rhs)
}

#[cfg(test)]
fn syntax_expression_is_simple_path_logical(expression: &SyntaxExpression) -> bool {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return syntax_expression_is_simple_path_logical(&inner);
    }
    let Some(binary) = expression.as_binary() else {
        return false;
    };
    if !matches!(binary.operator(), Some(BinaryOp::And | BinaryOp::Or)) {
        return false;
    }
    let Some(lhs) = binary.lhs() else {
        return false;
    };
    let Some(rhs) = binary.rhs() else {
        return false;
    };
    syntax_expression_is_simple_path_logical_operand(&lhs)
        && syntax_expression_is_simple_path_logical_operand(&rhs)
}

#[cfg(test)]
fn syntax_expression_is_simple_path_logical_operand(expression: &SyntaxExpression) -> bool {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return syntax_expression_is_simple_path_logical_operand(&inner);
    }
    syntax_expression_is_simple_path_or_field(expression)
        || syntax_expression_is_simple_path_binary(expression)
        || syntax_expression_is_simple_path_comparison(expression)
        || syntax_expression_is_simple_path_arithmetic(expression)
        || syntax_expression_is_simple_path_logical(expression)
}

#[cfg(test)]
fn syntax_expression_is_simple_try(expression: &SyntaxExpression) -> bool {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return syntax_expression_is_simple_try(&inner);
    }
    expression
        .as_try()
        .and_then(|try_expression| try_expression.expression())
        .is_some_and(|operand| {
            syntax_expression_is_simple_path(&operand)
                || syntax_expression_is_simple_path_field(&operand)
        })
}

#[cfg(test)]
fn syntax_expression_is_simple_container(expression: &SyntaxExpression) -> bool {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return syntax_expression_is_simple_container(&inner);
    }
    match expression.expression_kind() {
        SyntaxExpressionKind::Array => expression.as_array().is_some_and(|array| {
            let mut has_field_value = false;
            let all_supported = array.expressions().all(|element| {
                has_field_value |= syntax_expression_contains_simple_field_value(&element);
                syntax_expression_is_simple_container_value(&element)
            });
            all_supported && has_field_value
        }),
        SyntaxExpressionKind::Map => expression.as_map().is_some_and(|map| {
            let mut has_field_value = false;
            map.entries().all(|entry| {
                entry.key().is_some_and(|key| {
                    crate::compiler::param_defaults::syntax_map_key_supported(&key)
                }) && entry.value().is_some_and(|value| {
                    has_field_value |= syntax_expression_contains_simple_field_value(&value);
                    syntax_expression_is_simple_container_value(&value)
                })
            }) && has_field_value
        }),
        _ => false,
    }
}

#[cfg(test)]
fn syntax_expression_contains_simple_field_value(expression: &SyntaxExpression) -> bool {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return syntax_expression_contains_simple_field_value(&inner);
    }
    syntax_expression_is_simple_path_field(expression)
        || expression.as_array().is_some_and(|array| {
            array
                .expressions()
                .any(|element| syntax_expression_contains_simple_field_value(&element))
        })
        || expression.as_map().is_some_and(|map| {
            map.entries().any(|entry| {
                entry
                    .value()
                    .is_some_and(|value| syntax_expression_contains_simple_field_value(&value))
            })
        })
}

#[cfg(test)]
fn syntax_expression_is_simple_container_value(expression: &SyntaxExpression) -> bool {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return syntax_expression_is_simple_container_value(&inner);
    }
    syntax_expression_is_simple_literal(expression)
        || syntax_expression_is_simple_interpolated_string(expression)
        || syntax_expression_is_simple_path(expression)
        || syntax_expression_is_simple_path_field(expression)
        || syntax_expression_is_simple_path_unary(expression)
        || syntax_expression_is_simple_path_binary(expression)
        || syntax_expression_is_simple_path_comparison(expression)
        || syntax_expression_is_simple_path_arithmetic(expression)
        || syntax_expression_is_simple_path_logical(expression)
        || syntax_expression_is_simple_try(expression)
        || syntax_expression_is_simple_path_numeric_comparison(expression)
        || syntax_expression_is_simple_path_numeric_equality(expression)
        || syntax_expression_is_simple_path_numeric_arithmetic(expression)
        || syntax_expression_is_simple_container(expression)
}

#[cfg(test)]
fn syntax_expression_is_simple_path_unary(expression: &SyntaxExpression) -> bool {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return syntax_expression_is_simple_path_unary(&inner);
    }
    let Some(unary) = expression.as_unary() else {
        return false;
    };
    if !matches!(unary.operator(), Some(UnaryOp::Not | UnaryOp::Negate)) {
        return false;
    }
    unary
        .expression()
        .is_some_and(|operand| expression_syntax_path_or_field(&operand).is_some())
}

#[cfg(test)]
fn syntax_expression_is_simple_block(expression: &SyntaxExpression) -> bool {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return syntax_expression_is_simple_block(&inner);
    }
    expression
        .as_block()
        .is_some_and(|block| !CompilerBodyPayload::requires_body_block_lookup(&block))
}

#[cfg(test)]
fn syntax_expression_is_statement_block(expression: &SyntaxExpression) -> bool {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return syntax_expression_is_statement_block(&inner);
    }
    expression
        .as_block()
        .is_some_and(|block| !CompilerBodyPayload::requires_body_block_lookup(&block))
}

#[cfg(test)]
fn syntax_expression_is_simple_negated_number(expression: &SyntaxExpression) -> bool {
    expression_syntax_negated_number_literal(expression).is_some()
}

#[cfg(test)]
fn syntax_expression_is_simple_boolean_not(expression: &SyntaxExpression) -> bool {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return syntax_expression_is_simple_boolean_not(&inner);
    }
    let Some(unary) = expression.as_unary() else {
        return false;
    };
    if unary.operator() != Some(UnaryOp::Not) {
        return false;
    }
    let Some(operand) = unary.expression() else {
        return false;
    };
    expression_syntax_bool_literal(&operand).is_some()
}

#[cfg(test)]
fn syntax_expression_is_simple_constant_unary(expression: &SyntaxExpression) -> bool {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return syntax_expression_is_simple_constant_unary(&inner);
    }
    let Some(unary) = expression.as_unary() else {
        return false;
    };
    let Some(operand) = unary.expression() else {
        return false;
    };
    let operand_is_constant = match unary.operator() {
        Some(UnaryOp::Negate) => syntax_expression_is_inline_numeric_constant_operand(&operand),
        Some(UnaryOp::Not) => syntax_expression_is_inline_constant_logical_operand(&operand),
        None => false,
    };
    if !operand_is_constant {
        return false;
    }
    evaluate_syntax_const_expr(SourceId::new(0), expression, &BTreeMap::new())
        .ok()
        .flatten()
        .is_some()
}

#[cfg(test)]
fn syntax_expression_is_simple_constant_comparison(expression: &SyntaxExpression) -> bool {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return syntax_expression_is_simple_constant_comparison(&inner);
    }
    let Some(binary) = expression.as_binary() else {
        return false;
    };
    let Some(lhs) = binary.lhs() else {
        return false;
    };
    let Some(rhs) = binary.rhs() else {
        return false;
    };
    let operands_are_constant = match binary.operator() {
        Some(BinaryOp::Equal | BinaryOp::NotEqual) => {
            syntax_expression_is_inline_comparable_constant_operand(&lhs)
                && syntax_expression_is_inline_comparable_constant_operand(&rhs)
        }
        Some(BinaryOp::Less | BinaryOp::LessEqual | BinaryOp::Greater | BinaryOp::GreaterEqual) => {
            syntax_expression_is_inline_numeric_constant_operand(&lhs)
                && syntax_expression_is_inline_numeric_constant_operand(&rhs)
        }
        _ => false,
    };
    if !operands_are_constant {
        return false;
    }
    evaluate_syntax_const_expr(SourceId::new(0), expression, &BTreeMap::new())
        .ok()
        .flatten()
        .is_some()
}

#[cfg(test)]
fn syntax_expression_is_simple_constant_arithmetic(expression: &SyntaxExpression) -> bool {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return syntax_expression_is_simple_constant_arithmetic(&inner);
    }
    let Some(binary) = expression.as_binary() else {
        return false;
    };
    if !matches!(
        binary.operator(),
        Some(BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem)
    ) {
        return false;
    }
    let Some(lhs) = binary.lhs() else {
        return false;
    };
    let Some(rhs) = binary.rhs() else {
        return false;
    };
    if !syntax_expression_is_inline_constant_arithmetic_operand(&lhs)
        || !syntax_expression_is_inline_constant_arithmetic_operand(&rhs)
    {
        return false;
    }
    evaluate_syntax_const_expr(SourceId::new(0), expression, &BTreeMap::new())
        .ok()
        .flatten()
        .is_some()
}

#[cfg(test)]
fn syntax_expression_is_inline_constant_arithmetic_operand(expression: &SyntaxExpression) -> bool {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return syntax_expression_is_inline_constant_arithmetic_operand(&inner);
    }
    syntax_expression_is_inline_numeric_constant_operand(expression)
        || syntax_expression_is_simple_constant_arithmetic(expression)
}

#[cfg(test)]
fn syntax_expression_is_inline_numeric_constant_operand(expression: &SyntaxExpression) -> bool {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return syntax_expression_is_inline_numeric_constant_operand(&inner);
    }
    expression_syntax_numeric_literal_kind(expression).is_some()
        || syntax_expression_is_simple_negated_number(expression)
        || syntax_expression_is_simple_constant_unary(expression)
        || syntax_expression_is_simple_constant_arithmetic(expression)
}

#[cfg(test)]
fn syntax_expression_is_inline_comparable_constant_operand(expression: &SyntaxExpression) -> bool {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return syntax_expression_is_inline_comparable_constant_operand(&inner);
    }
    expression_syntax_comparable_literal(expression).is_some()
        || syntax_expression_is_simple_negated_number(expression)
        || syntax_expression_is_simple_boolean_not(expression)
        || syntax_expression_is_simple_constant_unary(expression)
        || syntax_expression_is_simple_constant_arithmetic(expression)
        || syntax_expression_is_simple_constant_logical(expression)
        || syntax_expression_is_simple_constant_comparison(expression)
}

#[cfg(test)]
fn syntax_expression_is_simple_constant_logical(expression: &SyntaxExpression) -> bool {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return syntax_expression_is_simple_constant_logical(&inner);
    }
    let Some(binary) = expression.as_binary() else {
        return false;
    };
    if !matches!(binary.operator(), Some(BinaryOp::And | BinaryOp::Or)) {
        return false;
    }
    let Some(lhs) = binary.lhs() else {
        return false;
    };
    let Some(rhs) = binary.rhs() else {
        return false;
    };
    if !syntax_expression_is_inline_constant_logical_operand(&lhs)
        || !syntax_expression_is_inline_constant_logical_operand(&rhs)
    {
        return false;
    }
    evaluate_syntax_const_expr(SourceId::new(0), expression, &BTreeMap::new())
        .ok()
        .flatten()
        .is_some()
}

#[cfg(test)]
fn syntax_expression_is_inline_constant_logical_operand(expression: &SyntaxExpression) -> bool {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return syntax_expression_is_inline_constant_logical_operand(&inner);
    }
    expression_syntax_bool_literal(expression).is_some()
        || syntax_expression_is_simple_boolean_not(expression)
        || syntax_expression_is_simple_constant_unary(expression)
        || syntax_expression_is_simple_constant_comparison(expression)
        || syntax_expression_is_simple_constant_logical(expression)
}

#[cfg(test)]
fn expression_syntax_bool_literal(expression: &SyntaxExpression) -> Option<bool> {
    if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
        return expression_syntax_bool_literal(&inner);
    }
    let literal = expression
        .as_literal()
        .and_then(|literal| literal.literal())?;
    match literal {
        Literal::Bool(value) => Some(value),
        _ => None,
    }
}

#[cfg(test)]
fn expression_syntax_comparable_literal(expression: &SyntaxExpression) -> Option<Literal> {
    let literal = expression_syntax_literal(expression)?;
    match literal {
        Literal::String(_) | Literal::Bytes(_) | Literal::Float(_) | Literal::Integer(_) => {
            Some(literal)
        }
        Literal::Null | Literal::Bool(_) | Literal::Char(_) => Some(literal),
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Eq, PartialEq)]
enum NumericLiteralKind {
    Integer,
    Float,
}

#[cfg(test)]
fn expression_syntax_numeric_literal_kind(
    expression: &SyntaxExpression,
) -> Option<NumericLiteralKind> {
    let literal = expression_syntax_literal(expression)?;
    match literal {
        Literal::Integer(value) if matches!(value.suffix, None | Some(IntegerSuffix::I64)) => {
            Some(NumericLiteralKind::Integer)
        }
        Literal::Float(value) if matches!(value.suffix, None | Some(FloatSuffix::F64)) => {
            Some(NumericLiteralKind::Float)
        }
        _ => None,
    }
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

#[cfg(test)]
fn syntax_expression_is_simple_value(expression: &SyntaxExpression) -> bool {
    syntax_expression_is_simple_literal(expression)
        || syntax_expression_is_simple_interpolated_string(expression)
        || syntax_expression_is_simple_if_value(expression)
        || syntax_expression_is_simple_path(expression)
        || syntax_expression_is_simple_empty_array(expression)
        || syntax_expression_is_simple_block(expression)
        || syntax_expression_is_simple_negated_number(expression)
        || syntax_expression_is_simple_boolean_not(expression)
        || syntax_expression_is_simple_constant_unary(expression)
        || syntax_expression_is_simple_constant_comparison(expression)
        || syntax_expression_is_simple_constant_arithmetic(expression)
        || syntax_expression_is_simple_constant_logical(expression)
        || syntax_expression_is_simple_range(expression)
}
