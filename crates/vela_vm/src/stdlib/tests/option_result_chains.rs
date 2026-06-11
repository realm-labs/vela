use super::*;
use crate::owned_value::OwnedValue;

#[test]
fn runs_compiled_option_ok_or_with_try_propagation() {
    let source = r#"
fn checked(raw) {
    let value = option::ok_or(raw, "bad level")?;
    return result::ok(value + 1);
}

fn main() {
    let ok = checked(option::some(41));
    let err = checked(option::none());
    if result::unwrap_or(ok, 0) == 42
        && result::is_err(err)
        && option::is_none(result::to_option(err))
    {
        return option::unwrap_or(result::to_option(ok), 0);
    }
    return 0;
}
"#;

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
fn managed_heap_execution_runs_option_result_standard_helper_natives() {
    let source = r#"
fn main() {
    let some = option::some("quest");
    let none = option::none();
    let ok = result::ok("done");
    let err = result::err("blocked");
    let converted_ok = option::ok_or(some, "missing");
    let converted_err = option::ok_or(none, "missing");
    let flattened_some = option::flatten(option::some(option::some(["quest", "done"])));
    let flattened_none = option::flatten(option::some(option::none()));
    let flattened_ok = result::flatten(result::ok(result::ok(["done"])));
    let flattened_err = result::flatten(result::ok(result::err(["nested"])));

    return option::is_some(some)
        && option::is_none(none)
        && result::is_ok(ok)
        && result::is_err(err)
        && result::is_ok(converted_ok)
        && result::is_err(converted_err)
        && option::unwrap_or(some, "fallback") == "quest"
        && option::unwrap_or(none, "fallback") == "fallback"
        && result::unwrap_or(ok, "fallback") == "done"
        && result::unwrap_or(err, "fallback") == "fallback"
        && option::unwrap_or(result::to_option(converted_ok), "fallback") == "quest"
        && option::unwrap_or(result::to_option(converted_err), "fallback") == "fallback"
        && option::is_none(result::to_error_option(converted_ok))
        && option::unwrap_or(result::to_error_option(converted_err), "fallback") == "missing"
        && option::unwrap_or(flattened_some, ["", ""])[0] == "quest"
        && option::unwrap_or(flattened_some, ["", ""])[1] == "done"
        && option::is_none(flattened_none)
        && result::unwrap_or(flattened_ok, [""])[0] == "done"
        && option::unwrap_or(result::to_error_option(flattened_err), [""])[0] == "nested";
}
"#;

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
    let source = r#"
fn first_tag(values) {
    return option::some(values[0]);
}

fn join_values(values) {
    return result::ok(values[0]);
}

fn main() {
    let some = option::some(["quest"]).and_then(|values| first_tag(values));
    let none = option::none().and_then(|values| first_tag(values));
    let ok = result::ok(["a", "b"]).and_then(|values| join_values(values));
    let err = result::err(["blocked"]).and_then(|values| join_values(values));

    return option::unwrap_or(some, "") == "quest"
        && option::is_none(none)
        && result::unwrap_or(ok, "") == "a"
        && result::is_err(err);
}
"#;

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
    let source = r#"
fn fallback_tags() {
    return option::some(["fallback"]);
}

fn fallback_result(errors) {
    return result::ok(errors[0]);
}

fn main() {
    let some = option::some(["keep"]).or_else(| | fallback_tags());
    let none = option::none().or_else(| | fallback_tags());
    let ok = result::ok("done").or_else(|errors| fallback_result(errors));
    let err = result::err(["bad", "level"]).or_else(|errors| fallback_result(errors));

    return option::unwrap_or(some, [""])[0] == "keep"
        && option::unwrap_or(none, [""])[0] == "fallback"
        && result::unwrap_or(ok, "") == "done"
        && result::unwrap_or(err, "") == "bad";
}
"#;

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
