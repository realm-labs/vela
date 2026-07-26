use vela_bytecode::linked::{LinkedFloatLiteral, LinkedIntLiteral};
use vela_bytecode::{BinaryLiteralOp, BinaryLiteralSide};

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
                .map($ctor)
                .ok_or_else(|| arithmetic_overflow($operation)),
            BinaryLiteralOp::Sub => lhs
                .checked_sub(rhs)
                .map($ctor)
                .ok_or_else(|| arithmetic_overflow($operation)),
            BinaryLiteralOp::Mul => lhs
                .checked_mul(rhs)
                .map($ctor)
                .ok_or_else(|| arithmetic_overflow($operation)),
            BinaryLiteralOp::Div => {
                if rhs == 0 {
                    Err(VmError::new(VmErrorKind::DivisionByZero))
                } else {
                    lhs.checked_div(rhs)
                        .map($ctor)
                        .ok_or_else(|| arithmetic_overflow($operation))
                }
            }
            BinaryLiteralOp::Rem => {
                if rhs == 0 {
                    Err(VmError::new(VmErrorKind::DivisionByZero))
                } else {
                    lhs.checked_rem(rhs)
                        .map($ctor)
                        .ok_or_else(|| arithmetic_overflow($operation))
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
    ($op:expr, $value:expr, $literal:expr, $side:expr, $ctor:path) => {{
        let (lhs, rhs) = match $side {
            BinaryLiteralSide::Left => ($literal, $value),
            BinaryLiteralSide::Right => ($value, $literal),
        };
        match $op {
            BinaryLiteralOp::Add => Ok($ctor(lhs + rhs)),
            BinaryLiteralOp::Sub => Ok($ctor(lhs - rhs)),
            BinaryLiteralOp::Mul => Ok($ctor(lhs * rhs)),
            BinaryLiteralOp::Div => {
                if rhs == 0.0 {
                    Err(VmError::new(VmErrorKind::DivisionByZero))
                } else {
                    Ok($ctor(lhs / rhs))
                }
            }
            BinaryLiteralOp::Rem => {
                if rhs == 0.0 {
                    Err(VmError::new(VmErrorKind::DivisionByZero))
                } else {
                    Ok($ctor(lhs % rhs))
                }
            }
            BinaryLiteralOp::Less => Ok(Value::Bool(lhs < rhs)),
            BinaryLiteralOp::LessEqual => Ok(Value::Bool(lhs <= rhs)),
            BinaryLiteralOp::Greater => Ok(Value::Bool(lhs > rhs)),
            BinaryLiteralOp::GreaterEqual => Ok(Value::Bool(lhs >= rhs)),
        }
    }};
}

macro_rules! scalar_checked_arithmetic {
    ($lhs:expr, $rhs:expr, $operation:expr, $method:ident, $float_op:tt) => {{
        match ($lhs, $rhs) {
            (Value::I8(lhs), Value::I8(rhs)) => lhs
                .$method(*rhs)
                .map(Value::I8)
                .ok_or_else(|| arithmetic_overflow($operation)),
            (Value::I16(lhs), Value::I16(rhs)) => lhs
                .$method(*rhs)
                .map(Value::I16)
                .ok_or_else(|| arithmetic_overflow($operation)),
            (Value::I32(lhs), Value::I32(rhs)) => lhs
                .$method(*rhs)
                .map(Value::I32)
                .ok_or_else(|| arithmetic_overflow($operation)),
            (Value::I64(lhs), Value::I64(rhs)) => lhs
                .$method(*rhs)
                .map(Value::I64)
                .ok_or_else(|| arithmetic_overflow($operation)),
            (Value::U8(lhs), Value::U8(rhs)) => lhs
                .$method(*rhs)
                .map(Value::U8)
                .ok_or_else(|| arithmetic_overflow($operation)),
            (Value::U16(lhs), Value::U16(rhs)) => lhs
                .$method(*rhs)
                .map(Value::U16)
                .ok_or_else(|| arithmetic_overflow($operation)),
            (Value::U32(lhs), Value::U32(rhs)) => lhs
                .$method(*rhs)
                .map(Value::U32)
                .ok_or_else(|| arithmetic_overflow($operation)),
            (Value::U64(lhs), Value::U64(rhs)) => lhs
                .$method(*rhs)
                .map(Value::U64)
                .ok_or_else(|| arithmetic_overflow($operation)),
            (Value::F32(lhs), Value::F32(rhs)) => Ok(Value::F32(*lhs $float_op *rhs)),
            (Value::F64(lhs), Value::F64(rhs)) => Ok(Value::F64(*lhs $float_op *rhs)),
            _ => type_mismatch($operation),
        }
    }};
}

macro_rules! scalar_div_rem {
    ($lhs:expr, $rhs:expr, $operation:expr, $method:ident, $float_op:tt) => {{
        match ($lhs, $rhs) {
            (Value::I8(lhs), Value::I8(rhs)) => {
                checked_div_rem(*lhs, *rhs, Value::I8, $operation, |lhs, rhs| {
                    lhs.$method(rhs)
                })
            }
            (Value::I16(lhs), Value::I16(rhs)) => {
                checked_div_rem(*lhs, *rhs, Value::I16, $operation, |lhs, rhs| {
                    lhs.$method(rhs)
                })
            }
            (Value::I32(lhs), Value::I32(rhs)) => {
                checked_div_rem(*lhs, *rhs, Value::I32, $operation, |lhs, rhs| {
                    lhs.$method(rhs)
                })
            }
            (Value::I64(lhs), Value::I64(rhs)) => {
                checked_div_rem(*lhs, *rhs, Value::I64, $operation, |lhs, rhs| {
                    lhs.$method(rhs)
                })
            }
            (Value::U8(lhs), Value::U8(rhs)) => {
                checked_div_rem(*lhs, *rhs, Value::U8, $operation, |lhs, rhs| {
                    lhs.$method(rhs)
                })
            }
            (Value::U16(lhs), Value::U16(rhs)) => {
                checked_div_rem(*lhs, *rhs, Value::U16, $operation, |lhs, rhs| {
                    lhs.$method(rhs)
                })
            }
            (Value::U32(lhs), Value::U32(rhs)) => {
                checked_div_rem(*lhs, *rhs, Value::U32, $operation, |lhs, rhs| {
                    lhs.$method(rhs)
                })
            }
            (Value::U64(lhs), Value::U64(rhs)) => {
                checked_div_rem(*lhs, *rhs, Value::U64, $operation, |lhs, rhs| {
                    lhs.$method(rhs)
                })
            }
            (Value::F32(lhs), Value::F32(rhs)) => {
                if *rhs == 0.0 {
                    Err(VmError::new(VmErrorKind::DivisionByZero))
                } else {
                    Ok(Value::F32(*lhs $float_op *rhs))
                }
            }
            (Value::F64(lhs), Value::F64(rhs)) => {
                if *rhs == 0.0 {
                    Err(VmError::new(VmErrorKind::DivisionByZero))
                } else {
                    Ok(Value::F64(*lhs $float_op *rhs))
                }
            }
            _ => type_mismatch($operation),
        }
    }};
}

macro_rules! scalar_comparison {
    ($lhs:expr, $rhs:expr, $operation:expr, $op:tt) => {{
        match ($lhs, $rhs) {
            (Value::I8(lhs), Value::I8(rhs)) => Ok(*lhs $op *rhs),
            (Value::I16(lhs), Value::I16(rhs)) => Ok(*lhs $op *rhs),
            (Value::I32(lhs), Value::I32(rhs)) => Ok(*lhs $op *rhs),
            (Value::I64(lhs), Value::I64(rhs)) => Ok(*lhs $op *rhs),
            (Value::U8(lhs), Value::U8(rhs)) => Ok(*lhs $op *rhs),
            (Value::U16(lhs), Value::U16(rhs)) => Ok(*lhs $op *rhs),
            (Value::U32(lhs), Value::U32(rhs)) => Ok(*lhs $op *rhs),
            (Value::U64(lhs), Value::U64(rhs)) => Ok(*lhs $op *rhs),
            (Value::F32(lhs), Value::F32(rhs)) => Ok(*lhs $op *rhs),
            (Value::F64(lhs), Value::F64(rhs)) => Ok(*lhs $op *rhs),
            _ => type_mismatch($operation),
        }
    }};
}

#[inline]
pub(crate) fn add_numeric(lhs: &Value, rhs: &Value) -> VmResult<Value> {
    scalar_checked_arithmetic!(lhs, rhs, "add", checked_add, +)
}

#[inline]
pub(crate) fn sub_numeric(lhs: &Value, rhs: &Value) -> VmResult<Value> {
    scalar_checked_arithmetic!(lhs, rhs, "sub", checked_sub, -)
}

#[inline]
pub(crate) fn mul_numeric(lhs: &Value, rhs: &Value) -> VmResult<Value> {
    scalar_checked_arithmetic!(lhs, rhs, "mul", checked_mul, *)
}

#[inline]
pub(crate) fn negate_numeric(value: &Value) -> VmResult<Value> {
    match value {
        Value::I8(value) => value
            .checked_neg()
            .map(Value::I8)
            .ok_or_else(|| arithmetic_overflow("negate")),
        Value::I16(value) => value
            .checked_neg()
            .map(Value::I16)
            .ok_or_else(|| arithmetic_overflow("negate")),
        Value::I32(value) => value
            .checked_neg()
            .map(Value::I32)
            .ok_or_else(|| arithmetic_overflow("negate")),
        Value::I64(value) => value
            .checked_neg()
            .map(Value::I64)
            .ok_or_else(|| arithmetic_overflow("negate")),
        Value::F32(value) => Ok(Value::F32(-value)),
        Value::F64(value) => Ok(Value::F64(-value)),
        _ => type_mismatch("negate"),
    }
}

#[inline]
pub(crate) fn div_numeric(lhs: &Value, rhs: &Value) -> VmResult<Value> {
    scalar_div_rem!(lhs, rhs, "div", checked_div, /)
}

#[inline]
pub(crate) fn rem_numeric(lhs: &Value, rhs: &Value) -> VmResult<Value> {
    scalar_div_rem!(lhs, rhs, "rem", checked_rem, %)
}

#[inline]
pub(crate) fn less_numeric(lhs: &Value, rhs: &Value) -> VmResult<bool> {
    scalar_comparison!(lhs, rhs, "less", <)
}

#[inline]
pub(crate) fn less_equal_numeric(lhs: &Value, rhs: &Value) -> VmResult<bool> {
    scalar_comparison!(lhs, rhs, "less_equal", <=)
}

#[inline]
pub(crate) fn greater_numeric(lhs: &Value, rhs: &Value) -> VmResult<bool> {
    scalar_comparison!(lhs, rhs, "greater", >)
}

#[inline]
pub(crate) fn greater_equal_numeric(lhs: &Value, rhs: &Value) -> VmResult<bool> {
    scalar_comparison!(lhs, rhs, "greater_equal", >=)
}

pub(crate) fn binary_int_literal_numeric(
    op: BinaryLiteralOp,
    value: &Value,
    magnitude: LinkedIntLiteral,
    side: BinaryLiteralSide,
) -> VmResult<Value> {
    match value {
        Value::I8(value) => {
            let literal = int_literal_as(magnitude, i8::MAX as u64)? as i8;
            eval_int_literal_op!(op, *value, literal, side, Value::I8, "binary_int_literal")
        }
        Value::I16(value) => {
            let literal = int_literal_as(magnitude, i16::MAX as u64)? as i16;
            eval_int_literal_op!(op, *value, literal, side, Value::I16, "binary_int_literal")
        }
        Value::I32(value) => {
            let literal = int_literal_as(magnitude, i32::MAX as u64)? as i32;
            eval_int_literal_op!(op, *value, literal, side, Value::I32, "binary_int_literal")
        }
        Value::I64(value) => {
            let literal = int_literal_as(magnitude, i64::MAX as u64)? as i64;
            eval_int_literal_op!(op, *value, literal, side, Value::I64, "binary_int_literal")
        }
        Value::U8(value) => {
            let literal = int_literal_as(magnitude, u8::MAX as u64)? as u8;
            eval_int_literal_op!(op, *value, literal, side, Value::U8, "binary_int_literal")
        }
        Value::U16(value) => {
            let literal = int_literal_as(magnitude, u16::MAX as u64)? as u16;
            eval_int_literal_op!(op, *value, literal, side, Value::U16, "binary_int_literal")
        }
        Value::U32(value) => {
            let literal = int_literal_as(magnitude, u32::MAX as u64)? as u32;
            eval_int_literal_op!(op, *value, literal, side, Value::U32, "binary_int_literal")
        }
        Value::U64(value) => {
            let literal = int_literal_as(magnitude, u64::MAX)?;
            eval_int_literal_op!(op, *value, literal, side, Value::U64, "binary_int_literal")
        }
        _ => type_mismatch("binary_int_literal"),
    }
}

pub(crate) fn binary_float_literal_numeric(
    op: BinaryLiteralOp,
    value: &Value,
    literal: LinkedFloatLiteral,
    side: BinaryLiteralSide,
) -> VmResult<Value> {
    let LinkedFloatLiteral::Value { as_f32, as_f64 } = literal else {
        return type_mismatch("binary_float_literal");
    };
    match value {
        Value::F32(value) => {
            eval_float_literal_op!(op, *value, as_f32, side, Value::F32)
        }
        Value::F64(value) => {
            eval_float_literal_op!(op, *value, as_f64, side, Value::F64)
        }
        _ => type_mismatch("binary_float_literal"),
    }
}

/// Narrows a link-time magnitude to one integer target type's range.
///
/// The source text was parsed when the program linked; only the target-type
/// range check is left, because which type applies depends on the runtime
/// value. An unrepresentable literal fails here exactly as the earlier textual
/// parse failed.
#[inline]
fn int_literal_as(magnitude: LinkedIntLiteral, max: u64) -> VmResult<u64> {
    match magnitude {
        LinkedIntLiteral::Magnitude(value) if value <= max => Ok(value),
        _ => type_mismatch("binary_int_literal"),
    }
}

fn checked_div_rem<T>(
    lhs: T,
    rhs: T,
    ctor: impl FnOnce(T) -> Value,
    operation: &'static str,
    apply: impl FnOnce(T, T) -> Option<T>,
) -> VmResult<Value>
where
    T: Default + PartialEq,
{
    if rhs == T::default() {
        return Err(VmError::new(VmErrorKind::DivisionByZero));
    }
    apply(lhs, rhs)
        .map(ctor)
        .ok_or_else(|| arithmetic_overflow(operation))
}

#[inline]
fn arithmetic_overflow(operation: &'static str) -> VmError {
    VmError::new(VmErrorKind::ArithmeticOverflow { operation })
}

#[inline]
fn type_mismatch<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}
