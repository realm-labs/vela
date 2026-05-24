use crate::{Value, Vm, VmError, VmErrorKind, VmResult, expect_arity};

pub(crate) fn register(vm: &mut Vm) {
    vm.register_native("math.max", math_max);
    vm.register_native("math.min", math_min);
    vm.register_native("math.clamp", math_clamp);
    vm.register_native("math.floor", math_floor);
    vm.register_native("math.ceil", math_ceil);
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

    use crate::{ExecutionBudget, Vm};

    #[test]
    fn runs_compiled_math_standard_natives() {
        let source = r#"
fn main() {
    let clamped = math.clamp(15, 1, 10);
    let rounded = math.floor(3.9) + math.ceil(2.1);
    return math.max(clamped, math.abs(-4))
        + math.min(rounded, 10);
}
"#;

        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("math stdlib source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm.run(&code).expect("math stdlib source should run");
        assert_eq!(result, crate::Value::Int(16));
    }

    #[test]
    fn managed_heap_execution_runs_math_standard_natives() {
        let source = r#"
fn main() {
    let value = math.max(1.5, math.min(4.5, 3.25));
    let bounded = math.clamp(value, 2.0, 3.0);
    return bounded == 3.0 && math.abs(-2.5) == 2.5;
}
"#;

        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap math stdlib source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();
        let mut budget = ExecutionBudget::unbounded();

        let result = vm
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap math stdlib source should run");
        assert_eq!(result, crate::Value::Bool(true));
    }
}
