use crate::{Value, VmError, VmErrorKind, VmResult};

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

#[inline]
fn type_mismatch<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}
