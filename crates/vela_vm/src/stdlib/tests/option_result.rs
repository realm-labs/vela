use super::*;
use crate::owned_value::OwnedValue as Value;

#[test]
fn runs_compiled_math_standard_natives() {
    let source = r#"
fn main() {
    let clamped = math::clamp(15, 1, 10);
    let rounded = math::floor(3.9) + math::ceil(2.1);
    let midpoint = math::floor(math::lerp(10, 20, 0.5));
    let powered = math::pow(2, 3);
    return math::max(clamped, math::abs(-4))
        + math::min(rounded, 10)
        + math::round(2.5)
        + midpoint
        + powered;
}
"#;

    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("math stdlib source should compile");
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let result = vm.run(&code).expect("math stdlib source should run");
    assert_eq!(result, Value::Int(42));
}

#[test]
fn managed_heap_execution_runs_math_standard_natives() {
    let source = r#"
fn main() {
    let value = math::max(1.5, math::min(4.5, 3.25));
    let bounded = math::clamp(value, 2.0, 3.0);
    return bounded == 3.0
        && math::abs(-2.5) == 2.5
        && math::lerp(2.0, 10.0, 0.25) == 4.0
        && math::pow(16, 0.5) == 4.0
        && math::round(2.5) == 3
        && math::round(-2.5) == -3;
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
    assert_eq!(result, Value::Bool(true));
}

#[test]
fn runs_compiled_option_result_standard_natives_with_try() {
    let source = r#"
fn checked(value) {
    if value > 0 {
        return option::some(value);
    }
    return option::none();
}

fn main() {
    let value = checked(4)?;
    return result::ok(value + 6);
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
        Value::Enum {
            enum_name: "Result".to_owned(),
            variant: "Ok".to_owned(),
            fields: [("0".to_owned(), Value::Int(10))].into()
        }
    );
}

#[test]
fn runs_compiled_option_result_standard_helper_natives() {
    let source = r#"
fn main() {
    let some = option::some(4);
    let none = option::none();
    let ok = result::ok(9);
    let err = result::err("missing");
    let converted_ok = option::ok_or(some, "missing");
    let converted_err = option::ok_or(none, "missing");
    let flattened_some = option::flatten(option::some(option::some(6)));
    let flattened_none = option::flatten(option::some(option::none()));
    let flattened_ok = result::flatten(result::ok(result::ok(8)));
    let flattened_err = result::flatten(result::ok(result::err("nested")));

    if option::is_some(some)
        && option::is_none(none)
        && result::is_ok(ok)
        && result::is_err(err)
        && result::is_ok(converted_ok)
        && result::is_err(converted_err)
        && option::is_none(result::to_error_option(converted_ok))
        && option::unwrap_or(result::to_error_option(converted_err), "ok") == "missing"
        && option::unwrap_or(flattened_some, 0) == 6
        && option::is_none(flattened_none)
        && result::unwrap_or(flattened_ok, 0) == 8
        && option::unwrap_or(result::to_error_option(flattened_err), "ok") == "nested"
    {
        return option::unwrap_or(some, 0)
            + option::unwrap_or(none, 5)
            + result::unwrap_or(ok, 0)
            + result::unwrap_or(err, 7)
            + option::unwrap_or(result::to_option(converted_ok), 0)
            + option::unwrap_or(result::to_option(converted_err), 11);
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
    assert_eq!(result, Value::Int(40));
}

#[test]
fn runs_compiled_option_result_map_methods() {
    let source = r#"
fn main() {
    let some = option::some(4).map(|value| value + 1);
    let none = option::none().map(|value| value + 1);
    let ok = result::ok("xp").map(|value| value.len());
    let err = result::err("blocked").map(|value| value.len());

    if option::unwrap_or(some, 0) == 5
        && option::is_none(none)
        && result::unwrap_or(ok, 0) == 2
        && result::unwrap_or(err, 9) == 9
    {
        return 1;
    }
    return 0;
}
"#;

    let program = compile_program_source(SourceId::new(1), source)
        .expect("option/result map source should compile");
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let result = vm
        .run_program(&program, "main", &[])
        .expect("option/result map source should run");
    assert_eq!(result, Value::Int(1));
}

#[test]
fn runs_compiled_result_map_err_method() {
    let source = r#"
fn main() {
    let ok = result::ok(4).map_err(|error| error.to_upper());
    let err = result::err("blocked").map_err(|error| error.to_upper());

    if result::unwrap_or(ok, 0) == 4 && result::is_err(err) {
        return match err {
            Result::Err(reason) => reason == "BLOCKED",
            _ => false,
        };
    }
    return false;
}
"#;

    let program = compile_program_source(SourceId::new(1), source)
        .expect("result map_err source should compile");
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let result = vm
        .run_program(&program, "main", &[])
        .expect("result map_err source should run");
    assert_eq!(result, Value::Bool(true));
}

#[test]
fn runs_compiled_option_result_and_then_methods() {
    let source = r#"
fn checked_option(value) {
    if value > 0 {
        return option::some(value + 1);
    }
    return option::none();
}

fn checked_result(value) {
    if value > 0 {
        return result::ok(value + 1);
    }
    return result::err("bad");
}

fn main() {
    let some = option::some(4).and_then(|value| checked_option(value));
    let none = option::none().and_then(|value| checked_option(value));
    let ok = result::ok(4).and_then(|value| checked_result(value));
    let err = result::err("blocked").and_then(|value| checked_result(value));

    return option::unwrap_or(some, 0) == 5
        && option::is_none(none)
        && result::unwrap_or(ok, 0) == 5
        && result::is_err(err);
}
"#;

    let program = compile_program_source(SourceId::new(1), source)
        .expect("option/result and_then source should compile");
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let result = vm
        .run_program(&program, "main", &[])
        .expect("option/result and_then source should run");
    assert_eq!(result, Value::Bool(true));
}

#[test]
fn runs_compiled_option_result_or_else_methods() {
    let source = r#"
fn recover_option() {
    return option::some(9);
}

fn recover_result(error) {
    return result::ok(error.len());
}

fn main() {
    let some = option::some(4).or_else(| | recover_option());
    let none = option::none().or_else(| | recover_option());
    let ok = result::ok(4).or_else(|error| recover_result(error));
    let err = result::err("bad").or_else(|error| recover_result(error));

    return option::unwrap_or(some, 0) == 4
        && option::unwrap_or(none, 0) == 9
        && result::unwrap_or(ok, 0) == 4
        && result::unwrap_or(err, 0) == 3;
}
"#;

    let program = compile_program_source(SourceId::new(1), source)
        .expect("option/result or_else source should compile");
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let result = vm
        .run_program(&program, "main", &[])
        .expect("option/result or_else source should run");
    assert_eq!(result, Value::Bool(true));
}

#[test]
fn runs_compiled_option_filter_method() {
    let source = r#"
fn main() {
    let kept = option::some(4).filter(|value| value > 2);
    let dropped = option::some(1).filter(|value| value > 2);
    let none = option::none().filter(|value| value > 2);

    return option::unwrap_or(kept, 0) == 4
        && option::is_none(dropped)
        && option::is_none(none);
}
"#;

    let program = compile_program_source(SourceId::new(1), source)
        .expect("option filter source should compile");
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let result = vm
        .run_program(&program, "main", &[])
        .expect("option filter source should run");
    assert_eq!(result, Value::Bool(true));
}

#[test]
fn runs_compiled_option_result_helper_methods() {
    let source = r#"
fn main() {
    let some = option::some(4);
    let none = option::none();
    let ok = result::ok(9);
    let err = result::err("missing");
    let converted_ok = some.ok_or("missing");
    let converted_err = none.ok_or("missing");
    let flattened_some = option::some(option::some(6)).flatten();
    let flattened_none = option::some(option::none()).flatten();
    let flattened_ok = result::ok(result::ok(8)).flatten();
    let flattened_err = result::ok(result::err("nested")).flatten();

    if some.is_some()
        && none.is_none()
        && ok.is_ok()
        && err.is_err()
        && converted_ok.is_ok()
        && converted_err.is_err()
        && converted_ok.to_error_option().is_none()
        && converted_err.to_error_option().unwrap_or("ok") == "missing"
        && flattened_some.unwrap_or(0) == 6
        && flattened_none.is_none()
        && flattened_ok.unwrap_or(0) == 8
        && flattened_err.to_error_option().unwrap_or("ok") == "nested"
    {
        return some.unwrap_or(0)
            + none.unwrap_or(5)
            + ok.unwrap_or(0)
            + err.unwrap_or(7)
            + converted_ok.to_option().unwrap_or(0)
            + converted_err.to_option().unwrap_or(11);
    }
    return 0;
}
"#;

    let program = compile_program_source(SourceId::new(1), source)
        .expect("option/result helper method source should compile");
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let result = vm
        .run_program(&program, "main", &[])
        .expect("option/result helper method source should run");
    assert_eq!(result, Value::Int(40));
}
