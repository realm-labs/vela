use crate::{Value, VmError, VmErrorKind, VmResult};

pub(crate) fn binary_numeric(
    lhs: &Value,
    rhs: &Value,
    operation: &'static str,
    int_op: impl FnOnce(i64, i64) -> i64,
) -> VmResult<Value> {
    match (lhs, rhs) {
        (Value::Int(lhs), Value::Int(rhs)) => Ok(Value::Int(int_op(*lhs, *rhs))),
        (Value::Float(lhs), Value::Float(rhs)) => {
            Ok(Value::Float(int_op_float(*lhs, *rhs, operation)?))
        }
        _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
    }
}

pub(crate) fn negate_numeric(value: &Value) -> VmResult<Value> {
    match value {
        Value::Int(value) => value.checked_neg().map(Value::Int).ok_or_else(|| {
            VmError::new(VmErrorKind::TypeMismatch {
                operation: "negate",
            })
        }),
        Value::Float(value) => Ok(Value::Float(-value)),
        _ => Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "negate",
        })),
    }
}

pub(crate) fn div_numeric(lhs: &Value, rhs: &Value) -> VmResult<Value> {
    match (lhs, rhs) {
        (Value::Int(_), Value::Int(0)) => Err(VmError::new(VmErrorKind::DivisionByZero)),
        (Value::Int(lhs), Value::Int(rhs)) => Ok(Value::Int(lhs / rhs)),
        (Value::Float(_), Value::Float(rhs)) if *rhs == 0.0 => {
            Err(VmError::new(VmErrorKind::DivisionByZero))
        }
        (Value::Float(lhs), Value::Float(rhs)) => Ok(Value::Float(lhs / rhs)),
        _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation: "div" })),
    }
}

pub(crate) fn rem_numeric(lhs: &Value, rhs: &Value) -> VmResult<Value> {
    match (lhs, rhs) {
        (Value::Int(_), Value::Int(0)) => Err(VmError::new(VmErrorKind::DivisionByZero)),
        (Value::Int(lhs), Value::Int(rhs)) => Ok(Value::Int(lhs % rhs)),
        (Value::Float(_), Value::Float(rhs)) if *rhs == 0.0 => {
            Err(VmError::new(VmErrorKind::DivisionByZero))
        }
        (Value::Float(lhs), Value::Float(rhs)) => Ok(Value::Float(lhs % rhs)),
        _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation: "rem" })),
    }
}

pub(crate) fn compare_numeric(
    lhs: &Value,
    rhs: &Value,
    operation: &'static str,
    compare: impl FnOnce(f64, f64) -> bool,
) -> VmResult<bool> {
    match (lhs, rhs) {
        (Value::Int(lhs), Value::Int(rhs)) => Ok(compare(*lhs as f64, *rhs as f64)),
        (Value::Float(lhs), Value::Float(rhs)) => Ok(compare(*lhs, *rhs)),
        _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
    }
}

fn int_op_float(lhs: f64, rhs: f64, operation: &'static str) -> VmResult<f64> {
    match operation {
        "add" => Ok(lhs + rhs),
        "sub" => Ok(lhs - rhs),
        "mul" => Ok(lhs * rhs),
        _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
    }
}
