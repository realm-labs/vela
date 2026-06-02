use crate::{Value, VmError, VmErrorKind, VmResult, expect_arity};

use super::{expect_finite_float, type_error};

pub(crate) fn math_pow(args: &[Value]) -> VmResult<Value> {
    expect_arity("math::pow", args, 2)?;
    if let (Value::Int(base), Value::Int(exponent)) = (&args[0], &args[1]) {
        if *exponent < 0 {
            return numeric_pow(args);
        }
        let exponent = u32::try_from(*exponent).map_err(|_| {
            VmError::new(VmErrorKind::TypeMismatch {
                operation: "math::pow",
            })
        })?;
        return base.checked_pow(exponent).map(Value::Int).ok_or_else(|| {
            VmError::new(VmErrorKind::TypeMismatch {
                operation: "math::pow",
            })
        });
    }

    numeric_pow(args)
}

fn numeric_pow(args: &[Value]) -> VmResult<Value> {
    let base = expect_finite_float(&args[0], "math::pow")?;
    let exponent = expect_finite_float(&args[1], "math::pow")?;
    let value = base.powf(exponent);
    if value.is_finite() {
        Ok(Value::Float(value))
    } else {
        type_error("math::pow")
    }
}
