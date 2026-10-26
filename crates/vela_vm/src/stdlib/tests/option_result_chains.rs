use super::*;
use crate::owned_value::OwnedValue;

#[test]
fn runs_compiled_option_ok_or_with_try_propagation() {
    let source = include_str!("../../../tests/fixtures/stdlib/option_ok_or_try.vela");

    let program = compile_standard_program_source(SourceId::new(1), source)
        .expect("option ok_or stdlib source should compile");
    let mut vm = Vm::new();
    vm.register_standard_natives();
    let mut budget = ExecutionBudget::unbounded();

    let result =
        run_linked_stdlib_test_program_with_budget(&vm, &program, "main", &[], &mut budget)
            .expect("option ok_or stdlib source should run");
    assert_eq!(
        result,
        OwnedValue::Scalar(vela_common::ScalarValue::I64(42))
    );
}

#[test]
fn managed_heap_execution_runs_result_standard_natives_with_try() {
    let source = r#"
fn checked(value) {
    if value > 0 {
        return result::ok("good");
    }
    return result::err("bad");
}

fn main() {
    let value = checked(0)?;
    return result::ok(value);
}
"#;

    let program = compile_standard_program_source(SourceId::new(1), source)
        .expect("heap result stdlib source should compile");
    let mut vm = Vm::new();
    vm.register_standard_natives();
    let mut budget = ExecutionBudget::unbounded();

    let result =
        run_linked_stdlib_test_program_with_budget(&vm, &program, "main", &[], &mut budget)
            .expect("heap result stdlib source should run");
    assert_eq!(
        result,
        OwnedValue::Enum {
            enum_name: "Result".to_owned(),
            variant: "Err".to_owned(),
            fields: [("0".to_owned(), OwnedValue::String("bad".to_owned()))].into()
        }
    );
}

#[test]
fn managed_heap_execution_runs_result_tuple_payloads_with_try_destructuring() {
    let source = include_str!("../../../tests/fixtures/stdlib/result_tuple_try.vela");

    let program = compile_standard_program_source(SourceId::new(1), source)
        .expect("result tuple payload stdlib source should compile");
    let mut vm = Vm::new();
    vm.register_standard_natives();
    let mut budget = ExecutionBudget::unbounded();

    let result =
        run_linked_stdlib_test_program_with_budget(&vm, &program, "main", &[], &mut budget)
            .expect("result tuple payload stdlib source should run");
    assert_eq!(result, OwnedValue::Bool(true));
}

#[test]
fn managed_heap_execution_runs_option_result_standard_helper_natives() {
    let source =
        include_str!("../../../tests/fixtures/stdlib/managed_option_result_native_helpers.vela");

    let program = compile_standard_program_source(SourceId::new(1), source)
        .expect("heap option/result helper stdlib source should compile");
    let mut vm = Vm::new();
    vm.register_standard_natives();
    let mut budget = ExecutionBudget::unbounded();

    let result =
        run_linked_stdlib_test_program_with_budget(&vm, &program, "main", &[], &mut budget)
            .expect("heap option/result helper stdlib source should run");
    assert_eq!(result, OwnedValue::Bool(true));
}

#[test]
fn managed_heap_execution_runs_option_result_map_methods() {
    let source = r#"
fn main() {
    let some = option::some("quest").map(|value| "mapped");
    let none = option::none().map(|value| "mapped");
    let ok = result::ok(["a", "b"]).map(|values| values[0]);
    let err = result::err("blocked").map(|value| "mapped");

    return option::unwrap_or(some, "") == "mapped"
        && option::is_none(none)
        && result::unwrap_or(ok, "") == "a"
        && result::unwrap_or(err, "fallback") == "fallback";
}
"#;

    let program = compile_standard_program_source(SourceId::new(1), source)
        .expect("heap option/result map source should compile");
    let mut vm = Vm::new();
    vm.register_standard_natives();
    let mut budget = ExecutionBudget::unbounded();

    let result =
        run_linked_stdlib_test_program_with_budget(&vm, &program, "main", &[], &mut budget)
            .expect("heap option/result map source should run");
    assert_eq!(result, OwnedValue::Bool(true));
}

#[test]
fn managed_heap_execution_runs_result_map_err_method() {
    let source = r#"
fn main() {
    let ok = result::ok(["a", "b"]).map_err(|errors| errors[0]);
    let err = result::err(["bad", "level"]).map_err(|errors| errors[0]);

    return result::unwrap_or(ok, ["", ""])[0] == "a"
        && option::unwrap_or(result::to_error_option(err), "") == "bad";
}
"#;

    let program = compile_standard_program_source(SourceId::new(1), source)
        .expect("heap result map_err source should compile");
    let mut vm = Vm::new();
    vm.register_standard_natives();
    let mut budget = ExecutionBudget::unbounded();

    let result =
        run_linked_stdlib_test_program_with_budget(&vm, &program, "main", &[], &mut budget)
            .expect("heap result map_err source should run");
    assert_eq!(result, OwnedValue::Bool(true));
}

#[test]
fn managed_heap_execution_runs_option_result_and_then_methods() {
    let source = include_str!("../../../tests/fixtures/stdlib/managed_option_result_and_then.vela");

    let program = compile_standard_program_source(SourceId::new(1), source)
        .expect("heap option/result and_then source should compile");
    let mut vm = Vm::new();
    vm.register_standard_natives();
    let mut budget = ExecutionBudget::unbounded();

    let result =
        run_linked_stdlib_test_program_with_budget(&vm, &program, "main", &[], &mut budget)
            .expect("heap option/result and_then source should run");
    assert_eq!(result, OwnedValue::Bool(true));
}

#[test]
fn managed_heap_execution_runs_option_result_or_else_methods() {
    let source = include_str!("../../../tests/fixtures/stdlib/managed_option_result_or_else.vela");

    let program = compile_standard_program_source(SourceId::new(1), source)
        .expect("heap option/result or_else source should compile");
    let mut vm = Vm::new();
    vm.register_standard_natives();
    let mut budget = ExecutionBudget::unbounded();

    let result =
        run_linked_stdlib_test_program_with_budget(&vm, &program, "main", &[], &mut budget)
            .expect("heap option/result or_else source should run");
    assert_eq!(result, OwnedValue::Bool(true));
}
