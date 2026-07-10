use std::collections::BTreeMap;
use std::num::{ParseFloatError, ParseIntError};

use vela_common::{PrimitiveTag, ScalarValue};
use vela_hir::binding::{BindingMap, BindingResolution};
use vela_hir::body::{
    HirBinaryOp, HirBody, HirBodyRoot, HirExprKind, HirFloatLiteral, HirFloatSuffix, HirIntRadix,
    HirIntegerLiteral, HirIntegerSuffix, HirLiteral, HirPatternKind, HirStmtKind, HirUnaryOp,
};
use vela_hir::ids::{HirBlockId, HirDeclId, HirExprId, HirLocalId};

use crate::Constant;

use super::{CompileError, CompileErrorKind, CompileResult};

pub(super) fn compile_literal_constant(literal: &HirLiteral) -> CompileResult<Constant> {
    Ok(match literal {
        HirLiteral::Bool(value) => Constant::Bool(*value),
        HirLiteral::Char(value) => Constant::Char(*value),
        HirLiteral::Integer(value) => Constant::Scalar(parse_i64eger_scalar(value)?),
        HirLiteral::Float(value) => Constant::Scalar(parse_f64_scalar(value)?),
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
        HirLiteral::Integer(value) if value.suffix.is_none() && is_integer_tag(expected) => {
            parse_i64eger_scalar_as(value, expected).map(|value| Some(Constant::Scalar(value)))
        }
        HirLiteral::Float(value) if value.suffix.is_none() && is_float_tag(expected) => {
            parse_f64_scalar_as(value, expected).map(|value| Some(Constant::Scalar(value)))
        }
        _ => Ok(None),
    }
}

pub(super) fn compile_negated_literal_constant(
    literal: &HirLiteral,
) -> CompileResult<Option<Constant>> {
    match literal {
        HirLiteral::Integer(value) => {
            parse_negated_integer_scalar(value).map(|value| value.map(Constant::Scalar))
        }
        HirLiteral::Float(value) => Ok(Some(Constant::Scalar(negate_float_scalar(
            parse_f64_scalar(value)?,
        )))),
        _ => Ok(None),
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
                && let Some(HirExprKind::Literal(literal)) =
                    body.expression(*operand).map(|expression| &expression.kind)
                && let Some(value) = compile_negated_literal_constant(literal)?
            {
                return Ok(Some(value));
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
                let (Some(key), Some(value)) = (entry.key, entry.value) else {
                    return Ok(None);
                };
                let Some(key) = const_map_key_name(body, key) else {
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

fn const_map_key_name(body: &HirBody, expression: HirExprId) -> Option<String> {
    match &body.expression(expression)?.kind {
        HirExprKind::Literal(HirLiteral::String(value)) => Some(value.clone()),
        HirExprKind::Literal(HirLiteral::Integer(value)) => Some(integer_text(value)),
        HirExprKind::Literal(HirLiteral::Float(value)) => Some(float_text(value)),
        HirExprKind::Path(path) => body
            .paths
            .get(path)
            .map(|path| path.path.join("::"))
            .filter(|path| !path.is_empty()),
        _ => None,
    }
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

fn integer_text(value: &HirIntegerLiteral) -> String {
    let suffix = match value.suffix {
        Some(HirIntegerSuffix::I8) => "i8",
        Some(HirIntegerSuffix::I16) => "i16",
        Some(HirIntegerSuffix::I32) => "i32",
        Some(HirIntegerSuffix::I64) => "i64",
        Some(HirIntegerSuffix::U8) => "u8",
        Some(HirIntegerSuffix::U16) => "u16",
        Some(HirIntegerSuffix::U32) => "u32",
        Some(HirIntegerSuffix::U64) => "u64",
        None => "",
    };
    format!("{}{suffix}", value.text)
}

fn int_radix_base(radix: HirIntRadix) -> u32 {
    match radix {
        HirIntRadix::Binary => 2,
        HirIntRadix::Decimal => 10,
        HirIntRadix::Hex => 16,
    }
}

fn float_text(value: &HirFloatLiteral) -> String {
    let suffix = match value.suffix {
        Some(HirFloatSuffix::F32) => "f32",
        Some(HirFloatSuffix::F64) => "f64",
        None => "",
    };
    format!("{}{suffix}", value.text)
}

fn parse_i64eger_scalar(value: &HirIntegerLiteral) -> CompileResult<ScalarValue> {
    let magnitude = parse_i64eger_magnitude(value)?;
    let scalar = match value.suffix {
        None | Some(HirIntegerSuffix::I64) => {
            ScalarValue::I64(checked_signed_positive(value, magnitude, i64::MAX as u128)? as i64)
        }
        Some(HirIntegerSuffix::I8) => {
            ScalarValue::I8(checked_signed_positive(value, magnitude, i8::MAX as u128)? as i8)
        }
        Some(HirIntegerSuffix::I16) => {
            ScalarValue::I16(checked_signed_positive(value, magnitude, i16::MAX as u128)? as i16)
        }
        Some(HirIntegerSuffix::I32) => {
            ScalarValue::I32(checked_signed_positive(value, magnitude, i32::MAX as u128)? as i32)
        }
        Some(HirIntegerSuffix::U8) => {
            ScalarValue::U8(checked_unsigned_positive(value, magnitude, u8::MAX as u128)? as u8)
        }
        Some(HirIntegerSuffix::U16) => {
            ScalarValue::U16(checked_unsigned_positive(value, magnitude, u16::MAX as u128)? as u16)
        }
        Some(HirIntegerSuffix::U32) => {
            ScalarValue::U32(checked_unsigned_positive(value, magnitude, u32::MAX as u128)? as u32)
        }
        Some(HirIntegerSuffix::U64) => {
            ScalarValue::U64(checked_unsigned_positive(value, magnitude, u64::MAX as u128)? as u64)
        }
    };
    Ok(scalar)
}

fn parse_i64eger_scalar_as(
    value: &HirIntegerLiteral,
    expected: PrimitiveTag,
) -> CompileResult<ScalarValue> {
    let magnitude = parse_i64eger_magnitude(value)?;
    let scalar = match expected {
        PrimitiveTag::I8 => {
            ScalarValue::I8(checked_signed_positive(value, magnitude, i8::MAX as u128)? as i8)
        }
        PrimitiveTag::I16 => {
            ScalarValue::I16(checked_signed_positive(value, magnitude, i16::MAX as u128)? as i16)
        }
        PrimitiveTag::I32 => {
            ScalarValue::I32(checked_signed_positive(value, magnitude, i32::MAX as u128)? as i32)
        }
        PrimitiveTag::I64 => {
            ScalarValue::I64(checked_signed_positive(value, magnitude, i64::MAX as u128)? as i64)
        }
        PrimitiveTag::U8 => {
            ScalarValue::U8(checked_unsigned_positive(value, magnitude, u8::MAX as u128)? as u8)
        }
        PrimitiveTag::U16 => {
            ScalarValue::U16(checked_unsigned_positive(value, magnitude, u16::MAX as u128)? as u16)
        }
        PrimitiveTag::U32 => {
            ScalarValue::U32(checked_unsigned_positive(value, magnitude, u32::MAX as u128)? as u32)
        }
        PrimitiveTag::U64 => {
            ScalarValue::U64(checked_unsigned_positive(value, magnitude, u64::MAX as u128)? as u64)
        }
        _ => unreachable!("caller only passes integer primitive tags"),
    };
    Ok(scalar)
}

fn parse_negated_integer_scalar(value: &HirIntegerLiteral) -> CompileResult<Option<ScalarValue>> {
    let magnitude = parse_i64eger_magnitude(value)?;
    let scalar = match value.suffix {
        None | Some(HirIntegerSuffix::I64) => {
            ScalarValue::I64(checked_signed_negative(value, magnitude, i64::MAX as u128)? as i64)
        }
        Some(HirIntegerSuffix::I8) => {
            ScalarValue::I8(checked_signed_negative(value, magnitude, i8::MAX as u128)? as i8)
        }
        Some(HirIntegerSuffix::I16) => {
            ScalarValue::I16(checked_signed_negative(value, magnitude, i16::MAX as u128)? as i16)
        }
        Some(HirIntegerSuffix::I32) => {
            ScalarValue::I32(checked_signed_negative(value, magnitude, i32::MAX as u128)? as i32)
        }
        Some(
            HirIntegerSuffix::U8
            | HirIntegerSuffix::U16
            | HirIntegerSuffix::U32
            | HirIntegerSuffix::U64,
        ) => {
            return Ok(None);
        }
    };
    Ok(Some(scalar))
}

fn parse_i64eger_magnitude(value: &HirIntegerLiteral) -> CompileResult<u128> {
    let value_without_separators = value.text.replace('_', "");
    let digits = match value.radix {
        HirIntRadix::Binary | HirIntRadix::Hex => &value_without_separators[2..],
        HirIntRadix::Decimal => value_without_separators.as_str(),
    };
    u128::from_str_radix(digits, int_radix_base(value.radix)).map_err(|error: ParseIntError| {
        CompileError::new(CompileErrorKind::InvalidIntLiteral {
            literal: integer_text(value),
            error: error.to_string(),
        })
    })
}

fn checked_signed_positive(
    literal: &HirIntegerLiteral,
    magnitude: u128,
    max: u128,
) -> CompileResult<u128> {
    if magnitude <= max {
        Ok(magnitude)
    } else {
        Err(out_of_range_integer(literal))
    }
}

fn checked_unsigned_positive(
    literal: &HirIntegerLiteral,
    magnitude: u128,
    max: u128,
) -> CompileResult<u128> {
    if magnitude <= max {
        Ok(magnitude)
    } else {
        Err(out_of_range_integer(literal))
    }
}

fn checked_signed_negative(
    literal: &HirIntegerLiteral,
    magnitude: u128,
    positive_max: u128,
) -> CompileResult<i128> {
    if magnitude <= positive_max + 1 {
        Ok(-(magnitude as i128))
    } else {
        Err(out_of_range_integer(literal))
    }
}

fn out_of_range_integer(value: &HirIntegerLiteral) -> CompileError {
    CompileError::new(CompileErrorKind::InvalidIntLiteral {
        literal: integer_text(value),
        error: "integer literal out of range".to_owned(),
    })
}

fn parse_f64_scalar(value: &HirFloatLiteral) -> CompileResult<ScalarValue> {
    match value.suffix {
        Some(HirFloatSuffix::F32) => parse_f64::<f32>(value).map(ScalarValue::F32),
        None | Some(HirFloatSuffix::F64) => parse_f64::<f64>(value).map(ScalarValue::F64),
    }
}

fn parse_f64_scalar_as(
    value: &HirFloatLiteral,
    expected: PrimitiveTag,
) -> CompileResult<ScalarValue> {
    match expected {
        PrimitiveTag::F32 => parse_f64::<f32>(value).map(ScalarValue::F32),
        PrimitiveTag::F64 => parse_f64::<f64>(value).map(ScalarValue::F64),
        _ => unreachable!("caller only passes float primitive tags"),
    }
}

fn is_integer_tag(tag: PrimitiveTag) -> bool {
    matches!(
        tag,
        PrimitiveTag::I8
            | PrimitiveTag::I16
            | PrimitiveTag::I32
            | PrimitiveTag::I64
            | PrimitiveTag::U8
            | PrimitiveTag::U16
            | PrimitiveTag::U32
            | PrimitiveTag::U64
    )
}

fn is_float_tag(tag: PrimitiveTag) -> bool {
    matches!(tag, PrimitiveTag::F32 | PrimitiveTag::F64)
}

fn parse_f64<T>(value: &HirFloatLiteral) -> CompileResult<T>
where
    T: Copy + Into<f64> + std::str::FromStr<Err = ParseFloatError>,
{
    let parsed: T = value
        .text
        .replace('_', "")
        .parse()
        .map_err(|error: ParseFloatError| {
            CompileError::new(CompileErrorKind::InvalidFloatLiteral {
                literal: float_text(value),
                error: error.to_string(),
            })
        })?;
    if parsed.into().is_finite() {
        Ok(parsed)
    } else {
        Err(CompileError::new(CompileErrorKind::InvalidFloatLiteral {
            literal: float_text(value),
            error: "float literal out of range".to_owned(),
        }))
    }
}

fn negate_float_scalar(value: ScalarValue) -> ScalarValue {
    match value {
        ScalarValue::F32(value) => ScalarValue::F32(-value),
        ScalarValue::F64(value) => ScalarValue::F64(-value),
        _ => value,
    }
}
