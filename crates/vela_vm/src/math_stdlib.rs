use crate::{Value, Vm, VmError, VmErrorKind, VmResult, expect_arity};

pub(crate) fn register(vm: &mut Vm) {
    vm.register_native("math.max", math_max);
    vm.register_native("math.min", math_min);
    vm.register_native("math.clamp", math_clamp);
    vm.register_native("math.lerp", math_lerp);
    vm.register_native("math.distance2d", math_distance2d);
    vm.register_native("math.distance3d", math_distance3d);
    vm.register_native("math.pow", math_pow);
    vm.register_native("math.sign", math_sign);
    vm.register_native("math.floor", math_floor);
    vm.register_native("math.ceil", math_ceil);
    vm.register_native("math.round", math_round);
    vm.register_native("math.abs", math_abs);
}

fn math_max(args: &[Value]) -> VmResult<Value> {
    numeric_pair("math.max", args, i64::max, f64::max)
}

fn math_min(args: &[Value]) -> VmResult<Value> {
    numeric_pair("math.min", args, i64::min, f64::min)
}

fn math_clamp(args: &[Value]) -> VmResult<Value> {
    expect_arity("math.clamp", args, 3)?;
    match (&args[0], &args[1], &args[2]) {
        (Value::Int(value), Value::Int(min), Value::Int(max)) => {
            if min > max {
                return type_error("math.clamp");
            }
            Ok(Value::Int((*value).clamp(*min, *max)))
        }
        _ => {
            let value = expect_finite_float(&args[0], "math.clamp")?;
            let min = expect_finite_float(&args[1], "math.clamp")?;
            let max = expect_finite_float(&args[2], "math.clamp")?;
            if min > max {
                return type_error("math.clamp");
            }
            Ok(Value::Float(value.clamp(min, max)))
        }
    }
}

fn math_lerp(args: &[Value]) -> VmResult<Value> {
    expect_arity("math.lerp", args, 3)?;
    let start = expect_finite_float(&args[0], "math.lerp")?;
    let end = expect_finite_float(&args[1], "math.lerp")?;
    let t = expect_finite_float(&args[2], "math.lerp")?;
    Ok(Value::Float(start + (end - start) * t))
}

fn math_distance2d(args: &[Value]) -> VmResult<Value> {
    expect_arity("math.distance2d", args, 4)?;
    let x1 = expect_finite_float(&args[0], "math.distance2d")?;
    let y1 = expect_finite_float(&args[1], "math.distance2d")?;
    let x2 = expect_finite_float(&args[2], "math.distance2d")?;
    let y2 = expect_finite_float(&args[3], "math.distance2d")?;
    let distance = (x2 - x1).hypot(y2 - y1);
    if distance.is_finite() {
        Ok(Value::Float(distance))
    } else {
        type_error("math.distance2d")
    }
}

fn math_distance3d(args: &[Value]) -> VmResult<Value> {
    expect_arity("math.distance3d", args, 6)?;
    let x1 = expect_finite_float(&args[0], "math.distance3d")?;
    let y1 = expect_finite_float(&args[1], "math.distance3d")?;
    let z1 = expect_finite_float(&args[2], "math.distance3d")?;
    let x2 = expect_finite_float(&args[3], "math.distance3d")?;
    let y2 = expect_finite_float(&args[4], "math.distance3d")?;
    let z2 = expect_finite_float(&args[5], "math.distance3d")?;
    let distance = (x2 - x1).hypot(y2 - y1).hypot(z2 - z1);
    if distance.is_finite() {
        Ok(Value::Float(distance))
    } else {
        type_error("math.distance3d")
    }
}

fn math_pow(args: &[Value]) -> VmResult<Value> {
    expect_arity("math.pow", args, 2)?;
    if let (Value::Int(base), Value::Int(exponent)) = (&args[0], &args[1]) {
        if *exponent < 0 {
            return numeric_pow(args);
        }
        let exponent = u32::try_from(*exponent).map_err(|_| {
            VmError::new(VmErrorKind::TypeMismatch {
                operation: "math.pow",
            })
        })?;
        return base.checked_pow(exponent).map(Value::Int).ok_or_else(|| {
            VmError::new(VmErrorKind::TypeMismatch {
                operation: "math.pow",
            })
        });
    }

    numeric_pow(args)
}

fn math_sign(args: &[Value]) -> VmResult<Value> {
    expect_arity("math.sign", args, 1)?;
    match &args[0] {
        Value::Int(value) => Ok(Value::Int(value.signum())),
        Value::Float(value) if value.is_finite() => Ok(Value::Int(if *value > 0.0 {
            1
        } else if *value < 0.0 {
            -1
        } else {
            0
        })),
        _ => type_error("math.sign"),
    }
}

fn numeric_pow(args: &[Value]) -> VmResult<Value> {
    let base = expect_finite_float(&args[0], "math.pow")?;
    let exponent = expect_finite_float(&args[1], "math.pow")?;
    let value = base.powf(exponent);
    if value.is_finite() {
        Ok(Value::Float(value))
    } else {
        type_error("math.pow")
    }
}

fn math_floor(args: &[Value]) -> VmResult<Value> {
    expect_arity("math.floor", args, 1)?;
    match &args[0] {
        Value::Int(value) => Ok(Value::Int(*value)),
        Value::Float(value) => float_to_int(value.floor(), "math.floor").map(Value::Int),
        _ => type_error("math.floor"),
    }
}

fn math_ceil(args: &[Value]) -> VmResult<Value> {
    expect_arity("math.ceil", args, 1)?;
    match &args[0] {
        Value::Int(value) => Ok(Value::Int(*value)),
        Value::Float(value) => float_to_int(value.ceil(), "math.ceil").map(Value::Int),
        _ => type_error("math.ceil"),
    }
}

fn math_round(args: &[Value]) -> VmResult<Value> {
    expect_arity("math.round", args, 1)?;
    match &args[0] {
        Value::Int(value) => Ok(Value::Int(*value)),
        Value::Float(value) => float_to_int(value.round(), "math.round").map(Value::Int),
        _ => type_error("math.round"),
    }
}

fn math_abs(args: &[Value]) -> VmResult<Value> {
    expect_arity("math.abs", args, 1)?;
    match &args[0] {
        Value::Int(value) => value.checked_abs().map(Value::Int).ok_or_else(|| {
            VmError::new(VmErrorKind::TypeMismatch {
                operation: "math.abs",
            })
        }),
        Value::Float(value) if value.is_finite() => Ok(Value::Float(value.abs())),
        _ => type_error("math.abs"),
    }
}

fn numeric_pair(
    name: &'static str,
    args: &[Value],
    int_op: impl FnOnce(i64, i64) -> i64,
    float_op: impl FnOnce(f64, f64) -> f64,
) -> VmResult<Value> {
    expect_arity(name, args, 2)?;
    match (&args[0], &args[1]) {
        (Value::Int(lhs), Value::Int(rhs)) => Ok(Value::Int(int_op(*lhs, *rhs))),
        _ => {
            let lhs = expect_finite_float(&args[0], name)?;
            let rhs = expect_finite_float(&args[1], name)?;
            Ok(Value::Float(float_op(lhs, rhs)))
        }
    }
}

fn expect_finite_float(value: &Value, operation: &'static str) -> VmResult<f64> {
    match value {
        Value::Int(value) => Ok(*value as f64),
        Value::Float(value) if value.is_finite() => Ok(*value),
        _ => type_error(operation),
    }
}

fn float_to_int(value: f64, operation: &'static str) -> VmResult<i64> {
    if value.is_finite() && value >= i64::MIN as f64 && value <= i64::MAX as f64 {
        Ok(value as i64)
    } else {
        type_error(operation)
    }
}

fn type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}

#[cfg(test)]
mod tests {
    use vela_bytecode::compiler::compile_function_source;
    use vela_common::SourceId;

    use crate::{ExecutionBudget, Value, Vm};

    #[test]
    fn runs_compiled_math_distance2d() {
        let source = r#"
fn main() {
    let distance = math.distance2d(0, 0, 3, 4);
    if distance == 5.0 && math.distance2d(-1.5, 2.0, -1.5, 5.0) == 3.0 {
        return math.round(distance);
    }
    return 0;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("math distance2d source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm.run(&code).expect("math distance2d should run");
        assert_eq!(result, Value::Int(5));
    }

    #[test]
    fn runs_compiled_math_distance3d() {
        let source = r#"
fn main() {
    let distance = math.distance3d(0, 0, 0, 2, 3, 6);
    if distance == 7.0 && math.distance3d(-1.5, 2.0, 4.0, -1.5, 5.0, 8.0) == 5.0 {
        return math.round(distance);
    }
    return 0;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("math distance3d source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm.run(&code).expect("math distance3d should run");
        assert_eq!(result, Value::Int(7));
    }

    #[test]
    fn runs_compiled_math_pow() {
        let source = r#"
fn main() {
    if math.pow(2, 10) == 1024 && math.pow(9, 0.5) == 3.0 {
        return math.pow(2, 3);
    }
    return 0;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("math pow source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm.run(&code).expect("math pow should run");
        assert_eq!(result, Value::Int(8));
    }

    #[test]
    fn runs_compiled_math_sign() {
        let source = r#"
fn main() {
    return math.sign(-12)
        + math.sign(0)
        + math.sign(3.5)
        + math.sign(-0.0);
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("math sign source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm.run(&code).expect("math sign should run");
        assert_eq!(result, Value::Int(0));
    }

    #[test]
    fn managed_heap_execution_runs_math_distance2d() {
        let source = r#"
fn main() {
    return math.distance2d(2, 4, 8, 12) == 10.0;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap math distance2d source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();
        let mut budget = ExecutionBudget::unbounded();

        let result = vm
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap math distance2d should run");
        assert_eq!(result, Value::Bool(true));
    }

    #[test]
    fn managed_heap_execution_runs_math_distance3d() {
        let source = r#"
fn main() {
    return math.distance3d(1, 2, 3, 4, 6, 15) == 13.0;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap math distance3d source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();
        let mut budget = ExecutionBudget::unbounded();

        let result = vm
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap math distance3d should run");
        assert_eq!(result, Value::Bool(true));
    }

    #[test]
    fn managed_heap_execution_runs_math_pow() {
        let source = r#"
fn main() {
    return math.pow(16, 0.5) == 4.0 && math.pow(3, 4) == 81;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap math pow source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();
        let mut budget = ExecutionBudget::unbounded();

        let result = vm
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap math pow should run");
        assert_eq!(result, Value::Bool(true));
    }

    #[test]
    fn managed_heap_execution_runs_math_sign() {
        let source = r#"
fn main() {
    return math.sign(-2.5) == -1 && math.sign(0.0) == 0 && math.sign(8) == 1;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap math sign source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();
        let mut budget = ExecutionBudget::unbounded();

        let result = vm
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap math sign should run");
        assert_eq!(result, Value::Bool(true));
    }

    #[test]
    fn math_distance2d_rejects_non_numeric_values() {
        let source = r#"
fn main() {
    return math.distance2d(0, 0, "x", 1);
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("math distance2d type error source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let error = vm
            .run(&code)
            .expect_err("math distance2d should reject non-numeric values");
        assert_eq!(
            error.kind,
            crate::VmErrorKind::TypeMismatch {
                operation: "math.distance2d"
            }
        );
    }

    #[test]
    fn math_pow_rejects_non_numeric_values() {
        let source = r#"
fn main() {
    return math.pow("xp", 2);
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("math pow type error source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let error = vm
            .run(&code)
            .expect_err("math pow should reject non-numeric values");
        assert_eq!(
            error.kind,
            crate::VmErrorKind::TypeMismatch {
                operation: "math.pow"
            }
        );
    }

    #[test]
    fn math_sign_rejects_non_numeric_values() {
        let source = r#"
fn main() {
    return math.sign("left");
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("math sign type error source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let error = vm
            .run(&code)
            .expect_err("math sign should reject non-numeric values");
        assert_eq!(
            error.kind,
            crate::VmErrorKind::TypeMismatch {
                operation: "math.sign"
            }
        );
    }

    #[test]
    fn math_pow_rejects_non_finite_results() {
        let source = r#"
fn main() {
    return math.pow(0, -1);
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("math pow non-finite source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let error = vm
            .run(&code)
            .expect_err("math pow should reject non-finite results");
        assert_eq!(
            error.kind,
            crate::VmErrorKind::TypeMismatch {
                operation: "math.pow"
            }
        );
    }

    #[test]
    fn math_distance3d_rejects_non_numeric_values() {
        let source = r#"
fn main() {
    return math.distance3d(0, 0, 0, 1, "y", 1);
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("math distance3d type error source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let error = vm
            .run(&code)
            .expect_err("math distance3d should reject non-numeric values");
        assert_eq!(
            error.kind,
            crate::VmErrorKind::TypeMismatch {
                operation: "math.distance3d"
            }
        );
    }
}
