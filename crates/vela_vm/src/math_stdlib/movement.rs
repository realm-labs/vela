use crate::{VmError, VmErrorKind, VmResult};

use super::{OwnedValue, expect_arity};

use super::{expect_finite_float, type_error};

pub(crate) fn math_lerp(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    expect_arity("math::lerp", args, 3)?;
    let start = expect_finite_float(&args[0], "math::lerp")?;
    let end = expect_finite_float(&args[1], "math::lerp")?;
    let t = expect_finite_float(&args[2], "math::lerp")?;
    let value = start + (end - start) * t;
    if value.is_finite() {
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::F64(value)))
    } else {
        type_error("math::lerp")
    }
}

pub(crate) fn math_move_towards(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    expect_arity("math::move_towards", args, 3)?;
    match (&args[0], &args[1], &args[2]) {
        (
            OwnedValue::Scalar(vela_common::ScalarValue::I64(current)),
            OwnedValue::Scalar(vela_common::ScalarValue::I64(target)),
            OwnedValue::Scalar(vela_common::ScalarValue::I64(max_delta)),
        ) => int_move_towards(*current, *target, *max_delta).map(OwnedValue::i64),
        _ => float_move_towards(args).map(OwnedValue::f64),
    }
}

fn int_move_towards(current: i64, target: i64, max_delta: i64) -> VmResult<i64> {
    if max_delta < 0 {
        return type_error("math::move_towards");
    }

    let delta = i128::from(target) - i128::from(current);
    if delta.unsigned_abs() <= max_delta as u128 {
        return Ok(target);
    }

    let step = delta.signum() * i128::from(max_delta);
    let value = i128::from(current) + step;
    i64::try_from(value).map_err(|_| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "math::move_towards",
        })
    })
}

fn float_move_towards(args: &[OwnedValue]) -> VmResult<f64> {
    let current = expect_finite_float(&args[0], "math::move_towards")?;
    let target = expect_finite_float(&args[1], "math::move_towards")?;
    let max_delta = expect_finite_float(&args[2], "math::move_towards")?;
    if max_delta < 0.0 {
        return type_error("math::move_towards");
    }

    let delta = target - current;
    if delta.abs() <= max_delta {
        return Ok(target);
    }

    let value = current + delta.signum() * max_delta;
    if value.is_finite() {
        Ok(value)
    } else {
        type_error("math::move_towards")
    }
}
