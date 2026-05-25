use crate::Vm;

pub(crate) fn register(vm: &mut Vm) {
    crate::option_result::register(vm);
    crate::math_stdlib::register(vm);
    vm.register_native("set.from_array", crate::set_methods::from_array);
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
    let midpoint = math.floor(math.lerp(10, 20, 0.5));
    let powered = math.pow(2, 3);
    return math.max(clamped, math.abs(-4))
        + math.min(rounded, 10)
        + math.round(2.5)
        + midpoint
        + powered;
}
"#;

        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("math stdlib source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm.run(&code).expect("math stdlib source should run");
        assert_eq!(result, crate::Value::Int(42));
    }

    #[test]
    fn managed_heap_execution_runs_math_standard_natives() {
        let source = r#"
fn main() {
    let value = math.max(1.5, math.min(4.5, 3.25));
    let bounded = math.clamp(value, 2.0, 3.0);
    return bounded == 3.0
        && math.abs(-2.5) == 2.5
        && math.lerp(2.0, 10.0, 0.25) == 4.0
        && math.pow(16, 0.5) == 4.0
        && math.round(2.5) == 3
        && math.round(-2.5) == -3;
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
    let converted_ok = option.ok_or(some, "missing");
    let converted_err = option.ok_or(none, "missing");

    if option.is_some(some)
        && option.is_none(none)
        && result.is_ok(ok)
        && result.is_err(err)
        && result.is_ok(converted_ok)
        && result.is_err(converted_err)
        && option.is_none(result.to_error_option(converted_ok))
        && option.unwrap_or(result.to_error_option(converted_err), "ok") == "missing"
    {
        return option.unwrap_or(some, 0)
            + option.unwrap_or(none, 5)
            + result.unwrap_or(ok, 0)
            + result.unwrap_or(err, 7)
            + option.unwrap_or(result.to_option(converted_ok), 0)
            + option.unwrap_or(result.to_option(converted_err), 11);
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
        assert_eq!(result, crate::Value::Int(40));
    }

    #[test]
    fn runs_compiled_option_result_map_methods() {
        let source = r#"
fn main() {
    let some = option.some(4).map(|value| value + 1);
    let none = option.none().map(|value| value + 1);
    let ok = result.ok("xp").map(|value| value.len());
    let err = result.err("blocked").map(|value| value.len());

    if option.unwrap_or(some, 0) == 5
        && option.is_none(none)
        && result.unwrap_or(ok, 0) == 2
        && result.unwrap_or(err, 9) == 9
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
        assert_eq!(result, crate::Value::Int(1));
    }

    #[test]
    fn runs_compiled_result_map_err_method() {
        let source = r#"
fn main() {
    let ok = result.ok(4).map_err(|error| error.to_upper());
    let err = result.err("blocked").map_err(|error| error.to_upper());

    if result.unwrap_or(ok, 0) == 4 && result.is_err(err) {
        return match err {
            Result.Err(reason) => reason == "BLOCKED",
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
        assert_eq!(result, crate::Value::Bool(true));
    }

    #[test]
    fn runs_compiled_option_result_and_then_methods() {
        let source = r#"
fn checked_option(value) {
    if value > 0 {
        return option.some(value + 1);
    }
    return option.none();
}

fn checked_result(value) {
    if value > 0 {
        return result.ok(value + 1);
    }
    return result.err("bad");
}

fn main() {
    let some = option.some(4).and_then(|value| checked_option(value));
    let none = option.none().and_then(|value| checked_option(value));
    let ok = result.ok(4).and_then(|value| checked_result(value));
    let err = result.err("blocked").and_then(|value| checked_result(value));

    return option.unwrap_or(some, 0) == 5
        && option.is_none(none)
        && result.unwrap_or(ok, 0) == 5
        && result.is_err(err);
}
"#;

        let program = compile_program_source(SourceId::new(1), source)
            .expect("option/result and_then source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm
            .run_program(&program, "main", &[])
            .expect("option/result and_then source should run");
        assert_eq!(result, crate::Value::Bool(true));
    }

    #[test]
    fn runs_compiled_option_result_or_else_methods() {
        let source = r#"
fn recover_option() {
    return option.some(9);
}

fn recover_result(error) {
    return result.ok(error.len());
}

fn main() {
    let some = option.some(4).or_else(| | recover_option());
    let none = option.none().or_else(| | recover_option());
    let ok = result.ok(4).or_else(|error| recover_result(error));
    let err = result.err("bad").or_else(|error| recover_result(error));

    return option.unwrap_or(some, 0) == 4
        && option.unwrap_or(none, 0) == 9
        && result.unwrap_or(ok, 0) == 4
        && result.unwrap_or(err, 0) == 3;
}
"#;

        let program = compile_program_source(SourceId::new(1), source)
            .expect("option/result or_else source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm
            .run_program(&program, "main", &[])
            .expect("option/result or_else source should run");
        assert_eq!(result, crate::Value::Bool(true));
    }

    #[test]
    fn runs_compiled_option_filter_method() {
        let source = r#"
fn main() {
    let kept = option.some(4).filter(|value| value > 2);
    let dropped = option.some(1).filter(|value| value > 2);
    let none = option.none().filter(|value| value > 2);

    return option.unwrap_or(kept, 0) == 4
        && option.is_none(dropped)
        && option.is_none(none);
}
"#;

        let program = compile_program_source(SourceId::new(1), source)
            .expect("option filter source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm
            .run_program(&program, "main", &[])
            .expect("option filter source should run");
        assert_eq!(result, crate::Value::Bool(true));
    }

    #[test]
    fn runs_compiled_option_result_helper_methods() {
        let source = r#"
fn main() {
    let some = option.some(4);
    let none = option.none();
    let ok = result.ok(9);
    let err = result.err("missing");
    let converted_ok = some.ok_or("missing");
    let converted_err = none.ok_or("missing");

    if some.is_some()
        && none.is_none()
        && ok.is_ok()
        && err.is_err()
        && converted_ok.is_ok()
        && converted_err.is_err()
        && converted_ok.to_error_option().is_none()
        && converted_err.to_error_option().unwrap_or("ok") == "missing"
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
        assert_eq!(result, crate::Value::Int(40));
    }

    #[test]
    fn runs_compiled_option_ok_or_with_try_propagation() {
        let source = r#"
fn checked(raw) {
    let value = option.ok_or(raw.parse_int(), "bad level")?;
    return result.ok(value + 1);
}

fn main() {
    let ok = checked("41");
    let err = checked("forty-one");
    if result.unwrap_or(ok, 0) == 42
        && result.is_err(err)
        && option.is_none(result.to_option(err))
    {
        return option.unwrap_or(result.to_option(ok), 0);
    }
    return 0;
}
"#;

        let program = compile_program_source(SourceId::new(1), source)
            .expect("option ok_or stdlib source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm
            .run_program(&program, "main", &[])
            .expect("option ok_or stdlib source should run");
        assert_eq!(result, crate::Value::Int(42));
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
    let converted_ok = option.ok_or(some, "missing");
    let converted_err = option.ok_or(none, "missing");

    return option.is_some(some)
        && option.is_none(none)
        && result.is_ok(ok)
        && result.is_err(err)
        && result.is_ok(converted_ok)
        && result.is_err(converted_err)
        && option.unwrap_or(some, "fallback") == "quest"
        && option.unwrap_or(none, "fallback") == "fallback"
        && result.unwrap_or(ok, "fallback") == "done"
        && result.unwrap_or(err, "fallback") == "fallback"
        && option.unwrap_or(result.to_option(converted_ok), "fallback") == "quest"
        && option.unwrap_or(result.to_option(converted_err), "fallback") == "fallback"
        && option.is_none(result.to_error_option(converted_ok))
        && option.unwrap_or(result.to_error_option(converted_err), "fallback") == "missing";
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
    fn managed_heap_execution_runs_option_result_map_methods() {
        let source = r#"
fn main() {
    let some = option.some("quest").map(|value| value.to_upper());
    let none = option.none().map(|value| value.to_upper());
    let ok = result.ok(["a", "b"]).map(|values| values.join("."));
    let err = result.err("blocked").map(|value| value.to_upper());

    return option.unwrap_or(some, "") == "QUEST"
        && option.is_none(none)
        && result.unwrap_or(ok, "") == "a.b"
        && result.unwrap_or(err, "fallback") == "fallback";
}
"#;

        let program = compile_program_source(SourceId::new(1), source)
            .expect("heap option/result map source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();
        let mut budget = ExecutionBudget::unbounded();

        let result = vm
            .run_program_with_managed_heap_and_budget(&program, "main", &[], &mut budget)
            .expect("heap option/result map source should run");
        assert_eq!(result, crate::Value::Bool(true));
    }

    #[test]
    fn managed_heap_execution_runs_result_map_err_method() {
        let source = r#"
fn main() {
    let ok = result.ok(["a", "b"]).map_err(|errors| errors.join("."));
    let err = result.err(["bad", "level"]).map_err(|errors| errors.join("."));

    if result.unwrap_or(ok, []).join(".") == "a.b" && result.is_err(err) {
        return match err {
            Result.Err(reason) => reason == "bad.level",
            _ => false,
        };
    }
    return false;
}
"#;

        let program = compile_program_source(SourceId::new(1), source)
            .expect("heap result map_err source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();
        let mut budget = ExecutionBudget::unbounded();

        let result = vm
            .run_program_with_managed_heap_and_budget(&program, "main", &[], &mut budget)
            .expect("heap result map_err source should run");
        assert_eq!(result, crate::Value::Bool(true));
    }

    #[test]
    fn managed_heap_execution_runs_option_result_and_then_methods() {
        let source = r#"
fn first_tag(values) {
    return values.first();
}

fn join_values(values) {
    return result.ok(values.join("."));
}

fn main() {
    let some = option.some(["quest"]).and_then(|values| first_tag(values));
    let none = option.none().and_then(|values| first_tag(values));
    let ok = result.ok(["a", "b"]).and_then(|values| join_values(values));
    let err = result.err(["blocked"]).and_then(|values| join_values(values));

    return option.unwrap_or(some, "") == "quest"
        && option.is_none(none)
        && result.unwrap_or(ok, "") == "a.b"
        && result.is_err(err);
}
"#;

        let program = compile_program_source(SourceId::new(1), source)
            .expect("heap option/result and_then source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();
        let mut budget = ExecutionBudget::unbounded();

        let result = vm
            .run_program_with_managed_heap_and_budget(&program, "main", &[], &mut budget)
            .expect("heap option/result and_then source should run");
        assert_eq!(result, crate::Value::Bool(true));
    }

    #[test]
    fn managed_heap_execution_runs_option_result_or_else_methods() {
        let source = r#"
fn fallback_tags() {
    return option.some(["fallback"]);
}

fn fallback_result(errors) {
    return result.ok(errors.join("."));
}

fn main() {
    let some = option.some(["keep"]).or_else(| | fallback_tags());
    let none = option.none().or_else(| | fallback_tags());
    let ok = result.ok("done").or_else(|errors| fallback_result(errors));
    let err = result.err(["bad", "level"]).or_else(|errors| fallback_result(errors));

    return option.unwrap_or(some, []).join(".") == "keep"
        && option.unwrap_or(none, []).join(".") == "fallback"
        && result.unwrap_or(ok, "") == "done"
        && result.unwrap_or(err, "") == "bad.level";
}
"#;

        let program = compile_program_source(SourceId::new(1), source)
            .expect("heap option/result or_else source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();
        let mut budget = ExecutionBudget::unbounded();

        let result = vm
            .run_program_with_managed_heap_and_budget(&program, "main", &[], &mut budget)
            .expect("heap option/result or_else source should run");
        assert_eq!(result, crate::Value::Bool(true));
    }

    #[test]
    fn managed_heap_execution_runs_option_filter_method() {
        let source = r#"
fn main() {
    let kept = option.some("quest").filter(|value| value.starts_with("q"));
    let dropped = option.some("quest").filter(|value| value.starts_with("x"));
    let aggregate = option.some(["quest", "done"]).filter(|values| values.len() == 2);
    let none = option.none().filter(|value| value.starts_with("q"));

    return option.unwrap_or(kept, "") == "quest"
        && option.is_none(dropped)
        && option.unwrap_or(aggregate, []).join(".") == "quest.done"
        && option.is_none(none);
}
"#;

        let program = compile_program_source(SourceId::new(1), source)
            .expect("heap option filter source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();
        let mut budget = ExecutionBudget::unbounded();

        let result = vm
            .run_program_with_managed_heap_and_budget(&program, "main", &[], &mut budget)
            .expect("heap option filter source should run");
        assert_eq!(result, crate::Value::Bool(true));
    }

    #[test]
    fn managed_heap_execution_runs_option_result_helper_methods() {
        let source = r#"
fn main() {
    let some = option.some(["quest", "done"]);
    let none = option.none();
    let ok = result.ok(["done"]);
    let err = result.err(["blocked"]);
    let converted_ok = some.ok_or(["missing"]);
    let converted_err = none.ok_or(["missing"]);

    return some.is_some()
        && none.is_none()
        && ok.is_ok()
        && err.is_err()
        && converted_ok.is_ok()
        && converted_err.is_err()
        && some.unwrap_or([]).join(".") == "quest.done"
        && none.unwrap_or(["fallback"]).join(".") == "fallback"
        && ok.unwrap_or([]).join(".") == "done"
        && err.unwrap_or(["fallback"]).join(".") == "fallback"
        && converted_ok.to_option().unwrap_or([]).join(".") == "quest.done"
        && converted_err.to_option().unwrap_or(["fallback"]).join(".") == "fallback"
        && converted_ok.to_error_option().is_none()
        && converted_err.to_error_option().unwrap_or(["fallback"]).join(".") == "missing";
}
"#;

        let program = compile_program_source(SourceId::new(1), source)
            .expect("heap option/result helper method source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();
        let mut budget = ExecutionBudget::unbounded();

        let result = vm
            .run_program_with_managed_heap_and_budget(&program, "main", &[], &mut budget)
            .expect("heap option/result helper method source should run");
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
    fn option_result_and_then_rejects_non_enum_callback_results() {
        let source = r#"
fn main() {
    return option.some(1).and_then(|value| value + 1);
}
"#;

        let program = compile_program_source(SourceId::new(1), source)
            .expect("invalid and_then callback source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let error = vm
            .run_program(&program, "main", &[])
            .expect_err("invalid and_then callback should fail");
        assert_eq!(
            error.kind,
            VmErrorKind::TypeMismatch {
                operation: "method and_then"
            }
        );
    }

    #[test]
    fn option_result_or_else_rejects_non_enum_callback_results() {
        let source = r#"
fn main() {
    return option.none().or_else(| | 1);
}
"#;

        let program = compile_program_source(SourceId::new(1), source)
            .expect("invalid or_else callback source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let error = vm
            .run_program(&program, "main", &[])
            .expect_err("invalid or_else callback should fail");
        assert_eq!(
            error.kind,
            VmErrorKind::TypeMismatch {
                operation: "method or_else"
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
