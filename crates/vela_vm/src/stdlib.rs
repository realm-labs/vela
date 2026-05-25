use crate::script_object::ScriptFields;
use crate::{Value, Vm, VmError, VmErrorKind, VmResult, expect_arity};

pub(crate) fn register(vm: &mut Vm) {
    vm.register_native("option.some", option_some);
    vm.register_native("option.none", option_none);
    vm.register_native("option.is_some", option_is_some);
    vm.register_native("option.is_none", option_is_none);
    vm.register_native("option.unwrap_or", option_unwrap_or);
    vm.register_native("result.ok", result_ok);
    vm.register_native("result.err", result_err);
    vm.register_native("result.is_ok", result_is_ok);
    vm.register_native("result.is_err", result_is_err);
    vm.register_native("result.unwrap_or", result_unwrap_or);
    vm.register_native("math.max", math_max);
    vm.register_native("math.min", math_min);
    vm.register_native("math.clamp", math_clamp);
    vm.register_native("math.floor", math_floor);
    vm.register_native("math.ceil", math_ceil);
    vm.register_native("math.abs", math_abs);
    vm.register_native("set.from_array", crate::set_methods::from_array);
}

fn option_some(args: &[Value]) -> VmResult<Value> {
    expect_arity("option.some", args, 1)?;
    Ok(enum_value("Option", "Some", Some(args[0].clone())))
}

fn option_none(args: &[Value]) -> VmResult<Value> {
    expect_arity("option.none", args, 0)?;
    Ok(enum_value("Option", "None", None))
}

fn option_is_some(args: &[Value]) -> VmResult<Value> {
    expect_arity("option.is_some", args, 1)?;
    option_variant(&args[0], "option.is_some").map(|variant| Value::Bool(variant == "Some"))
}

fn option_is_none(args: &[Value]) -> VmResult<Value> {
    expect_arity("option.is_none", args, 1)?;
    option_variant(&args[0], "option.is_none").map(|variant| Value::Bool(variant == "None"))
}

fn option_unwrap_or(args: &[Value]) -> VmResult<Value> {
    expect_arity("option.unwrap_or", args, 2)?;
    match option_variant(&args[0], "option.unwrap_or")? {
        "Some" => enum_payload(&args[0], "option.unwrap_or"),
        "None" => Ok(args[1].clone()),
        _ => type_error("option.unwrap_or"),
    }
}

fn result_ok(args: &[Value]) -> VmResult<Value> {
    expect_arity("result.ok", args, 1)?;
    Ok(enum_value("Result", "Ok", Some(args[0].clone())))
}

fn result_err(args: &[Value]) -> VmResult<Value> {
    expect_arity("result.err", args, 1)?;
    Ok(enum_value("Result", "Err", Some(args[0].clone())))
}

fn result_is_ok(args: &[Value]) -> VmResult<Value> {
    expect_arity("result.is_ok", args, 1)?;
    result_variant(&args[0], "result.is_ok").map(|variant| Value::Bool(variant == "Ok"))
}

fn result_is_err(args: &[Value]) -> VmResult<Value> {
    expect_arity("result.is_err", args, 1)?;
    result_variant(&args[0], "result.is_err").map(|variant| Value::Bool(variant == "Err"))
}

fn result_unwrap_or(args: &[Value]) -> VmResult<Value> {
    expect_arity("result.unwrap_or", args, 2)?;
    match result_variant(&args[0], "result.unwrap_or")? {
        "Ok" => enum_payload(&args[0], "result.unwrap_or"),
        "Err" => Ok(args[1].clone()),
        _ => type_error("result.unwrap_or"),
    }
}

fn enum_value(enum_name: &str, variant: &str, payload: Option<Value>) -> Value {
    let fields = payload
        .map(|payload| vec![("0".to_owned(), payload)])
        .unwrap_or_default();
    Value::Enum {
        enum_name: enum_name.to_owned(),
        variant: variant.to_owned(),
        fields: ScriptFields::from_pairs(&format!("{enum_name}.{variant}"), fields),
    }
}

fn option_variant<'a>(value: &'a Value, operation: &'static str) -> VmResult<&'a str> {
    let (enum_name, variant) =
        enum_tag(value).ok_or_else(|| VmError::new(VmErrorKind::TypeMismatch { operation }))?;
    if enum_name == "Option" || enum_name.rsplit('.').next() == Some("Option") {
        return Ok(variant);
    }
    type_error(operation)
}

fn result_variant<'a>(value: &'a Value, operation: &'static str) -> VmResult<&'a str> {
    let (enum_name, variant) =
        enum_tag(value).ok_or_else(|| VmError::new(VmErrorKind::TypeMismatch { operation }))?;
    if enum_name == "Result" || enum_name.rsplit('.').next() == Some("Result") {
        return Ok(variant);
    }
    type_error(operation)
}

fn enum_tag(value: &Value) -> Option<(&str, &str)> {
    match value {
        Value::Enum {
            enum_name, variant, ..
        } => Some((enum_name.as_str(), variant.as_str())),
        _ => None,
    }
}

fn enum_payload(value: &Value, operation: &'static str) -> VmResult<Value> {
    let Value::Enum { fields, .. } = value else {
        return type_error(operation);
    };
    fields
        .get("0")
        .cloned()
        .ok_or_else(|| VmError::new(VmErrorKind::TypeMismatch { operation }))
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
    use vela_bytecode::compiler::{compile_function_source, compile_program_source};
    use vela_common::SourceId;

    use crate::{ExecutionBudget, Vm, VmErrorKind};

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

    #[test]
    fn runs_compiled_option_result_standard_natives_with_try() {
        let source = r#"
fn checked(value) {
    if value > 0 {
        return option.some(value);
    }
    return option.none();
}

fn main() {
    let value = checked(4)?;
    return result.ok(value + 6);
}
"#;

        let program = compile_program_source(SourceId::new(1), source)
            .expect("option/result stdlib source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm
            .run_program(&program, "main", &[])
            .expect("option/result stdlib source should run");
        assert_eq!(
            result,
            crate::Value::Enum {
                enum_name: "Result".to_owned(),
                variant: "Ok".to_owned(),
                fields: [("0".to_owned(), crate::Value::Int(10))].into()
            }
        );
    }

    #[test]
    fn runs_compiled_option_result_standard_helper_natives() {
        let source = r#"
fn main() {
    let some = option.some(4);
    let none = option.none();
    let ok = result.ok(9);
    let err = result.err("missing");

    if option.is_some(some)
        && option.is_none(none)
        && result.is_ok(ok)
        && result.is_err(err)
    {
        return option.unwrap_or(some, 0)
            + option.unwrap_or(none, 5)
            + result.unwrap_or(ok, 0)
            + result.unwrap_or(err, 7);
    }
    return 0;
}
"#;

        let program = compile_program_source(SourceId::new(1), source)
            .expect("option/result helper stdlib source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm
            .run_program(&program, "main", &[])
            .expect("option/result helper stdlib source should run");
        assert_eq!(result, crate::Value::Int(25));
    }

    #[test]
    fn managed_heap_execution_runs_result_standard_natives_with_try() {
        let source = r#"
fn checked(value) {
    if value > 0 {
        return result.ok("good");
    }
    return result.err("bad");
}

fn main() {
    let value = checked(0)?;
    return result.ok(value);
}
"#;

        let program = compile_program_source(SourceId::new(1), source)
            .expect("heap result stdlib source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();
        let mut budget = ExecutionBudget::unbounded();

        let result = vm
            .run_program_with_managed_heap_and_budget(&program, "main", &[], &mut budget)
            .expect("heap result stdlib source should run");
        assert_eq!(
            result,
            crate::Value::Enum {
                enum_name: "Result".to_owned(),
                variant: "Err".to_owned(),
                fields: [("0".to_owned(), crate::Value::String("bad".to_owned()))].into()
            }
        );
    }

    #[test]
    fn managed_heap_execution_runs_option_result_standard_helper_natives() {
        let source = r#"
fn main() {
    let some = option.some("quest");
    let none = option.none();
    let ok = result.ok("done");
    let err = result.err("blocked");

    return option.is_some(some)
        && option.is_none(none)
        && result.is_ok(ok)
        && result.is_err(err)
        && option.unwrap_or(some, "fallback") == "quest"
        && option.unwrap_or(none, "fallback") == "fallback"
        && result.unwrap_or(ok, "fallback") == "done"
        && result.unwrap_or(err, "fallback") == "fallback";
}
"#;

        let program = compile_program_source(SourceId::new(1), source)
            .expect("heap option/result helper stdlib source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();
        let mut budget = ExecutionBudget::unbounded();

        let result = vm
            .run_program_with_managed_heap_and_budget(&program, "main", &[], &mut budget)
            .expect("heap option/result helper stdlib source should run");
        assert_eq!(result, crate::Value::Bool(true));
    }

    #[test]
    fn option_result_helpers_reject_wrong_dynamic_shapes() {
        let source = r#"
fn main() {
    return option.unwrap_or(result.ok(1), 0);
}
"#;

        let program = compile_program_source(SourceId::new(1), source)
            .expect("option/result helper type-error source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let error = vm
            .run_program(&program, "main", &[])
            .expect_err("option helper should reject Result values");
        assert_eq!(
            error.kind,
            VmErrorKind::TypeMismatch {
                operation: "option.unwrap_or"
            }
        );
    }

    #[test]
    fn runs_compiled_set_standard_natives_and_methods() {
        let source = r#"
fn main() {
    let tags = set.from_array(["fire", "ice", "fire"]);
    let added = tags.add("arcane");
    let duplicate = tags.add("ice");
    let removed = tags.remove("fire");
    let values = tags.values().sort_by(|tag| tag);
    if tags.len() == 2
        && added
        && !duplicate
        && removed
        && !tags.has("fire")
        && tags.has("arcane")
        && values[0] == "arcane"
        && values[1] == "ice"
    {
        return values.len();
    }
    return 0;
}
"#;

        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("set stdlib source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm.run(&code).expect("set stdlib source should run");
        assert_eq!(result, crate::Value::Int(2));
    }

    #[test]
    fn managed_heap_execution_runs_set_standard_natives_and_iteration() {
        let source = r#"
fn main() {
    let ids = set.from_array([1, 2, 2, 3]);
    ids.add(4);
    ids.remove(2);
    let total = 0;
    for id in ids {
        total += id;
    }
    return total;
}
"#;

        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap set stdlib source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();
        let mut budget = ExecutionBudget::unbounded();

        let result = vm
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap set stdlib source should run");
        assert_eq!(result, crate::Value::Int(8));
    }

    #[test]
    fn set_from_array_rejects_non_scalar_elements() {
        let source = r#"
fn main() {
    return set.from_array([[1]]);
}
"#;

        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("set type error source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let error = vm
            .run(&code)
            .expect_err("set.from_array should reject non-scalar elements");
        assert_eq!(
            error.kind,
            VmErrorKind::TypeMismatch {
                operation: "set.from_array"
            }
        );
    }
}
