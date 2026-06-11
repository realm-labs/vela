use vela_bytecode::{BinaryLiteralOp, BinaryLiteralSide};
use vela_common::ScalarValue;

use crate::{Value, VmError, VmErrorKind, VmResult};

macro_rules! eval_int_literal_op {
    ($op:expr, $value:expr, $literal:expr, $side:expr, $ctor:path, $operation:expr) => {{
        let (lhs, rhs) = match $side {
            BinaryLiteralSide::Left => ($literal, $value),
            BinaryLiteralSide::Right => ($value, $literal),
        };
        match $op {
            BinaryLiteralOp::Add => lhs
                .checked_add(rhs)
                .map(|value| Value::Scalar($ctor(value)))
                .ok_or_else(|| {
                    VmError::new(VmErrorKind::TypeMismatch {
                        operation: $operation,
                    })
                }),
            BinaryLiteralOp::Sub => lhs
                .checked_sub(rhs)
                .map(|value| Value::Scalar($ctor(value)))
                .ok_or_else(|| {
                    VmError::new(VmErrorKind::TypeMismatch {
                        operation: $operation,
                    })
                }),
            BinaryLiteralOp::Mul => lhs
                .checked_mul(rhs)
                .map(|value| Value::Scalar($ctor(value)))
                .ok_or_else(|| {
                    VmError::new(VmErrorKind::TypeMismatch {
                        operation: $operation,
                    })
                }),
            BinaryLiteralOp::Div => {
                if rhs == 0 {
                    Err(VmError::new(VmErrorKind::DivisionByZero))
                } else {
                    lhs.checked_div(rhs)
                        .map(|value| Value::Scalar($ctor(value)))
                        .ok_or_else(|| {
                            VmError::new(VmErrorKind::TypeMismatch {
                                operation: $operation,
                            })
                        })
                }
            }
            BinaryLiteralOp::Rem => {
                if rhs == 0 {
                    Err(VmError::new(VmErrorKind::DivisionByZero))
                } else {
                    lhs.checked_rem(rhs)
                        .map(|value| Value::Scalar($ctor(value)))
                        .ok_or_else(|| {
                            VmError::new(VmErrorKind::TypeMismatch {
                                operation: $operation,
                            })
                        })
                }
            }
            BinaryLiteralOp::Less => Ok(Value::Bool(lhs < rhs)),
            BinaryLiteralOp::LessEqual => Ok(Value::Bool(lhs <= rhs)),
            BinaryLiteralOp::Greater => Ok(Value::Bool(lhs > rhs)),
            BinaryLiteralOp::GreaterEqual => Ok(Value::Bool(lhs >= rhs)),
        }
    }};
}

macro_rules! eval_float_literal_op {
    ($op:expr, $value:expr, $literal:expr, $side:expr, $ctor:path, $operation:expr) => {{
        let (lhs, rhs) = match $side {
            BinaryLiteralSide::Left => ($literal, $value),
            BinaryLiteralSide::Right => ($value, $literal),
        };
        match $op {
            BinaryLiteralOp::Add => Ok(Value::Scalar($ctor(lhs + rhs))),
            BinaryLiteralOp::Sub => Ok(Value::Scalar($ctor(lhs - rhs))),
            BinaryLiteralOp::Mul => Ok(Value::Scalar($ctor(lhs * rhs))),
            BinaryLiteralOp::Div => {
                if rhs == 0.0 {
                    Err(VmError::new(VmErrorKind::DivisionByZero))
                } else {
                    Ok(Value::Scalar($ctor(lhs / rhs)))
                }
            }
            BinaryLiteralOp::Rem => {
                if rhs == 0.0 {
                    Err(VmError::new(VmErrorKind::DivisionByZero))
                } else {
                    Ok(Value::Scalar($ctor(lhs % rhs)))
                }
            }
            BinaryLiteralOp::Less => Ok(Value::Bool(lhs < rhs)),
            BinaryLiteralOp::LessEqual => Ok(Value::Bool(lhs <= rhs)),
            BinaryLiteralOp::Greater => Ok(Value::Bool(lhs > rhs)),
            BinaryLiteralOp::GreaterEqual => Ok(Value::Bool(lhs >= rhs)),
        }
    }};
}

#[inline]
pub(crate) fn add_numeric(lhs: &Value, rhs: &Value) -> VmResult<Value> {
    match (lhs, rhs) {
        (
            Value::Scalar(vela_common::ScalarValue::I64(lhs)),
            Value::Scalar(vela_common::ScalarValue::I64(rhs)),
        ) => Ok(Value::Scalar(vela_common::ScalarValue::I64(lhs + rhs))),
        (
            Value::Scalar(vela_common::ScalarValue::F64(lhs)),
            Value::Scalar(vela_common::ScalarValue::F64(rhs)),
        ) => Ok(Value::Scalar(vela_common::ScalarValue::F64(lhs + rhs))),
        _ => type_mismatch("add"),
    }
}

#[inline]
pub(crate) fn sub_numeric(lhs: &Value, rhs: &Value) -> VmResult<Value> {
    match (lhs, rhs) {
        (
            Value::Scalar(vela_common::ScalarValue::I64(lhs)),
            Value::Scalar(vela_common::ScalarValue::I64(rhs)),
        ) => Ok(Value::Scalar(vela_common::ScalarValue::I64(lhs - rhs))),
        (
            Value::Scalar(vela_common::ScalarValue::F64(lhs)),
            Value::Scalar(vela_common::ScalarValue::F64(rhs)),
        ) => Ok(Value::Scalar(vela_common::ScalarValue::F64(lhs - rhs))),
        _ => type_mismatch("sub"),
    }
}

#[inline]
pub(crate) fn mul_numeric(lhs: &Value, rhs: &Value) -> VmResult<Value> {
    match (lhs, rhs) {
        (
            Value::Scalar(vela_common::ScalarValue::I64(lhs)),
            Value::Scalar(vela_common::ScalarValue::I64(rhs)),
        ) => Ok(Value::Scalar(vela_common::ScalarValue::I64(lhs * rhs))),
        (
            Value::Scalar(vela_common::ScalarValue::F64(lhs)),
            Value::Scalar(vela_common::ScalarValue::F64(rhs)),
        ) => Ok(Value::Scalar(vela_common::ScalarValue::F64(lhs * rhs))),
        _ => type_mismatch("mul"),
    }
}

#[inline]
pub(crate) fn negate_numeric(value: &Value) -> VmResult<Value> {
    match value {
        Value::Scalar(vela_common::ScalarValue::I64(value)) => {
            value.checked_neg().map(Value::i64).ok_or_else(|| {
                VmError::new(VmErrorKind::TypeMismatch {
                    operation: "negate",
                })
            })
        }
        Value::Scalar(vela_common::ScalarValue::F64(value)) => {
            Ok(Value::Scalar(vela_common::ScalarValue::F64(-value)))
        }
        _ => Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "negate",
        })),
    }
}

#[inline]
pub(crate) fn div_numeric(lhs: &Value, rhs: &Value) -> VmResult<Value> {
    match (lhs, rhs) {
        (
            Value::Scalar(vela_common::ScalarValue::I64(_)),
            Value::Scalar(vela_common::ScalarValue::I64(0)),
        ) => Err(VmError::new(VmErrorKind::DivisionByZero)),
        (
            Value::Scalar(vela_common::ScalarValue::I64(lhs)),
            Value::Scalar(vela_common::ScalarValue::I64(rhs)),
        ) => Ok(Value::Scalar(vela_common::ScalarValue::I64(lhs / rhs))),
        (
            Value::Scalar(vela_common::ScalarValue::F64(_)),
            Value::Scalar(vela_common::ScalarValue::F64(rhs)),
        ) if *rhs == 0.0 => Err(VmError::new(VmErrorKind::DivisionByZero)),
        (
            Value::Scalar(vela_common::ScalarValue::F64(lhs)),
            Value::Scalar(vela_common::ScalarValue::F64(rhs)),
        ) => Ok(Value::Scalar(vela_common::ScalarValue::F64(lhs / rhs))),
        _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation: "div" })),
    }
}

#[inline]
pub(crate) fn rem_numeric(lhs: &Value, rhs: &Value) -> VmResult<Value> {
    match (lhs, rhs) {
        (
            Value::Scalar(vela_common::ScalarValue::I64(_)),
            Value::Scalar(vela_common::ScalarValue::I64(0)),
        ) => Err(VmError::new(VmErrorKind::DivisionByZero)),
        (
            Value::Scalar(vela_common::ScalarValue::I64(lhs)),
            Value::Scalar(vela_common::ScalarValue::I64(rhs)),
        ) => Ok(Value::Scalar(vela_common::ScalarValue::I64(lhs % rhs))),
        (
            Value::Scalar(vela_common::ScalarValue::F64(_)),
            Value::Scalar(vela_common::ScalarValue::F64(rhs)),
        ) if *rhs == 0.0 => Err(VmError::new(VmErrorKind::DivisionByZero)),
        (
            Value::Scalar(vela_common::ScalarValue::F64(lhs)),
            Value::Scalar(vela_common::ScalarValue::F64(rhs)),
        ) => Ok(Value::Scalar(vela_common::ScalarValue::F64(lhs % rhs))),
        _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation: "rem" })),
    }
}

#[inline]
pub(crate) fn less_numeric(lhs: &Value, rhs: &Value) -> VmResult<bool> {
    match (lhs, rhs) {
        (
            Value::Scalar(vela_common::ScalarValue::I64(lhs)),
            Value::Scalar(vela_common::ScalarValue::I64(rhs)),
        ) => Ok(lhs < rhs),
        (
            Value::Scalar(vela_common::ScalarValue::F64(lhs)),
            Value::Scalar(vela_common::ScalarValue::F64(rhs)),
        ) => Ok(lhs < rhs),
        _ => type_mismatch("less"),
    }
}

#[inline]
pub(crate) fn less_equal_numeric(lhs: &Value, rhs: &Value) -> VmResult<bool> {
    match (lhs, rhs) {
        (
            Value::Scalar(vela_common::ScalarValue::I64(lhs)),
            Value::Scalar(vela_common::ScalarValue::I64(rhs)),
        ) => Ok(lhs <= rhs),
        (
            Value::Scalar(vela_common::ScalarValue::F64(lhs)),
            Value::Scalar(vela_common::ScalarValue::F64(rhs)),
        ) => Ok(lhs <= rhs),
        _ => type_mismatch("less_equal"),
    }
}

#[inline]
pub(crate) fn greater_numeric(lhs: &Value, rhs: &Value) -> VmResult<bool> {
    match (lhs, rhs) {
        (
            Value::Scalar(vela_common::ScalarValue::I64(lhs)),
            Value::Scalar(vela_common::ScalarValue::I64(rhs)),
        ) => Ok(lhs > rhs),
        (
            Value::Scalar(vela_common::ScalarValue::F64(lhs)),
            Value::Scalar(vela_common::ScalarValue::F64(rhs)),
        ) => Ok(lhs > rhs),
        _ => type_mismatch("greater"),
    }
}

#[inline]
pub(crate) fn greater_equal_numeric(lhs: &Value, rhs: &Value) -> VmResult<bool> {
    match (lhs, rhs) {
        (
            Value::Scalar(vela_common::ScalarValue::I64(lhs)),
            Value::Scalar(vela_common::ScalarValue::I64(rhs)),
        ) => Ok(lhs >= rhs),
        (
            Value::Scalar(vela_common::ScalarValue::F64(lhs)),
            Value::Scalar(vela_common::ScalarValue::F64(rhs)),
        ) => Ok(lhs >= rhs),
        _ => type_mismatch("greater_equal"),
    }
}

pub(crate) fn binary_int_literal_numeric(
    op: BinaryLiteralOp,
    value: &Value,
    literal: &str,
    side: BinaryLiteralSide,
) -> VmResult<Value> {
    match value {
        Value::Scalar(ScalarValue::I8(value)) => {
            let literal = parse_integer_literal_as(literal, i8::MAX as u128)? as i8;
            eval_int_literal_op!(
                op,
                *value,
                literal,
                side,
                ScalarValue::I8,
                "binary_int_literal"
            )
        }
        Value::Scalar(ScalarValue::I16(value)) => {
            let literal = parse_integer_literal_as(literal, i16::MAX as u128)? as i16;
            eval_int_literal_op!(
                op,
                *value,
                literal,
                side,
                ScalarValue::I16,
                "binary_int_literal"
            )
        }
        Value::Scalar(ScalarValue::I32(value)) => {
            let literal = parse_integer_literal_as(literal, i32::MAX as u128)? as i32;
            eval_int_literal_op!(
                op,
                *value,
                literal,
                side,
                ScalarValue::I32,
                "binary_int_literal"
            )
        }
        Value::Scalar(ScalarValue::I64(value)) => {
            let literal = parse_integer_literal_as(literal, i64::MAX as u128)? as i64;
            eval_int_literal_op!(
                op,
                *value,
                literal,
                side,
                ScalarValue::I64,
                "binary_int_literal"
            )
        }
        Value::Scalar(ScalarValue::U8(value)) => {
            let literal = parse_integer_literal_as(literal, u8::MAX as u128)? as u8;
            eval_int_literal_op!(
                op,
                *value,
                literal,
                side,
                ScalarValue::U8,
                "binary_int_literal"
            )
        }
        Value::Scalar(ScalarValue::U16(value)) => {
            let literal = parse_integer_literal_as(literal, u16::MAX as u128)? as u16;
            eval_int_literal_op!(
                op,
                *value,
                literal,
                side,
                ScalarValue::U16,
                "binary_int_literal"
            )
        }
        Value::Scalar(ScalarValue::U32(value)) => {
            let literal = parse_integer_literal_as(literal, u32::MAX as u128)? as u32;
            eval_int_literal_op!(
                op,
                *value,
                literal,
                side,
                ScalarValue::U32,
                "binary_int_literal"
            )
        }
        Value::Scalar(ScalarValue::U64(value)) => {
            let literal = parse_integer_literal_as(literal, u64::MAX as u128)? as u64;
            eval_int_literal_op!(
                op,
                *value,
                literal,
                side,
                ScalarValue::U64,
                "binary_int_literal"
            )
        }
        _ => type_mismatch("binary_int_literal"),
    }
}

pub(crate) fn binary_float_literal_numeric(
    op: BinaryLiteralOp,
    value: &Value,
    literal: &str,
    side: BinaryLiteralSide,
) -> VmResult<Value> {
    match value {
        Value::Scalar(ScalarValue::F32(value)) => {
            let literal = literal.parse::<f32>().map_err(|_| {
                VmError::new(VmErrorKind::TypeMismatch {
                    operation: "binary_float_literal",
                })
            })?;
            eval_float_literal_op!(
                op,
                *value,
                literal,
                side,
                ScalarValue::F32,
                "binary_float_literal"
            )
        }
        Value::Scalar(ScalarValue::F64(value)) => {
            let literal = literal.parse::<f64>().map_err(|_| {
                VmError::new(VmErrorKind::TypeMismatch {
                    operation: "binary_float_literal",
                })
            })?;
            eval_float_literal_op!(
                op,
                *value,
                literal,
                side,
                ScalarValue::F64,
                "binary_float_literal"
            )
        }
        _ => type_mismatch("binary_float_literal"),
    }
}

fn parse_integer_literal_as(literal: &str, max: u128) -> VmResult<u128> {
    let value = literal.replace('_', "");
    let (digits, radix) = if value.starts_with("0x") || value.starts_with("0X") {
        (&value[2..], 16)
    } else if value.starts_with("0b") || value.starts_with("0B") {
        (&value[2..], 2)
    } else {
        (value.as_str(), 10)
    };
    let magnitude = u128::from_str_radix(digits, radix).map_err(|_| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "binary_int_literal",
        })
    })?;
    if magnitude <= max {
        Ok(magnitude)
    } else {
        type_mismatch("binary_int_literal")
    }
}

#[inline]
fn type_mismatch<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}
