use vela_syntax::ast::{BinaryOp, IntegerSuffix, Literal, SyntaxExpression, UnaryOp};

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
