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

macro_rules! checked_int_binop {
    ($lhs:expr, $rhs:expr, $method:ident, $($variant:ident),* $(,)?) => {
        match ($lhs, $rhs) {
            $(
                (
                    HostValue::Scalar(ScalarValue::$variant(lhs)),
                    HostValue::Scalar(ScalarValue::$variant(rhs)),
                ) => lhs
                    .$method(*rhs)
                    .map(|value| HostValue::Scalar(ScalarValue::$variant(value))),
            )*
            _ => None,
        }
    };
}

macro_rules! checked_int_divop {
    ($lhs:expr, $rhs:expr, $method:ident, $($variant:ident),* $(,)?) => {
        match ($lhs, $rhs) {
            $(
                (
                    HostValue::Scalar(ScalarValue::$variant(_)),
                    HostValue::Scalar(ScalarValue::$variant(0)),
                ) => None,
                (
                    HostValue::Scalar(ScalarValue::$variant(lhs)),
                    HostValue::Scalar(ScalarValue::$variant(rhs)),
                ) => lhs
                    .$method(*rhs)
                    .map(|value| HostValue::Scalar(ScalarValue::$variant(value))),
            )*
            _ => None,
        }
    };
}

macro_rules! exact_numeric_binop {
    ($lhs:expr, $rhs:expr, $method:ident, $float_op:tt) => {
        checked_int_binop!(
            $lhs, $rhs, $method,
            I8, I16, I32, I64, U8, U16, U32, U64,
        )
        .or_else(|| match ($lhs, $rhs) {
            (
                HostValue::Scalar(ScalarValue::F32(lhs)),
                HostValue::Scalar(ScalarValue::F32(rhs)),
            ) => Some(HostValue::Scalar(ScalarValue::F32(lhs $float_op rhs))),
            (
                HostValue::Scalar(ScalarValue::F64(lhs)),
                HostValue::Scalar(ScalarValue::F64(rhs)),
            ) => Some(HostValue::Scalar(ScalarValue::F64(lhs $float_op rhs))),
            _ => None,
        })
    };
}

pub(crate) fn add_values(lhs: &HostValue, rhs: &HostValue) -> Option<HostValue> {
    exact_numeric_binop!(lhs, rhs, checked_add, +)
}

pub(crate) fn sub_values(lhs: &HostValue, rhs: &HostValue) -> Option<HostValue> {
    exact_numeric_binop!(lhs, rhs, checked_sub, -)
}

pub(crate) fn mul_values(lhs: &HostValue, rhs: &HostValue) -> Option<HostValue> {
    exact_numeric_binop!(lhs, rhs, checked_mul, *)
}

pub(crate) fn div_values(lhs: &HostValue, rhs: &HostValue) -> Option<HostValue> {
    checked_int_divop!(lhs, rhs, checked_div, I8, I16, I32, I64, U8, U16, U32, U64,).or_else(|| {
        match (lhs, rhs) {
            (HostValue::Scalar(ScalarValue::F32(_)), HostValue::Scalar(ScalarValue::F32(rhs)))
                if *rhs == 0.0 =>
            {
                None
            }
            (
                HostValue::Scalar(ScalarValue::F32(lhs)),
                HostValue::Scalar(ScalarValue::F32(rhs)),
            ) => Some(HostValue::Scalar(ScalarValue::F32(lhs / rhs))),
            (HostValue::Scalar(ScalarValue::F64(_)), HostValue::Scalar(ScalarValue::F64(rhs)))
                if *rhs == 0.0 =>
            {
                None
            }
            (
                HostValue::Scalar(ScalarValue::F64(lhs)),
                HostValue::Scalar(ScalarValue::F64(rhs)),
            ) => Some(HostValue::Scalar(ScalarValue::F64(lhs / rhs))),
            _ => None,
        }
    })
}

pub(crate) fn rem_values(lhs: &HostValue, rhs: &HostValue) -> Option<HostValue> {
    checked_int_divop!(lhs, rhs, checked_rem, I8, I16, I32, I64, U8, U16, U32, U64,).or_else(|| {
        match (lhs, rhs) {
            (HostValue::Scalar(ScalarValue::F32(_)), HostValue::Scalar(ScalarValue::F32(rhs)))
                if *rhs == 0.0 =>
            {
                None
            }
            (
                HostValue::Scalar(ScalarValue::F32(lhs)),
                HostValue::Scalar(ScalarValue::F32(rhs)),
            ) => Some(HostValue::Scalar(ScalarValue::F32(lhs % rhs))),
            (HostValue::Scalar(ScalarValue::F64(_)), HostValue::Scalar(ScalarValue::F64(rhs)))
                if *rhs == 0.0 =>
            {
                None
            }
            (
                HostValue::Scalar(ScalarValue::F64(lhs)),
                HostValue::Scalar(ScalarValue::F64(rhs)),
            ) => Some(HostValue::Scalar(ScalarValue::F64(lhs % rhs))),
            _ => None,
        }
    })
}
