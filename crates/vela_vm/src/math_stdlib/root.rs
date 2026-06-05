use crate::VmResult;

use super::{Value, expect_arity};

use super::{expect_finite_float, type_error};

pub(crate) fn math_sqrt(args: &[Value]) -> VmResult<Value> {
    expect_arity("math::sqrt", args, 1)?;
    let value = expect_finite_float(&args[0], "math::sqrt")?;
    if value < 0.0 {
        return type_error("math::sqrt");
    }

    let root = value.sqrt();
    if root.is_finite() {
        Ok(Value::Float(root))
    } else {
        type_error("math::sqrt")
    }
}
