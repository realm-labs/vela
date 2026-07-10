use vela_common::Diagnostic;
use vela_hir::body::{HirBinaryOp, HirBody, HirExpr, HirStmt};

use super::{
    ArrayOrderingCapabilityFact, CapabilityFact, LoopControlFact, LoopControlKind,
    LoopControlPlacement, OperatorCapabilityFact,
};

pub(super) fn operator(
    body: &HirBody,
    expression: &HirExpr,
    fact: &OperatorCapabilityFact,
) -> Option<Diagnostic> {
    match fact {
        OperatorCapabilityFact::ReferenceIdentity {
            operator,
            lhs_expression,
            lhs,
            rhs_expression,
            rhs,
        } => [
            ("left", *lhs_expression, lhs),
            ("right", *rhs_expression, rhs),
        ]
        .into_iter()
        .find_map(|(side, operand, capability)| {
            let CapabilityFact::Unsupported { type_name } = capability else {
                return None;
            };
            let operand_span = body
                .expression(operand)
                .map_or(expression.origin.span, |operand| operand.origin.span);
            Some(
                Diagnostic::error(format!(
                    "`{}` requires reference identity operands, but the {side} operand has type `{type_name}`",
                    binary_operator_name(*operator)
                ))
                .with_code("compiler::invalid_identity_comparison")
                .with_span(expression.origin.span)
                .with_label(
                    expression.origin.span,
                    "identity comparison requires reference operands",
                )
                .with_label(
                    operand_span,
                    format!("{side} operand is statically `{type_name}`"),
                ),
            )
        }),
        OperatorCapabilityFact::ComparisonTrait {
            operator,
            required,
            capability: CapabilityFact::Unsupported { type_name },
            ..
        } => {
            let operator = binary_operator_name(*operator);
            let required = required.source_name();
            Some(
                Diagnostic::error(format!(
                    "`{type_name}` does not implement `{required}` for `{operator}`"
                ))
                .with_code("compiler::missing_comparison_trait")
                .with_span(expression.origin.span)
                .with_label(
                    expression.origin.span,
                    format!("static `{operator}` comparison requires `{required}`"),
                )
                .with_label(
                    expression.origin.span,
                    format!("add `impl {required} for {type_name}` or make the value dynamic"),
                ),
            )
        }
        OperatorCapabilityFact::ComparisonTrait { .. } => None,
    }
}

pub(super) fn array_ordering(
    expression: &HirExpr,
    fact: &ArrayOrderingCapabilityFact,
) -> Option<Diagnostic> {
    let CapabilityFact::Unsupported { type_name } = &fact.capability else {
        return None;
    };
    let method = fact.method.source_name();
    let value_kind = fact.value_kind.source_name();
    Some(
        Diagnostic::error(format!(
            "`Array.{method}` requires an `Ord` {value_kind}, but `{type_name}` does not implement `Ord`"
        ))
        .with_code("compiler::missing_ord_for_array_ordering")
        .with_span(expression.origin.span)
        .with_label(
            expression.origin.span,
            format!("static `Array.{method}` requires `Ord`"),
        )
        .with_label(
            expression.origin.span,
            format!("add `impl Ord for {type_name}` or use a dynamic value"),
        ),
    )
}

pub(super) fn loop_control(statement: &HirStmt, fact: LoopControlFact) -> Option<Diagnostic> {
    if fact.placement != LoopControlPlacement::OutsideLoop {
        return None;
    }
    let (keyword, code) = match fact.kind {
        LoopControlKind::Break => ("break", "analysis::break_outside_loop"),
        LoopControlKind::Continue => ("continue", "analysis::continue_outside_loop"),
    };
    Some(
        Diagnostic::error(format!("{keyword} outside loop"))
            .with_code(code)
            .with_span(statement.origin.span),
    )
}

fn binary_operator_name(operator: HirBinaryOp) -> &'static str {
    match operator {
        HirBinaryOp::Add => "+",
        HirBinaryOp::Sub => "-",
        HirBinaryOp::Mul => "*",
        HirBinaryOp::Div => "/",
        HirBinaryOp::Rem => "%",
        HirBinaryOp::Equal => "==",
        HirBinaryOp::NotEqual => "!=",
        HirBinaryOp::IdentityEqual => "===",
        HirBinaryOp::IdentityNotEqual => "!==",
        HirBinaryOp::Less => "<",
        HirBinaryOp::LessEqual => "<=",
        HirBinaryOp::Greater => ">",
        HirBinaryOp::GreaterEqual => ">=",
        HirBinaryOp::Range => "..",
        HirBinaryOp::RangeInclusive => "..=",
        HirBinaryOp::Or => "||",
        HirBinaryOp::And => "&&",
    }
}
