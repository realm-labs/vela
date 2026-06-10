use super::*;
use crate::owned_value::OwnedValue;

#[test]
fn managed_heap_execution_runs_option_filter_method() {
    let source = r#"
fn main() {
    let kept = option::some("quest").filter(|value| value.starts_with("q"));
    let dropped = option::some("quest").filter(|value| value.starts_with("x"));
    let aggregate = option::some(["quest", "done"]).filter(|values| values.len() == 2);
    let none = option::some("quest").filter(|value| value.starts_with("q") && false);

    return option::unwrap_or(kept, "") == "quest"
        && option::is_none(dropped)
        && option::unwrap_or(aggregate, []).join(".") == "quest.done"
        && option::is_none(none);
}
"#;

    let program = compile_standard_program_source(SourceId::new(1), source)
        .expect("heap option filter source should compile");
    let mut vm = Vm::new();
    vm.register_standard_natives();
    let mut budget = ExecutionBudget::unbounded();

    let result =
        run_linked_stdlib_test_program_with_budget(&vm, &program, "main", &[], &mut budget)
            .expect("heap option filter source should run");
    assert_eq!(result, OwnedValue::Bool(true));
}

#[test]
fn managed_heap_execution_runs_option_result_helper_methods() {
    let source = r#"
fn main() {
    let some = option::some(["quest", "done"]);
    let none = option::none();
    let ok = result::ok(["done"]);
    let err = result::err(["blocked"]);
    let converted_ok = some.ok_or(["missing"]);
    let converted_err = none.ok_or(["missing"]);
    let flattened_some = option::some(option::some(["quest", "done"])).flatten();
    let flattened_none = option::some(option::none()).flatten();
    let flattened_ok = result::ok(result::ok(["done"])).flatten();
    let flattened_err = result::ok(result::err(["nested"])).flatten();

    return some.is_some()
        && none.is_none()
        && ok.is_ok()
        && err.is_err()
        && converted_ok.is_ok()
        && converted_err.is_err()
        && option::unwrap_or(some, ["", ""])[0] == "quest"
        && option::unwrap_or(some, ["", ""])[1] == "done"
        && option::unwrap_or(none, ["fallback"])[0] == "fallback"
        && result::unwrap_or(ok, [""])[0] == "done"
        && result::unwrap_or(err, ["fallback"])[0] == "fallback"
        && option::unwrap_or(converted_ok.to_option(), ["", ""])[0] == "quest"
        && option::unwrap_or(converted_ok.to_option(), ["", ""])[1] == "done"
        && option::unwrap_or(converted_err.to_option(), ["fallback"])[0] == "fallback"
        && converted_ok.to_error_option().is_none()
        && option::unwrap_or(converted_err.to_error_option(), ["fallback"])[0] == "missing"
        && option::unwrap_or(flattened_some, ["", ""])[0] == "quest"
        && option::unwrap_or(flattened_some, ["", ""])[1] == "done"
        && flattened_none.is_none()
        && result::unwrap_or(flattened_ok, [""])[0] == "done"
        && option::unwrap_or(flattened_err.to_error_option(), [""])[0] == "nested";
}
"#;

    let program = compile_standard_program_source(SourceId::new(1), source)
        .expect("heap option/result helper method source should compile");
    let mut vm = Vm::new();
    vm.register_standard_natives();
    let mut budget = ExecutionBudget::unbounded();

    let result =
        run_linked_stdlib_test_program_with_budget(&vm, &program, "main", &[], &mut budget)
            .expect("heap option/result helper method source should run");
    assert_eq!(result, OwnedValue::Bool(true));
}

#[test]
fn option_result_helpers_reject_wrong_dynamic_shapes() {
    let source = r#"
fn main() {
    return option::unwrap_or(result::ok(1), 0);
}
"#;

    let program = compile_standard_program_source(SourceId::new(1), source)
        .expect("option/result helper type-error source should compile");
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let error = vm
        .run_program(&program, "main", &[])
        .expect_err("option helper should reject Result values");
    assert_eq!(
        error.kind(),
        VmErrorKind::TypeMismatch {
            operation: "option::unwrap_or"
        }
    );
}

#[test]
fn option_result_flatten_rejects_non_nested_values() {
    let source = r#"
fn main() {
    return option::some(1).flatten();
}
"#;

    let program = compile_standard_program_source(SourceId::new(1), source)
        .expect("invalid option flatten source should compile");
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let error = vm
        .run_program(&program, "main", &[])
        .expect_err("invalid option flatten should fail");
    assert_eq!(
        error.kind(),
        VmErrorKind::TypeMismatch {
            operation: "method flatten"
        }
    );
}

#[test]
fn option_result_and_then_rejects_non_enum_callback_results() {
    let source = r#"
fn main() {
    return option::some(1).and_then(|value| value + 1);
}
"#;

    let program = compile_standard_program_source(SourceId::new(1), source)
        .expect("invalid and_then callback source should compile");
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let error = vm
        .run_program(&program, "main", &[])
        .expect_err("invalid and_then callback should fail");
    assert_eq!(
        error.kind(),
        VmErrorKind::TypeMismatch {
            operation: "method and_then"
        }
    );
}

#[test]
fn option_result_or_else_rejects_non_enum_callback_results() {
    let source = r#"
fn main() {
    return option::none().or_else(| | 1);
}
"#;

    let program = compile_standard_program_source(SourceId::new(1), source)
        .expect("invalid or_else callback source should compile");
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let error = vm
        .run_program(&program, "main", &[])
        .expect_err("invalid or_else callback should fail");
    assert_eq!(
        error.kind(),
        VmErrorKind::TypeMismatch {
            operation: "method or_else"
        }
    );
}

#[test]
fn runs_compiled_set_standard_natives_and_methods() {
    let source = r#"
fn main() {
    let tags = set::from_array(["fire", "ice", "fire"]);
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
    assert_eq!(result, OwnedValue::Int(2));
}

#[test]
fn managed_heap_execution_runs_set_standard_natives_and_iteration() {
    let source = r#"
fn main() {
    let ids = set::from_array([1, 2, 2, 3]);
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
    assert_eq!(result, OwnedValue::Int(8));
}

#[test]
fn set_from_array_rejects_non_scalar_elements() {
    let source = r#"
fn main() {
    return set::from_array([[1]]);
}
"#;

    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("set type error source should compile");
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let error = vm
        .run(&code)
        .expect_err("set::from_array should reject non-scalar elements");
    assert_eq!(
        error.kind(),
        VmErrorKind::TypeMismatch {
            operation: "set::from_array"
        }
    );
}
