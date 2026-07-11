use std::collections::BTreeMap;

use vela_analysis::literals::{
    LiteralError, LiteralPrimitiveContext, LiteralSign, ResolvedLiteralFact,
    resolve_numeric_literal,
};
use vela_common::ScalarValue;
use vela_hir::binding::{BindingMap, BindingResolution};
use vela_hir::body::{
    HirBinaryOp, HirBody, HirBodyRoot, HirExprKind, HirIntegerSuffix, HirLiteral, HirPatternKind,
    HirStmtKind, HirUnaryOp,
};
use vela_hir::ids::{HirBlockId, HirDeclId, HirExprId, HirLocalId};
use vela_mir::MirEvaluatedConstant;

use super::{CompileError, CompileErrorKind, CompileResult};

pub(super) fn evaluate_const_body(
    body: &HirBody,
    bindings: &BindingMap,
    values: &BTreeMap<HirDeclId, MirEvaluatedConstant>,
) -> CompileResult<Option<MirEvaluatedConstant>> {
    let locals = BTreeMap::new();
    match body.root {
        HirBodyRoot::Expr(expression) => {
            evaluate_const_expression(body, bindings, expression, values, &locals)
        }
        HirBodyRoot::Block(block) => evaluate_const_block(body, bindings, block, values, &locals),
        HirBodyRoot::Empty => Ok(None),
    }
}

fn evaluate_const_expression(
    body: &HirBody,
    bindings: &BindingMap,
    expression: HirExprId,
    values: &BTreeMap<HirDeclId, MirEvaluatedConstant>,
    locals: &BTreeMap<HirLocalId, MirEvaluatedConstant>,
) -> CompileResult<Option<MirEvaluatedConstant>> {
    let Some(expression_record) = body.expression(expression) else {
        return Ok(None);
    };
    match &expression_record.kind {
        HirExprKind::Literal(literal) => hir_literal_constant(literal)
            .map_err(|error| error.with_span(expression_record.origin.span)),
        HirExprKind::Path(_) => Ok(match bindings.resolution(expression) {
            Some(BindingResolution::Local(local)) => locals.get(local).cloned(),
            Some(BindingResolution::Declaration(declaration)) => values.get(declaration).cloned(),
            Some(BindingResolution::Import(_) | BindingResolution::QualifiedPath(_)) | None => None,
        }),
        HirExprKind::Paren {
            expression: Some(inner),
        } => evaluate_const_expression(body, bindings, *inner, values, locals),
        HirExprKind::Unary {
            op: Some(op),
            operand: Some(operand),
        } => {
            if *op == HirUnaryOp::Negate
                && let Some(operand) = body.expression(*operand)
                && let HirExprKind::Literal(literal) = &operand.kind
            {
                let value = evaluated_negated_literal_constant(literal)
                    .map_err(|error| error.with_span(operand.origin.span))?;
                if let Some(value) = value {
                    return Ok(Some(value));
                }
            }
            let Some(value) = evaluate_const_expression(body, bindings, *operand, values, locals)?
            else {
                return Ok(None);
            };
            Ok(evaluate_unary_const(*op, value))
        }
        HirExprKind::Binary {
            op: Some(op),
            lhs: Some(lhs),
            rhs: Some(rhs),
        } => {
            let Some(left) = evaluate_const_expression(body, bindings, *lhs, values, locals)?
            else {
                return Ok(None);
            };
            let Some(right) = evaluate_const_expression(body, bindings, *rhs, values, locals)?
            else {
                return Ok(None);
            };
            Ok(evaluate_binary_const(*op, left, right))
        }
        HirExprKind::Array { elements } => elements
            .iter()
            .map(|element| evaluate_const_expression(body, bindings, *element, values, locals))
            .collect::<CompileResult<Option<Vec<_>>>>()
            .map(|elements| elements.map(MirEvaluatedConstant::Array)),
        HirExprKind::Map { entries } => entries
            .iter()
            .map(|entry| {
                let (Some(key), Some(value)) = (entry.logical_key.clone(), entry.value) else {
                    return Ok(None);
                };
                let Some(value) = evaluate_const_expression(body, bindings, value, values, locals)?
                else {
                    return Ok(None);
                };
                Ok(Some((key, value)))
            })
            .collect::<CompileResult<Option<Vec<_>>>>()
            .map(|entries| entries.map(MirEvaluatedConstant::Map)),
        HirExprKind::Block { block } => {
            evaluate_const_block(body, bindings, *block, values, locals)
        }
        HirExprKind::Paren { expression: None }
        | HirExprKind::Unary { .. }
        | HirExprKind::Binary { .. }
        | HirExprKind::Assign { .. }
        | HirExprKind::Unit
        | HirExprKind::Tuple { .. }
        | HirExprKind::Field(_)
        | HirExprKind::Call(_)
        | HirExprKind::Index(_)
        | HirExprKind::Try { .. }
        | HirExprKind::Record { .. }
        | HirExprKind::Lambda { .. }
        | HirExprKind::If(_)
        | HirExprKind::Match(_)
        | HirExprKind::Missing => Ok(None),
    }
}

fn evaluate_const_block(
    body: &HirBody,
    bindings: &BindingMap,
    block: HirBlockId,
    values: &BTreeMap<HirDeclId, MirEvaluatedConstant>,
    locals: &BTreeMap<HirLocalId, MirEvaluatedConstant>,
) -> CompileResult<Option<MirEvaluatedConstant>> {
    let Some(block) = body.blocks.get(&block) else {
        return Ok(None);
    };
    let mut locals = locals.clone();
    let mut tail_value = None;
    for statement in &block.statements {
        let Some(statement) = body.statements.get(statement) else {
            return Ok(None);
        };
        match &statement.kind {
            HirStmtKind::Let {
                pattern: Some(pattern),
                initializer: Some(initializer),
                ..
            } => {
                let Some(HirPatternKind::Binding { local: Some(local) }) =
                    body.patterns.get(pattern).map(|pattern| &pattern.kind)
                else {
                    return Ok(None);
                };
                let Some(value) =
                    evaluate_const_expression(body, bindings, *initializer, values, &locals)?
                else {
                    return Ok(None);
                };
                locals.insert(*local, value);
                tail_value = None;
            }
            HirStmtKind::Return { value } => {
                let Some(value) = value else {
                    return Ok(Some(MirEvaluatedConstant::Unit));
                };
                return evaluate_const_expression(body, bindings, *value, values, &locals);
            }
            HirStmtKind::Expr {
                expression: Some(expression),
                terminated,
            } => {
                tail_value = if *terminated {
                    None
                } else {
                    evaluate_const_expression(body, bindings, *expression, values, &locals)?
                };
            }
            HirStmtKind::Block(block) => {
                tail_value = evaluate_const_block(body, bindings, *block, values, &locals)?;
            }
            HirStmtKind::Let { .. }
            | HirStmtKind::Expr {
                expression: None, ..
            }
            | HirStmtKind::Break
            | HirStmtKind::Continue
            | HirStmtKind::For { .. }
            | HirStmtKind::If(_)
            | HirStmtKind::Match(_) => return Ok(None),
        }
    }
    Ok(tail_value)
}

fn hir_literal_constant(literal: &HirLiteral) -> CompileResult<Option<MirEvaluatedConstant>> {
    Ok(match literal {
        HirLiteral::Bool(value) => Some(MirEvaluatedConstant::Bool(*value)),
        HirLiteral::Char(value) => Some(MirEvaluatedConstant::Char(*value)),
        HirLiteral::Integer(_) | HirLiteral::Float(_) => {
            Some(MirEvaluatedConstant::Scalar(resolved_scalar(
                literal,
                LiteralPrimitiveContext::Default,
                LiteralSign::Positive,
            )?))
        }
        HirLiteral::String(value) => Some(MirEvaluatedConstant::String(value.clone())),
        HirLiteral::Bytes(value) => Some(MirEvaluatedConstant::Bytes(value.clone())),
        HirLiteral::Interpolated { .. } | HirLiteral::Invalid { .. } => None,
    })
}

fn evaluated_negated_literal_constant(
    literal: &HirLiteral,
) -> CompileResult<Option<MirEvaluatedConstant>> {
    match literal {
        HirLiteral::Integer(value) => {
            if matches!(
                value.suffix,
                Some(
                    HirIntegerSuffix::U8
                        | HirIntegerSuffix::U16
                        | HirIntegerSuffix::U32
                        | HirIntegerSuffix::U64
                )
            ) {
                return Ok(None);
            }
            resolved_scalar(
                literal,
                LiteralPrimitiveContext::Default,
                LiteralSign::Negated,
            )
            .map(|value| Some(MirEvaluatedConstant::Scalar(value)))
        }
        HirLiteral::Float(_) => resolved_scalar(
            literal,
            LiteralPrimitiveContext::Default,
            LiteralSign::Negated,
        )
        .map(|value| Some(MirEvaluatedConstant::Scalar(value))),
        _ => Ok(None),
    }
}

fn evaluate_unary_const(
    op: HirUnaryOp,
    value: MirEvaluatedConstant,
) -> Option<MirEvaluatedConstant> {
    match (op, value) {
        (HirUnaryOp::Negate, MirEvaluatedConstant::Scalar(ScalarValue::I8(value))) => value
            .checked_neg()
            .map(|value| MirEvaluatedConstant::Scalar(value.into())),
        (HirUnaryOp::Negate, MirEvaluatedConstant::Scalar(ScalarValue::I16(value))) => value
            .checked_neg()
            .map(|value| MirEvaluatedConstant::Scalar(value.into())),
        (HirUnaryOp::Negate, MirEvaluatedConstant::Scalar(ScalarValue::I32(value))) => value
            .checked_neg()
            .map(|value| MirEvaluatedConstant::Scalar(value.into())),
        (HirUnaryOp::Negate, MirEvaluatedConstant::Scalar(ScalarValue::I64(value))) => value
            .checked_neg()
            .map(|value| MirEvaluatedConstant::Scalar(value.into())),
        (HirUnaryOp::Negate, MirEvaluatedConstant::Scalar(ScalarValue::F32(value))) => {
            Some(MirEvaluatedConstant::Scalar(ScalarValue::F32(-value)))
        }
        (HirUnaryOp::Negate, MirEvaluatedConstant::Scalar(ScalarValue::F64(value))) => {
            Some(MirEvaluatedConstant::Scalar(ScalarValue::F64(-value)))
        }
        (HirUnaryOp::Not, MirEvaluatedConstant::Bool(value)) => {
            Some(MirEvaluatedConstant::Bool(!value))
        }
        _ => None,
    }
}

fn evaluate_binary_const(
    op: HirBinaryOp,
    left: MirEvaluatedConstant,
    right: MirEvaluatedConstant,
) -> Option<MirEvaluatedConstant> {
    match op {
        HirBinaryOp::Add => evaluate_numeric_const(left, right, i64::checked_add, |a, b| a + b),
        HirBinaryOp::Sub => evaluate_numeric_const(left, right, i64::checked_sub, |a, b| a - b),
        HirBinaryOp::Mul => evaluate_numeric_const(left, right, i64::checked_mul, |a, b| a * b),
        HirBinaryOp::Div => match (left, right) {
            (
                MirEvaluatedConstant::Scalar(vela_common::ScalarValue::I64(_)),
                MirEvaluatedConstant::Scalar(vela_common::ScalarValue::I64(0)),
            ) => None,
            (
                MirEvaluatedConstant::Scalar(vela_common::ScalarValue::I64(left)),
                MirEvaluatedConstant::Scalar(vela_common::ScalarValue::I64(right)),
            ) => left
                .checked_div(right)
                .map(|value| MirEvaluatedConstant::Scalar(value.into())),
            (
                MirEvaluatedConstant::Scalar(vela_common::ScalarValue::F64(_)),
                MirEvaluatedConstant::Scalar(vela_common::ScalarValue::F64(0.0)),
            ) => None,
            (
                MirEvaluatedConstant::Scalar(vela_common::ScalarValue::F64(left)),
                MirEvaluatedConstant::Scalar(vela_common::ScalarValue::F64(right)),
            ) => Some(MirEvaluatedConstant::Scalar(vela_common::ScalarValue::F64(
                left / right,
            ))),
            _ => None,
        },
        HirBinaryOp::Rem => match (left, right) {
            (
                MirEvaluatedConstant::Scalar(vela_common::ScalarValue::I64(_)),
                MirEvaluatedConstant::Scalar(vela_common::ScalarValue::I64(0)),
            ) => None,
            (
                MirEvaluatedConstant::Scalar(vela_common::ScalarValue::I64(left)),
                MirEvaluatedConstant::Scalar(vela_common::ScalarValue::I64(right)),
            ) => left
                .checked_rem(right)
                .map(|value| MirEvaluatedConstant::Scalar(value.into())),
            (
                MirEvaluatedConstant::Scalar(vela_common::ScalarValue::F64(_)),
                MirEvaluatedConstant::Scalar(vela_common::ScalarValue::F64(0.0)),
            ) => None,
            (
                MirEvaluatedConstant::Scalar(vela_common::ScalarValue::F64(left)),
                MirEvaluatedConstant::Scalar(vela_common::ScalarValue::F64(right)),
            ) => Some(MirEvaluatedConstant::Scalar(vela_common::ScalarValue::F64(
                left % right,
            ))),
            _ => None,
        },
        HirBinaryOp::Equal => {
            evaluate_equality_const(&left, &right).map(MirEvaluatedConstant::Bool)
        }
        HirBinaryOp::NotEqual => {
            evaluate_equality_const(&left, &right).map(|equal| MirEvaluatedConstant::Bool(!equal))
        }
        HirBinaryOp::IdentityEqual | HirBinaryOp::IdentityNotEqual => None,
        HirBinaryOp::Less => evaluate_numeric_compare_const(left, right, |a, b| a < b),
        HirBinaryOp::LessEqual => evaluate_numeric_compare_const(left, right, |a, b| a <= b),
        HirBinaryOp::Greater => evaluate_numeric_compare_const(left, right, |a, b| a > b),
        HirBinaryOp::GreaterEqual => evaluate_numeric_compare_const(left, right, |a, b| a >= b),
        HirBinaryOp::And => match (left, right) {
            (MirEvaluatedConstant::Bool(left), MirEvaluatedConstant::Bool(right)) => {
                Some(MirEvaluatedConstant::Bool(left && right))
            }
            _ => None,
        },
        HirBinaryOp::Or => match (left, right) {
            (MirEvaluatedConstant::Bool(left), MirEvaluatedConstant::Bool(right)) => {
                Some(MirEvaluatedConstant::Bool(left || right))
            }
            _ => None,
        },
        HirBinaryOp::Range | HirBinaryOp::RangeInclusive => None,
    }
}

fn evaluate_numeric_const(
    left: MirEvaluatedConstant,
    right: MirEvaluatedConstant,
    int_op: impl FnOnce(i64, i64) -> Option<i64>,
    float_op: impl FnOnce(f64, f64) -> f64,
) -> Option<MirEvaluatedConstant> {
    match (left, right) {
        (
            MirEvaluatedConstant::Scalar(vela_common::ScalarValue::I64(left)),
            MirEvaluatedConstant::Scalar(vela_common::ScalarValue::I64(right)),
        ) => int_op(left, right).map(|value| MirEvaluatedConstant::Scalar(value.into())),
        (
            MirEvaluatedConstant::Scalar(vela_common::ScalarValue::F64(left)),
            MirEvaluatedConstant::Scalar(vela_common::ScalarValue::F64(right)),
        ) => Some(MirEvaluatedConstant::Scalar(ScalarValue::F64(float_op(
            left, right,
        )))),
        _ => None,
    }
}

fn evaluate_equality_const(
    left: &MirEvaluatedConstant,
    right: &MirEvaluatedConstant,
) -> Option<bool> {
    match (left, right) {
        (MirEvaluatedConstant::Array(_) | MirEvaluatedConstant::Map(_), _)
        | (_, MirEvaluatedConstant::Array(_) | MirEvaluatedConstant::Map(_)) => None,
        (MirEvaluatedConstant::Unit, MirEvaluatedConstant::Unit) => Some(true),
        (MirEvaluatedConstant::Bool(left), MirEvaluatedConstant::Bool(right)) => {
            Some(left == right)
        }
        (MirEvaluatedConstant::Char(left), MirEvaluatedConstant::Char(right)) => {
            Some(left == right)
        }
        (MirEvaluatedConstant::Scalar(left), MirEvaluatedConstant::Scalar(right)) => {
            Some(left == right)
        }
        (MirEvaluatedConstant::String(left), MirEvaluatedConstant::String(right)) => {
            Some(left == right)
        }
        (MirEvaluatedConstant::Bytes(left), MirEvaluatedConstant::Bytes(right)) => {
            Some(left == right)
        }
        _ => Some(false),
    }
}

fn evaluate_numeric_compare_const(
    left: MirEvaluatedConstant,
    right: MirEvaluatedConstant,
    op: impl FnOnce(f64, f64) -> bool,
) -> Option<MirEvaluatedConstant> {
    match (left, right) {
        (
            MirEvaluatedConstant::Scalar(vela_common::ScalarValue::I64(left)),
            MirEvaluatedConstant::Scalar(vela_common::ScalarValue::I64(right)),
        ) => Some(MirEvaluatedConstant::Bool(op(left as f64, right as f64))),
        (
            MirEvaluatedConstant::Scalar(vela_common::ScalarValue::F64(left)),
            MirEvaluatedConstant::Scalar(vela_common::ScalarValue::F64(right)),
        ) => Some(MirEvaluatedConstant::Bool(op(left, right))),
        _ => None,
    }
}

fn resolved_scalar(
    literal: &HirLiteral,
    context: LiteralPrimitiveContext,
    sign: LiteralSign,
) -> CompileResult<ScalarValue> {
    let result = resolve_numeric_literal(literal, context, sign)
        .expect("resolved_scalar is only called for numeric literals")
        .map_err(literal_compile_error)?;
    match result {
        ResolvedLiteralFact::Scalar(value) => Ok(value.value()),
        ResolvedLiteralFact::Deferred(_) => unreachable!("scalar resolution never defers"),
    }
}

fn literal_compile_error(error: LiteralError) -> CompileError {
    let kind = match error.kind() {
        vela_analysis::literals::NumericLiteralKind::Integer => {
            CompileErrorKind::InvalidIntLiteral {
                literal: error.spelling().to_owned(),
                error: error.detail().to_owned(),
            }
        }
        vela_analysis::literals::NumericLiteralKind::Float => {
            CompileErrorKind::InvalidFloatLiteral {
                literal: error.spelling().to_owned(),
                error: error.detail().to_owned(),
            }
        }
    };
    CompileError::new(kind)
}
