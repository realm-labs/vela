use crate::{VmError, VmErrorKind, VmResult};

use super::{OwnedValue, expect_arity};

use super::{expect_finite_float, type_error};

pub(crate) fn math_max(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    numeric_pair("math::max", args, i64::max, f64::max)
}

pub(crate) fn math_min(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    numeric_pair("math::min", args, i64::min, f64::min)
}

pub(crate) fn math_clamp(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    expect_arity("math::clamp", args, 3)?;
    match (&args[0], &args[1], &args[2]) {
        (
            OwnedValue::Scalar(vela_common::ScalarValue::I64(value)),
            OwnedValue::Scalar(vela_common::ScalarValue::I64(min)),
            OwnedValue::Scalar(vela_common::ScalarValue::I64(max)),
        ) => {
            if min > max {
                return type_error("math::clamp");
            }
            Ok(OwnedValue::i64((*value).clamp(*min, *max)))
        }
        _ => {
            let value = expect_finite_float(&args[0], "math::clamp")?;
            let min = expect_finite_float(&args[1], "math::clamp")?;
            let max = expect_finite_float(&args[2], "math::clamp")?;
            if min > max {
                return type_error("math::clamp");
            }
            Ok(OwnedValue::f64(value.clamp(min, max)))
        }
    }
}

pub(crate) fn math_sign(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    expect_arity("math::sign", args, 1)?;
    match &args[0] {
        OwnedValue::Scalar(vela_common::ScalarValue::I64(value)) => {
            Ok(OwnedValue::i64(value.signum()))
        }
        OwnedValue::Scalar(vela_common::ScalarValue::F64(value)) if value.is_finite() => {
            Ok(OwnedValue::i64(if *value > 0.0 {
                1
            } else if *value < 0.0 {
                -1
            } else {
                0
            }))
        }
        _ => type_error("math::sign"),
    }
}

pub(crate) fn math_floor(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    expect_arity("math::floor", args, 1)?;
    match &args[0] {
        OwnedValue::Scalar(vela_common::ScalarValue::I64(value)) => {
            Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(*value)))
        }
        OwnedValue::Scalar(vela_common::ScalarValue::F64(value)) => {
            float_to_int(value.floor(), "math::floor").map(OwnedValue::i64)
        }
        _ => type_error("math::floor"),
    }
}

pub(crate) fn math_ceil(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    expect_arity("math::ceil", args, 1)?;
    match &args[0] {
        OwnedValue::Scalar(vela_common::ScalarValue::I64(value)) => {
            Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(*value)))
        }
        OwnedValue::Scalar(vela_common::ScalarValue::F64(value)) => {
            float_to_int(value.ceil(), "math::ceil").map(OwnedValue::i64)
        }
        _ => type_error("math::ceil"),
    }
}

pub(crate) fn math_round(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    expect_arity("math::round", args, 1)?;
    match &args[0] {
        OwnedValue::Scalar(vela_common::ScalarValue::I64(value)) => {
            Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(*value)))
        }
        OwnedValue::Scalar(vela_common::ScalarValue::F64(value)) => {
            float_to_int(value.round(), "math::round").map(OwnedValue::i64)
        }
        _ => type_error("math::round"),
    }
}

pub(crate) fn math_abs(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    expect_arity("math::abs", args, 1)?;
    match &args[0] {
        OwnedValue::Scalar(vela_common::ScalarValue::I64(value)) => {
            value.checked_abs().map(OwnedValue::i64).ok_or_else(|| {
                VmError::new(VmErrorKind::TypeMismatch {
                    operation: "math::abs",
                })
            })
        }
        OwnedValue::Scalar(vela_common::ScalarValue::F64(value)) if value.is_finite() => {
            Ok(OwnedValue::f64(value.abs()))
        }
        _ => type_error("math::abs"),
    }
}

fn numeric_pair(
    name: &'static str,
    args: &[OwnedValue],
    int_op: impl FnOnce(i64, i64) -> i64,
    float_op: impl FnOnce(f64, f64) -> f64,
) -> VmResult<OwnedValue> {
    expect_arity(name, args, 2)?;
    match (&args[0], &args[1]) {
        (
            OwnedValue::Scalar(vela_common::ScalarValue::I64(lhs)),
            OwnedValue::Scalar(vela_common::ScalarValue::I64(rhs)),
        ) => Ok(OwnedValue::i64(int_op(*lhs, *rhs))),
        _ => {
            let lhs = expect_finite_float(&args[0], name)?;
            let rhs = expect_finite_float(&args[1], name)?;
            Ok(OwnedValue::f64(float_op(lhs, rhs)))
        }
    }
}

fn float_to_int(value: f64, operation: &'static str) -> VmResult<i64> {
    if value.is_finite() && value >= i64::MIN as f64 && value <= i64::MAX as f64 {
        Ok(value as i64)
    } else {
        type_error(operation)
    }
}
