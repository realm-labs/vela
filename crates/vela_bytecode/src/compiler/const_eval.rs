use std::collections::BTreeMap;

use vela_analysis::literals::{
    DeferredNumericLiteral, LiteralError, LiteralPrimitiveContext, LiteralSign,
    ResolvedLiteralFact, resolve_numeric_literal,
};
use vela_common::{PrimitiveTag, ScalarValue};
use vela_hir::binding::{BindingMap, BindingResolution};
use vela_hir::body::{
    HirBinaryOp, HirBody, HirBodyRoot, HirExprKind, HirIntegerSuffix, HirLiteral, HirPatternKind,
    HirStmtKind, HirUnaryOp,
};
use vela_hir::ids::{HirBlockId, HirDeclId, HirExprId, HirLocalId};

use crate::Constant;

use super::{CompileError, CompileErrorKind, CompileResult};

pub(super) fn compile_literal_constant(literal: &HirLiteral) -> CompileResult<Constant> {
    Ok(match literal {
        HirLiteral::Bool(value) => Constant::Bool(*value),
        HirLiteral::Char(value) => Constant::Char(*value),
        HirLiteral::Integer(_) | HirLiteral::Float(_) => Constant::Scalar(resolved_scalar(
            literal,
            LiteralPrimitiveContext::Default,
            LiteralSign::Positive,
        )?),
        HirLiteral::String(value) => Constant::String(value.clone()),
        HirLiteral::Bytes(value) => Constant::Bytes(value.clone()),
        HirLiteral::Interpolated { .. } | HirLiteral::Invalid { .. } => {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "HIR literal",
            )));
        }
    })
}

pub(super) fn compile_literal_constant_for_type(
    literal: &HirLiteral,
    expected: PrimitiveTag,
) -> CompileResult<Option<Constant>> {
    match literal {
        HirLiteral::Integer(value)
            if value.suffix.is_none()
                && vela_analysis::literals::NumericLiteralKind::Integer
                    .accepts_primitive(expected) =>
        {
            resolved_scalar(
                literal,
                LiteralPrimitiveContext::Expected(expected),
                LiteralSign::Positive,
            )
            .map(|value| Some(Constant::Scalar(value)))
        }
        HirLiteral::Float(value)
            if value.suffix.is_none()
                && vela_analysis::literals::NumericLiteralKind::Float
                    .accepts_primitive(expected) =>
        {
            resolved_scalar(
                literal,
                LiteralPrimitiveContext::Expected(expected),
                LiteralSign::Positive,
            )
            .map(|value| Some(Constant::Scalar(value)))
        }
        _ => Ok(None),
    }
}

pub(super) fn compile_negated_literal_constant(
    literal: &HirLiteral,
) -> CompileResult<Option<Constant>> {
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
            .map(|value| Some(Constant::Scalar(value)))
        }
        HirLiteral::Float(_) => resolved_scalar(
            literal,
            LiteralPrimitiveContext::Default,
            LiteralSign::Negated,
        )
        .map(|value| Some(Constant::Scalar(value))),
        _ => Ok(None),
    }
}

pub(super) fn compile_negated_literal_constant_for_type(
    literal: &HirLiteral,
    expected: PrimitiveTag,
) -> CompileResult<Option<Constant>> {
    let compatible = match literal {
        HirLiteral::Integer(value) => {
            value.suffix.is_none()
                && expected
                    .numeric_tag()
                    .is_some_and(|tag| tag.is_signed_integer())
        }
        HirLiteral::Float(value) => {
            value.suffix.is_none()
                && vela_analysis::literals::NumericLiteralKind::Float.accepts_primitive(expected)
        }
        _ => false,
    };
    if !compatible {
        return Ok(None);
    }
    resolved_scalar(
        literal,
        LiteralPrimitiveContext::Expected(expected),
        LiteralSign::Negated,
    )
    .map(|value| Some(Constant::Scalar(value)))
}

pub(super) fn validate_deferred_numeric_literal(
    literal: &HirLiteral,
) -> CompileResult<DeferredNumericLiteral> {
    let resolved = resolve_numeric_literal(
        literal,
        LiteralPrimitiveContext::DeferredDynamic,
        LiteralSign::Positive,
    )
    .expect("deferred literal validation requires a numeric literal")
    .map_err(literal_compile_error)?;
    match resolved {
        ResolvedLiteralFact::Deferred(literal) => Ok(literal),
        ResolvedLiteralFact::Scalar(_) => {
            unreachable!("unsuffixed dynamic numeric literals always defer")
        }
    }
}

pub(super) fn evaluate_const_body(
    body: &HirBody,
    bindings: &BindingMap,
    values: &BTreeMap<HirDeclId, Constant>,
) -> CompileResult<Option<Constant>> {
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
    values: &BTreeMap<HirDeclId, Constant>,
    locals: &BTreeMap<HirLocalId, Constant>,
) -> CompileResult<Option<Constant>> {
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
                let value = compile_negated_literal_constant(literal)
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
            .map(|elements| elements.map(Constant::Array)),
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
            .map(|entries| entries.map(Constant::Map)),
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
    values: &BTreeMap<HirDeclId, Constant>,
    locals: &BTreeMap<HirLocalId, Constant>,
) -> CompileResult<Option<Constant>> {
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
                    return Ok(Some(Constant::Unit));
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

fn hir_literal_constant(literal: &HirLiteral) -> CompileResult<Option<Constant>> {
    match literal {
        HirLiteral::Interpolated { .. } | HirLiteral::Invalid { .. } => Ok(None),
        literal => compile_literal_constant(literal).map(Some),
    }
}

fn evaluate_unary_const(op: HirUnaryOp, value: Constant) -> Option<Constant> {
    match (op, value) {
        (HirUnaryOp::Negate, Constant::Scalar(ScalarValue::I8(value))) => value
            .checked_neg()
            .map(|value| Constant::Scalar(value.into())),
        (HirUnaryOp::Negate, Constant::Scalar(ScalarValue::I16(value))) => value
            .checked_neg()
            .map(|value| Constant::Scalar(value.into())),
        (HirUnaryOp::Negate, Constant::Scalar(ScalarValue::I32(value))) => value
            .checked_neg()
            .map(|value| Constant::Scalar(value.into())),
        (HirUnaryOp::Negate, Constant::Scalar(ScalarValue::I64(value))) => value
            .checked_neg()
            .map(|value| Constant::Scalar(value.into())),
        (HirUnaryOp::Negate, Constant::Scalar(ScalarValue::F32(value))) => {
            Some(Constant::Scalar(ScalarValue::F32(-value)))
        }
        (HirUnaryOp::Negate, Constant::Scalar(ScalarValue::F64(value))) => {
            Some(Constant::Scalar(ScalarValue::F64(-value)))
        }
        (HirUnaryOp::Not, Constant::Bool(value)) => Some(Constant::Bool(!value)),
        _ => None,
    }
}

fn evaluate_binary_const(op: HirBinaryOp, left: Constant, right: Constant) -> Option<Constant> {
    match op {
        HirBinaryOp::Add => evaluate_numeric_const(left, right, i64::checked_add, |a, b| a + b),
        HirBinaryOp::Sub => evaluate_numeric_const(left, right, i64::checked_sub, |a, b| a - b),
        HirBinaryOp::Mul => evaluate_numeric_const(left, right, i64::checked_mul, |a, b| a * b),
        HirBinaryOp::Div => match (left, right) {
            (
                Constant::Scalar(vela_common::ScalarValue::I64(_)),
                Constant::Scalar(vela_common::ScalarValue::I64(0)),
            ) => None,
            (
                Constant::Scalar(vela_common::ScalarValue::I64(left)),
                Constant::Scalar(vela_common::ScalarValue::I64(right)),
            ) => left.checked_div(right).map(Constant::i64),
            (
                Constant::Scalar(vela_common::ScalarValue::F64(_)),
                Constant::Scalar(vela_common::ScalarValue::F64(0.0)),
            ) => None,
            (
                Constant::Scalar(vela_common::ScalarValue::F64(left)),
                Constant::Scalar(vela_common::ScalarValue::F64(right)),
            ) => Some(Constant::Scalar(vela_common::ScalarValue::F64(
                left / right,
            ))),
            _ => None,
        },
        HirBinaryOp::Rem => match (left, right) {
            (
                Constant::Scalar(vela_common::ScalarValue::I64(_)),
                Constant::Scalar(vela_common::ScalarValue::I64(0)),
            ) => None,
            (
                Constant::Scalar(vela_common::ScalarValue::I64(left)),
                Constant::Scalar(vela_common::ScalarValue::I64(right)),
            ) => left.checked_rem(right).map(Constant::i64),
            (
                Constant::Scalar(vela_common::ScalarValue::F64(_)),
                Constant::Scalar(vela_common::ScalarValue::F64(0.0)),
            ) => None,
            (
                Constant::Scalar(vela_common::ScalarValue::F64(left)),
                Constant::Scalar(vela_common::ScalarValue::F64(right)),
            ) => Some(Constant::Scalar(vela_common::ScalarValue::F64(
                left % right,
            ))),
            _ => None,
        },
        HirBinaryOp::Equal => evaluate_equality_const(&left, &right).map(Constant::Bool),
        HirBinaryOp::NotEqual => {
            evaluate_equality_const(&left, &right).map(|equal| Constant::Bool(!equal))
        }
        HirBinaryOp::IdentityEqual | HirBinaryOp::IdentityNotEqual => None,
        HirBinaryOp::Less => evaluate_numeric_compare_const(left, right, |a, b| a < b),
        HirBinaryOp::LessEqual => evaluate_numeric_compare_const(left, right, |a, b| a <= b),
        HirBinaryOp::Greater => evaluate_numeric_compare_const(left, right, |a, b| a > b),
        HirBinaryOp::GreaterEqual => evaluate_numeric_compare_const(left, right, |a, b| a >= b),
        HirBinaryOp::And => match (left, right) {
            (Constant::Bool(left), Constant::Bool(right)) => Some(Constant::Bool(left && right)),
            _ => None,
        },
        HirBinaryOp::Or => match (left, right) {
            (Constant::Bool(left), Constant::Bool(right)) => Some(Constant::Bool(left || right)),
            _ => None,
        },
        HirBinaryOp::Range | HirBinaryOp::RangeInclusive => None,
    }
}

fn evaluate_numeric_const(
    left: Constant,
    right: Constant,
    int_op: impl FnOnce(i64, i64) -> Option<i64>,
    float_op: impl FnOnce(f64, f64) -> f64,
) -> Option<Constant> {
    match (left, right) {
        (
            Constant::Scalar(vela_common::ScalarValue::I64(left)),
            Constant::Scalar(vela_common::ScalarValue::I64(right)),
        ) => int_op(left, right).map(Constant::i64),
        (
            Constant::Scalar(vela_common::ScalarValue::F64(left)),
            Constant::Scalar(vela_common::ScalarValue::F64(right)),
        ) => Some(Constant::f64(float_op(left, right))),
        _ => None,
    }
}

fn evaluate_equality_const(left: &Constant, right: &Constant) -> Option<bool> {
    match (left, right) {
        (Constant::Array(_) | Constant::Map(_), _) | (_, Constant::Array(_) | Constant::Map(_)) => {
            None
        }
        (Constant::Unit, Constant::Unit) => Some(true),
        (Constant::Bool(left), Constant::Bool(right)) => Some(left == right),
        (Constant::Char(left), Constant::Char(right)) => Some(left == right),
        (Constant::Scalar(left), Constant::Scalar(right)) => Some(left == right),
        (Constant::String(left), Constant::String(right)) => Some(left == right),
        (Constant::Bytes(left), Constant::Bytes(right)) => Some(left == right),
        _ => Some(false),
    }
}

fn evaluate_numeric_compare_const(
    left: Constant,
    right: Constant,
    op: impl FnOnce(f64, f64) -> bool,
) -> Option<Constant> {
    match (left, right) {
        (
            Constant::Scalar(vela_common::ScalarValue::I64(left)),
            Constant::Scalar(vela_common::ScalarValue::I64(right)),
        ) => Some(Constant::Bool(op(left as f64, right as f64))),
        (
            Constant::Scalar(vela_common::ScalarValue::F64(left)),
            Constant::Scalar(vela_common::ScalarValue::F64(right)),
        ) => Some(Constant::Bool(op(left, right))),
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
