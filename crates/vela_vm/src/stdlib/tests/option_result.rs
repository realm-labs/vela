use super::*;
use crate::owned_value::OwnedValue;

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

    let program = compile_standard_program_source(SourceId::new(1), source)
        .expect("math stdlib source should compile");
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let mut budget = ExecutionBudget::unbounded();

    let result =
        run_linked_stdlib_test_program_with_budget(&vm, &program, "main", &[], &mut budget)
            .expect("math stdlib source should run");
    assert_eq!(
        result,
        OwnedValue::Scalar(vela_common::ScalarValue::I64(42))
    );
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

    let program = compile_standard_program_source(SourceId::new(1), source)
        .expect("heap math stdlib source should compile");
    let mut vm = Vm::new();
    vm.register_standard_natives();
    let mut budget = ExecutionBudget::unbounded();

    let result =
        run_linked_stdlib_test_program_with_budget(&vm, &program, "main", &[], &mut budget)
            .expect("heap math stdlib source should run");
    assert_eq!(result, OwnedValue::Bool(true));
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

    let program = compile_standard_program_source(SourceId::new(1), source)
        .expect("option/result stdlib source should compile");
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let mut budget = ExecutionBudget::unbounded();

    let result =
        run_linked_stdlib_test_program_with_budget(&vm, &program, "main", &[], &mut budget)
            .expect("option/result stdlib source should run");
    assert_eq!(
        result,
        OwnedValue::Enum {
            enum_name: "Result".to_owned(),
            variant: "Ok".to_owned(),
            fields: [(
                "0".to_owned(),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(10))
            )]
            .into()
        }
    );
}

#[test]
fn runs_compiled_option_result_standard_helper_natives() {
    let source = include_str!("../../../tests/fixtures/stdlib/option_result_native_helpers.vela");

    let program = compile_standard_program_source(SourceId::new(1), source)
        .expect("option/result helper stdlib source should compile");
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let mut budget = ExecutionBudget::unbounded();

    let result =
        run_linked_stdlib_test_program_with_budget(&vm, &program, "main", &[], &mut budget)
            .expect("option/result helper stdlib source should run");
    assert_eq!(
        result,
        OwnedValue::Scalar(vela_common::ScalarValue::I64(40))
    );
}

#[test]
fn runs_compiled_option_result_map_methods() {
    let source = r#"
fn main() {
    let some = option::some(4).map(|value| value + 1);
    let none = option::none().map(|value| value + 1);
    let ok = result::ok(2).map(|value| value + 0);
    let err = result::err(9).map(|value| value + 1);

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

    let program = compile_standard_program_source(SourceId::new(1), source)
        .expect("option/result map source should compile");
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let mut budget = ExecutionBudget::unbounded();

    let result =
        run_linked_stdlib_test_program_with_budget(&vm, &program, "main", &[], &mut budget)
            .expect("option/result map source should run");
    assert_eq!(result, OwnedValue::Scalar(vela_common::ScalarValue::I64(1)));
}

#[test]
fn runs_compiled_result_map_err_method() {
    let source = r#"
fn main() {
    let ok = result::ok(4).map_err(|error| error + 1);
    let err = result::err(5).map_err(|error| error + 1);

    if result::unwrap_or(ok, 0) == 4 && result::is_err(err) {
        return option::unwrap_or(result::to_error_option(err), 0) == 6;
    }
    return false;
}
"#;

    let program = compile_standard_program_source(SourceId::new(1), source)
        .expect("result map_err source should compile");
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let mut budget = ExecutionBudget::unbounded();

    let result =
        run_linked_stdlib_test_program_with_budget(&vm, &program, "main", &[], &mut budget)
            .expect("result map_err source should run");
    assert_eq!(result, OwnedValue::Bool(true));
}

#[test]
fn runs_compiled_option_result_and_then_methods() {
    let source = include_str!("../../../tests/fixtures/stdlib/option_result_and_then.vela");

    let program = compile_standard_program_source(SourceId::new(1), source)
        .expect("option/result and_then source should compile");
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let mut budget = ExecutionBudget::unbounded();

    let result =
        run_linked_stdlib_test_program_with_budget(&vm, &program, "main", &[], &mut budget)
            .expect("option/result and_then source should run");
    assert_eq!(result, OwnedValue::Bool(true));
}

#[test]
fn runs_compiled_option_result_or_else_methods() {
    let source = include_str!("../../../tests/fixtures/stdlib/option_result_or_else.vela");

    let program = compile_standard_program_source(SourceId::new(1), source)
        .expect("option/result or_else source should compile");
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let mut budget = ExecutionBudget::unbounded();

    let result =
        run_linked_stdlib_test_program_with_budget(&vm, &program, "main", &[], &mut budget)
            .expect("option/result or_else source should run");
    assert_eq!(result, OwnedValue::Bool(true));
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

    let program = compile_standard_program_source(SourceId::new(1), source)
        .expect("option filter source should compile");
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let mut budget = ExecutionBudget::unbounded();

    let result =
        run_linked_stdlib_test_program_with_budget(&vm, &program, "main", &[], &mut budget)
            .expect("option filter source should run");
    assert_eq!(result, OwnedValue::Bool(true));
}

#[test]
fn runs_compiled_option_result_helper_methods() {
    let source = include_str!("../../../tests/fixtures/stdlib/option_result_helpers.vela");

    let program = compile_standard_program_source(SourceId::new(1), source)
        .expect("option/result helper method source should compile");
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let mut budget = ExecutionBudget::unbounded();

    let result =
        run_linked_stdlib_test_program_with_budget(&vm, &program, "main", &[], &mut budget)
            .expect("option/result helper method source should run");
    assert_eq!(
        result,
        OwnedValue::Scalar(vela_common::ScalarValue::I64(40))
    );
}
