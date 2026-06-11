use vela_common::ScalarValue;

use crate::path::HostRef;

#[derive(Clone, Debug, PartialEq)]
pub enum HostValue {
    Null,
    Bool(bool),
    Scalar(ScalarValue),
    String(String),
    HostRef(HostRef),
}

impl HostValue {
    #[must_use]
    pub const fn i64(value: i64) -> Self {
        Self::Scalar(ScalarValue::I64(value))
    }

    #[must_use]
    pub const fn f64(value: f64) -> Self {
        Self::Scalar(ScalarValue::F64(value))
    }
}

pub(crate) fn add_values(lhs: &HostValue, rhs: &HostValue) -> Option<HostValue> {
    match (lhs, rhs) {
        (HostValue::Scalar(ScalarValue::I64(lhs)), HostValue::Scalar(ScalarValue::I64(rhs))) => {
            Some(HostValue::i64(lhs + rhs))
        }
        (HostValue::Scalar(ScalarValue::F64(lhs)), HostValue::Scalar(ScalarValue::F64(rhs))) => {
            Some(HostValue::f64(lhs + rhs))
        }
        _ => None,
    }
}

pub(crate) fn sub_values(lhs: &HostValue, rhs: &HostValue) -> Option<HostValue> {
    match (lhs, rhs) {
        (HostValue::Scalar(ScalarValue::I64(lhs)), HostValue::Scalar(ScalarValue::I64(rhs))) => {
            Some(HostValue::i64(lhs - rhs))
        }
        (HostValue::Scalar(ScalarValue::F64(lhs)), HostValue::Scalar(ScalarValue::F64(rhs))) => {
            Some(HostValue::f64(lhs - rhs))
        }
        _ => None,
    }
}

pub(crate) fn mul_values(lhs: &HostValue, rhs: &HostValue) -> Option<HostValue> {
    match (lhs, rhs) {
        (HostValue::Scalar(ScalarValue::I64(lhs)), HostValue::Scalar(ScalarValue::I64(rhs))) => {
            lhs.checked_mul(*rhs).map(HostValue::i64)
        }
        (HostValue::Scalar(ScalarValue::F64(lhs)), HostValue::Scalar(ScalarValue::F64(rhs))) => {
            Some(HostValue::f64(lhs * rhs))
        }
        _ => None,
    }
}

pub(crate) fn div_values(lhs: &HostValue, rhs: &HostValue) -> Option<HostValue> {
    match (lhs, rhs) {
        (HostValue::Scalar(ScalarValue::I64(_)), HostValue::Scalar(ScalarValue::I64(0))) => None,
        (HostValue::Scalar(ScalarValue::I64(lhs)), HostValue::Scalar(ScalarValue::I64(rhs))) => {
            lhs.checked_div(*rhs).map(HostValue::i64)
        }
        (HostValue::Scalar(ScalarValue::F64(_)), HostValue::Scalar(ScalarValue::F64(rhs)))
            if *rhs == 0.0 =>
        {
            None
        }
        (HostValue::Scalar(ScalarValue::F64(lhs)), HostValue::Scalar(ScalarValue::F64(rhs))) => {
            Some(HostValue::f64(lhs / rhs))
        }
        _ => None,
    }
}

pub(crate) fn rem_values(lhs: &HostValue, rhs: &HostValue) -> Option<HostValue> {
    match (lhs, rhs) {
        (HostValue::Scalar(ScalarValue::I64(_)), HostValue::Scalar(ScalarValue::I64(0))) => None,
        (HostValue::Scalar(ScalarValue::I64(lhs)), HostValue::Scalar(ScalarValue::I64(rhs))) => {
            lhs.checked_rem(*rhs).map(HostValue::i64)
        }
        (HostValue::Scalar(ScalarValue::F64(_)), HostValue::Scalar(ScalarValue::F64(rhs)))
            if *rhs == 0.0 =>
        {
            None
        }
        (HostValue::Scalar(ScalarValue::F64(lhs)), HostValue::Scalar(ScalarValue::F64(rhs))) => {
            Some(HostValue::f64(lhs % rhs))
        }
        _ => None,
    }
}
