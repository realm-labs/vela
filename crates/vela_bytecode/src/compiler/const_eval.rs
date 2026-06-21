use std::collections::BTreeMap;
use std::num::{ParseFloatError, ParseIntError};

use vela_common::{PrimitiveTag, ScalarValue, SourceId, Span};
use vela_syntax::TextRange;
use vela_syntax::ast::{
    AstNode, BinaryOp, FloatLiteral, FloatSuffix, IntRadix, IntegerLiteral, IntegerSuffix, Literal,
    SyntaxBlock, SyntaxExpression, SyntaxExpressionKind, SyntaxMapEntry, SyntaxStatementKind,
    UnaryOp,
};

use crate::Constant;

use super::{CompileError, CompileErrorKind, CompileResult};

pub(super) fn compile_literal_constant(literal: &Literal) -> CompileResult<Constant> {
    Ok(match literal {
        Literal::Null => Constant::Null,
        Literal::Bool(value) => Constant::Bool(*value),
        Literal::Char(value) => Constant::Char(*value),
        Literal::Integer(value) => Constant::Scalar(parse_i64eger_scalar(value)?),
        Literal::Float(value) => Constant::Scalar(parse_f64_scalar(value)?),
        Literal::String(value) => Constant::String(value.clone()),
        Literal::Bytes(value) => Constant::Bytes(value.clone()),
    })
}

pub(super) fn compile_literal_constant_for_type(
    literal: &Literal,
    expected: PrimitiveTag,
) -> CompileResult<Option<Constant>> {
    match literal {
        Literal::Integer(value) if value.suffix.is_none() && is_integer_tag(expected) => {
            parse_i64eger_scalar_as(value, expected).map(|value| Some(Constant::Scalar(value)))
        }
        Literal::Float(value) if value.suffix.is_none() && is_float_tag(expected) => {
            parse_f64_scalar_as(value, expected).map(|value| Some(Constant::Scalar(value)))
        }
        _ => Ok(None),
    }
}

pub(super) fn compile_negated_literal_constant(
    literal: &Literal,
) -> CompileResult<Option<Constant>> {
    match literal {
        Literal::Integer(value) => {
            parse_negated_integer_scalar(value).map(|value| value.map(Constant::Scalar))
        }
        Literal::Float(value) => Ok(Some(Constant::Scalar(negate_float_scalar(
            parse_f64_scalar(value)?,
        )))),
        _ => Ok(None),
    }
}

pub(super) fn evaluate_syntax_const_expr(
    source: SourceId,
    expr: &SyntaxExpression,
    values_by_name: &BTreeMap<String, Constant>,
) -> CompileResult<Option<Constant>> {
    match expr.expression_kind() {
        SyntaxExpressionKind::Literal => {
            let Some(literal) = expr.as_literal().and_then(|literal| literal.literal()) else {
                return Ok(None);
            };
            compile_literal_constant(&literal)
                .map(Some)
                .map_err(|error| error.with_span(span_for(source, expr.syntax().text_range())))
        }
        SyntaxExpressionKind::Path => {
            let Some(path) = expr.as_path() else {
                return Ok(None);
            };
            let segments = path.path_segments();
            let [name] = segments.as_slice() else {
                return Ok(None);
            };
            Ok(values_by_name.get(name).cloned())
        }
        SyntaxExpressionKind::Paren => {
            let Some(inner) = expr.as_paren().and_then(|paren| paren.expression()) else {
                return Ok(None);
            };
            evaluate_syntax_const_expr(source, &inner, values_by_name)
        }
        SyntaxExpressionKind::Unary => {
            let Some(unary) = expr.as_unary() else {
                return Ok(None);
            };
            let Some(op) = unary.operator() else {
                return Ok(None);
            };
            let Some(inner) = unary.expression() else {
                return Ok(None);
            };
            if op == UnaryOp::Negate
                && let Some(literal) = inner.as_literal().and_then(|literal| literal.literal())
                && let Some(value) =
                    compile_negated_literal_constant(&literal).map_err(|error| {
                        error.with_span(span_for(source, inner.syntax().text_range()))
                    })?
            {
                return Ok(Some(value));
            }
            let Some(value) = evaluate_syntax_const_expr(source, &inner, values_by_name)? else {
                return Ok(None);
            };
            Ok(evaluate_unary_const(op, value))
        }
        SyntaxExpressionKind::Binary => {
            let Some(binary) = expr.as_binary() else {
                return Ok(None);
            };
            let Some(op) = binary.operator() else {
                return Ok(None);
            };
            let Some(left_expr) = binary.lhs() else {
                return Ok(None);
            };
            let Some(right_expr) = binary.rhs() else {
                return Ok(None);
            };
            let Some(left) = evaluate_syntax_const_expr(source, &left_expr, values_by_name)? else {
                return Ok(None);
            };
            let Some(right) = evaluate_syntax_const_expr(source, &right_expr, values_by_name)?
            else {
                return Ok(None);
            };
            Ok(evaluate_binary_const(op, left, right))
        }
        SyntaxExpressionKind::Array => {
            let Some(array) = expr.as_array() else {
                return Ok(None);
            };
            array
                .expressions()
                .map(|value| evaluate_syntax_const_expr(source, &value, values_by_name))
                .collect::<CompileResult<Option<Vec<_>>>>()
                .map(|values| values.map(Constant::Array))
        }
        SyntaxExpressionKind::Map => {
            let Some(map) = expr.as_map() else {
                return Ok(None);
            };
            map.entries()
                .map(|entry| evaluate_syntax_map_entry(source, &entry, values_by_name))
                .collect::<CompileResult<Option<Vec<_>>>>()
                .map(|entries| entries.map(Constant::Map))
        }
        SyntaxExpressionKind::Block => {
            let Some(block) = expr.as_block() else {
                return Ok(None);
            };
            evaluate_syntax_const_block(source, &block, values_by_name)
        }
        SyntaxExpressionKind::Assign
        | SyntaxExpressionKind::Field
        | SyntaxExpressionKind::Call
        | SyntaxExpressionKind::Index
        | SyntaxExpressionKind::Try
        | SyntaxExpressionKind::Record
        | SyntaxExpressionKind::Lambda
        | SyntaxExpressionKind::If
        | SyntaxExpressionKind::Match => Ok(None),
    }
}

fn evaluate_syntax_const_block(
    source: SourceId,
    block: &SyntaxBlock,
    values_by_name: &BTreeMap<String, Constant>,
) -> CompileResult<Option<Constant>> {
    let mut local_values = values_by_name.clone();
    let mut tail_value = None;
    for statement in block.statements() {
        match statement.statement_kind() {
            SyntaxStatementKind::Let => {
                let Some(statement) = statement.as_let() else {
                    return Ok(None);
                };
                let Some(name) = statement.name_text() else {
                    return Ok(None);
                };
                let Some(initializer) = statement.initializer() else {
                    return Ok(None);
                };
                let Some(value) = evaluate_syntax_const_expr(source, &initializer, &local_values)?
                else {
                    return Ok(None);
                };
                local_values.insert(name, value);
                tail_value = None;
            }
            SyntaxStatementKind::Return => {
                let Some(statement) = statement.as_return() else {
                    return Ok(None);
                };
                let Some(value) = statement.expression() else {
                    return Ok(Some(Constant::Null));
                };
                return evaluate_syntax_const_expr(source, &value, &local_values);
            }
            SyntaxStatementKind::Expr => {
                let Some(statement) = statement.as_expr() else {
                    return Ok(None);
                };
                let Some(value) = statement.expression() else {
                    return Ok(None);
                };
                tail_value = if statement.semicolon_token().is_some() {
                    None
                } else {
                    evaluate_syntax_const_expr(source, &value, &local_values)?
                };
            }
            SyntaxStatementKind::Block => {
                let Some(statement) = statement.as_block() else {
                    return Ok(None);
                };
                tail_value = evaluate_syntax_const_block(source, &statement, &local_values)?;
            }
            SyntaxStatementKind::Break
            | SyntaxStatementKind::Continue
            | SyntaxStatementKind::For
            | SyntaxStatementKind::If
            | SyntaxStatementKind::Match => return Ok(None),
        }
    }
    Ok(tail_value)
}

fn evaluate_syntax_map_entry(
    source: SourceId,
    entry: &SyntaxMapEntry,
    values_by_name: &BTreeMap<String, Constant>,
) -> CompileResult<Option<(String, Constant)>> {
    let Some(value_expr) = entry.value() else {
        return Ok(None);
    };
    let Some(value) = evaluate_syntax_const_expr(source, &value_expr, values_by_name)? else {
        return Ok(None);
    };
    let Some(key_expr) = entry.key() else {
        return Ok(None);
    };
    let Some(key) = syntax_const_map_key_name(&key_expr)? else {
        return Ok(None);
    };
    Ok(Some((key, value)))
}

fn syntax_const_map_key_name(key: &SyntaxExpression) -> CompileResult<Option<String>> {
    match key.expression_kind() {
        SyntaxExpressionKind::Literal => {
            let Some(literal) = key.as_literal().and_then(|literal| literal.literal()) else {
                return Ok(None);
            };
            match literal {
                Literal::String(value) => Ok(Some(value)),
                Literal::Integer(value) => Ok(Some(value.source_text_with_suffix())),
                Literal::Float(value) => Ok(Some(value.source_text_with_suffix())),
                _ => Ok(None),
            }
        }
        SyntaxExpressionKind::Path => Ok(key
            .as_path()
            .map(|path| path.path_segments().join("::"))
            .filter(|path| !path.is_empty())),
        _ => Ok(None),
    }
}

fn span_for(source: SourceId, range: TextRange) -> Span {
    Span::new(source, range.start().into(), range.end().into())
}

fn evaluate_unary_const(op: UnaryOp, value: Constant) -> Option<Constant> {
    match (op, value) {
        (UnaryOp::Negate, Constant::Scalar(ScalarValue::I8(value))) => value
            .checked_neg()
            .map(|value| Constant::Scalar(value.into())),
        (UnaryOp::Negate, Constant::Scalar(ScalarValue::I16(value))) => value
            .checked_neg()
            .map(|value| Constant::Scalar(value.into())),
        (UnaryOp::Negate, Constant::Scalar(ScalarValue::I32(value))) => value
            .checked_neg()
            .map(|value| Constant::Scalar(value.into())),
        (UnaryOp::Negate, Constant::Scalar(ScalarValue::I64(value))) => value
            .checked_neg()
            .map(|value| Constant::Scalar(value.into())),
        (UnaryOp::Negate, Constant::Scalar(ScalarValue::F32(value))) => {
            Some(Constant::Scalar(ScalarValue::F32(-value)))
        }
        (UnaryOp::Negate, Constant::Scalar(ScalarValue::F64(value))) => {
            Some(Constant::Scalar(ScalarValue::F64(-value)))
        }
        (UnaryOp::Not, Constant::Bool(value)) => Some(Constant::Bool(!value)),
        _ => None,
    }
}

fn evaluate_binary_const(op: BinaryOp, left: Constant, right: Constant) -> Option<Constant> {
    match op {
        BinaryOp::Add => evaluate_numeric_const(left, right, i64::checked_add, |a, b| a + b),
        BinaryOp::Sub => evaluate_numeric_const(left, right, i64::checked_sub, |a, b| a - b),
        BinaryOp::Mul => evaluate_numeric_const(left, right, i64::checked_mul, |a, b| a * b),
        BinaryOp::Div => match (left, right) {
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
        BinaryOp::Rem => match (left, right) {
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
        BinaryOp::Equal => Some(Constant::Bool(left == right)),
        BinaryOp::NotEqual => Some(Constant::Bool(left != right)),
        BinaryOp::IdentityEqual | BinaryOp::IdentityNotEqual => None,
        BinaryOp::Less => evaluate_numeric_compare_const(left, right, |a, b| a < b),
        BinaryOp::LessEqual => evaluate_numeric_compare_const(left, right, |a, b| a <= b),
        BinaryOp::Greater => evaluate_numeric_compare_const(left, right, |a, b| a > b),
        BinaryOp::GreaterEqual => evaluate_numeric_compare_const(left, right, |a, b| a >= b),
        BinaryOp::Range | BinaryOp::RangeInclusive | BinaryOp::Or | BinaryOp::And => None,
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

fn parse_i64eger_scalar(value: &IntegerLiteral) -> CompileResult<ScalarValue> {
    let magnitude = parse_i64eger_magnitude(value)?;
    let scalar = match value.suffix {
        None | Some(IntegerSuffix::I64) => {
            ScalarValue::I64(checked_signed_positive(value, magnitude, i64::MAX as u128)? as i64)
        }
        Some(IntegerSuffix::I8) => {
            ScalarValue::I8(checked_signed_positive(value, magnitude, i8::MAX as u128)? as i8)
        }
        Some(IntegerSuffix::I16) => {
            ScalarValue::I16(checked_signed_positive(value, magnitude, i16::MAX as u128)? as i16)
        }
        Some(IntegerSuffix::I32) => {
            ScalarValue::I32(checked_signed_positive(value, magnitude, i32::MAX as u128)? as i32)
        }
        Some(IntegerSuffix::U8) => {
            ScalarValue::U8(checked_unsigned_positive(value, magnitude, u8::MAX as u128)? as u8)
        }
        Some(IntegerSuffix::U16) => {
            ScalarValue::U16(checked_unsigned_positive(value, magnitude, u16::MAX as u128)? as u16)
        }
        Some(IntegerSuffix::U32) => {
            ScalarValue::U32(checked_unsigned_positive(value, magnitude, u32::MAX as u128)? as u32)
        }
        Some(IntegerSuffix::U64) => {
            ScalarValue::U64(checked_unsigned_positive(value, magnitude, u64::MAX as u128)? as u64)
        }
    };
    Ok(scalar)
}

fn parse_i64eger_scalar_as(
    value: &IntegerLiteral,
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

fn parse_negated_integer_scalar(value: &IntegerLiteral) -> CompileResult<Option<ScalarValue>> {
    let magnitude = parse_i64eger_magnitude(value)?;
    let scalar = match value.suffix {
        None | Some(IntegerSuffix::I64) => {
            ScalarValue::I64(checked_signed_negative(value, magnitude, i64::MAX as u128)? as i64)
        }
        Some(IntegerSuffix::I8) => {
            ScalarValue::I8(checked_signed_negative(value, magnitude, i8::MAX as u128)? as i8)
        }
        Some(IntegerSuffix::I16) => {
            ScalarValue::I16(checked_signed_negative(value, magnitude, i16::MAX as u128)? as i16)
        }
        Some(IntegerSuffix::I32) => {
            ScalarValue::I32(checked_signed_negative(value, magnitude, i32::MAX as u128)? as i32)
        }
        Some(IntegerSuffix::U8 | IntegerSuffix::U16 | IntegerSuffix::U32 | IntegerSuffix::U64) => {
            return Ok(None);
        }
    };
    Ok(Some(scalar))
}

fn parse_i64eger_magnitude(value: &IntegerLiteral) -> CompileResult<u128> {
    let value_without_separators = value.source_text().replace('_', "");
    let digits = match value.radix {
        IntRadix::Binary | IntRadix::Hex => &value_without_separators[2..],
        IntRadix::Decimal => value_without_separators.as_str(),
    };
    u128::from_str_radix(digits, value.radix.base()).map_err(|error: ParseIntError| {
        CompileError::new(CompileErrorKind::InvalidIntLiteral {
            literal: value.source_text_with_suffix(),
            error: error.to_string(),
        })
    })
}

fn checked_signed_positive(
    literal: &IntegerLiteral,
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
    literal: &IntegerLiteral,
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
    literal: &IntegerLiteral,
    magnitude: u128,
    positive_max: u128,
) -> CompileResult<i128> {
    if magnitude <= positive_max + 1 {
        Ok(-(magnitude as i128))
    } else {
        Err(out_of_range_integer(literal))
    }
}

fn out_of_range_integer(value: &IntegerLiteral) -> CompileError {
    CompileError::new(CompileErrorKind::InvalidIntLiteral {
        literal: value.source_text_with_suffix(),
        error: "integer literal out of range".to_owned(),
    })
}

fn parse_f64_scalar(value: &FloatLiteral) -> CompileResult<ScalarValue> {
    match value.suffix {
        Some(FloatSuffix::F32) => parse_f64::<f32>(value).map(ScalarValue::F32),
        None | Some(FloatSuffix::F64) => parse_f64::<f64>(value).map(ScalarValue::F64),
    }
}

fn parse_f64_scalar_as(value: &FloatLiteral, expected: PrimitiveTag) -> CompileResult<ScalarValue> {
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

fn parse_f64<T>(value: &FloatLiteral) -> CompileResult<T>
where
    T: Copy + Into<f64> + std::str::FromStr<Err = ParseFloatError>,
{
    let parsed: T =
        value
            .source_text()
            .replace('_', "")
            .parse()
            .map_err(|error: ParseFloatError| {
                CompileError::new(CompileErrorKind::InvalidFloatLiteral {
                    literal: value.source_text_with_suffix(),
                    error: error.to_string(),
                })
            })?;
    if parsed.into().is_finite() {
        Ok(parsed)
    } else {
        Err(CompileError::new(CompileErrorKind::InvalidFloatLiteral {
            literal: value.source_text_with_suffix(),
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
