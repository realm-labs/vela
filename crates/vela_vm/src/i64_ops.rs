use vela_common::ScalarValue;

use crate::{Value, VmError, VmErrorKind, VmResult};

#[inline]
pub(crate) fn add(lhs: i64, rhs: i64) -> VmResult<Value> {
    lhs.checked_add(rhs)
        .map(i64_value)
        .ok_or_else(|| overflow("add"))
}

#[inline]
pub(crate) fn sub(lhs: i64, rhs: i64) -> VmResult<Value> {
    lhs.checked_sub(rhs)
        .map(i64_value)
        .ok_or_else(|| overflow("sub"))
}

#[inline]
pub(crate) fn mul(lhs: i64, rhs: i64) -> VmResult<Value> {
    lhs.checked_mul(rhs)
        .map(i64_value)
        .ok_or_else(|| overflow("mul"))
}

#[inline]
pub(crate) fn rem(lhs: i64, rhs: i64) -> VmResult<Value> {
    if rhs == 0 {
        return Err(VmError::new(VmErrorKind::DivisionByZero));
    }
    lhs.checked_rem(rhs)
        .map(i64_value)
        .ok_or_else(|| overflow("rem"))
}

#[inline]
pub(crate) fn eq_imm(lhs: i64, imm: i64) -> Value {
    Value::Bool(lhs == imm)
}

#[inline]
pub(crate) fn gt_imm(lhs: i64, imm: i64) -> Value {
    Value::Bool(lhs > imm)
}

#[inline]
pub(crate) fn read(value: &Value, operation: &'static str) -> VmResult<i64> {
    match value {
        Value::Scalar(ScalarValue::I64(value)) => Ok(*value),
        _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
    }
}

#[inline]
fn i64_value(value: i64) -> Value {
    Value::Scalar(ScalarValue::I64(value))
}

#[inline]
fn overflow(operation: &'static str) -> VmError {
    VmError::new(VmErrorKind::ArithmeticOverflow { operation })
}
