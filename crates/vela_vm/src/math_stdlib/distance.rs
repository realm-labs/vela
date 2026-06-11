use crate::VmResult;

use super::{OwnedValue, expect_arity};

use super::{expect_finite_float, type_error};

pub(crate) fn math_distance2d(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    expect_arity("math::distance2d", args, 4)?;
    let x1 = expect_finite_float(&args[0], "math::distance2d")?;
    let y1 = expect_finite_float(&args[1], "math::distance2d")?;
    let x2 = expect_finite_float(&args[2], "math::distance2d")?;
    let y2 = expect_finite_float(&args[3], "math::distance2d")?;
    let distance = (x2 - x1).hypot(y2 - y1);
    if distance.is_finite() {
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::F64(distance)))
    } else {
        type_error("math::distance2d")
    }
}

pub(crate) fn math_distance3d(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    expect_arity("math::distance3d", args, 6)?;
    let x1 = expect_finite_float(&args[0], "math::distance3d")?;
    let y1 = expect_finite_float(&args[1], "math::distance3d")?;
    let z1 = expect_finite_float(&args[2], "math::distance3d")?;
    let x2 = expect_finite_float(&args[3], "math::distance3d")?;
    let y2 = expect_finite_float(&args[4], "math::distance3d")?;
    let z2 = expect_finite_float(&args[5], "math::distance3d")?;
    let distance = (x2 - x1).hypot(y2 - y1).hypot(z2 - z1);
    if distance.is_finite() {
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::F64(distance)))
    } else {
        type_error("math::distance3d")
    }
}
