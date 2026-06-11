use super::*;

#[test]
fn runs_compiled_array_higher_order_methods() {
    let source = r#"
fn main() {
    let values = [1, 2, 3, 4];
    let doubled = values.map(|value| value * 2);
    let evens = values.filter(|value| value % 2 == 0);
    let first_large = values.find(|value| value > 2);
    let missing = values.find(|value| value > 10);
    let count = values.count(|value| value > 1);
    if doubled[2] == 6 && evens[0] == 2 && evens[1] == 4
        && option::unwrap_or(first_large, 0) == 3
        && option::unwrap_or(missing, 9) == 9
        && values.any(|value| value == 4)
        && values.all(|value| value > 0)
    {
        return count;
    }
    return 0;
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("array higher-order methods should compile");
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let result =
        run_linked_array_test_code(&vm, code).expect("array higher-order methods should run");
    assert_eq!(result, OwnedValue::Scalar(vela_common::ScalarValue::I64(3)));
}

#[test]
fn managed_heap_execution_runs_array_higher_order_methods() {
    let source = r#"
fn main() {
    let names = ["boar", "wolf", "wyrm"];
    let lengths = names.map(|name| name.len());
    let matches = names.filter(|name| name.starts_with("w"));
    let found = names.find(|name| name.contains("yr"));
    let missing = names.find(|name| name == "dragon");
    if lengths[0] == 4 && lengths[2] == 4
        && matches.len() == 2 && matches[1] == "wyrm"
        && option::unwrap_or(found, "missing") == "wyrm"
        && option::unwrap_or(missing, "missing") == "missing"
        && names.any(|name| name.ends_with("f"))
        && names.all(|name| name.len() == 4)
    {
        return names.count(|name| name.contains("o"));
    }
    return 0;
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("heap array higher-order methods should compile");
    let mut budget = ExecutionBudget::unbounded();
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let result = run_linked_array_test_code_with_budget(&vm, code, &mut budget)
        .expect("heap array higher-order methods should run");
    assert_eq!(result, OwnedValue::Scalar(vela_common::ScalarValue::I64(2)));
}

#[test]
fn runs_compiled_array_endpoint_methods() {
    let source = r#"
fn main() {
    let values = [10, 20, 30];
    let empty = [];
    if option::unwrap_or(values.first(), 0) == 10
        && option::unwrap_or(values.last(), 0) == 30
        && option::unwrap_or(empty.first(), 7) == 7
        && option::unwrap_or(empty.last(), 9) == 9
    {
        return 1;
    }
    return 0;
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("array endpoint methods should compile");
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let result = run_linked_array_test_code(&vm, code).expect("array endpoint methods should run");
    assert_eq!(result, OwnedValue::Scalar(vela_common::ScalarValue::I64(1)));
}

#[test]
fn managed_heap_execution_runs_array_endpoint_methods() {
    let source = r#"
fn main() {
    let names = ["boar", "wolf", "wyrm"];
    let empty = [];
    if option::unwrap_or(names.first(), "missing") == "boar"
        && option::unwrap_or(names.last(), "missing") == "wyrm"
        && option::unwrap_or(empty.first(), "empty") == "empty"
        && option::unwrap_or(empty.last(), "empty") == "empty"
    {
        return option::unwrap_or(names.last(), "missing");
    }
    return "";
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("heap array endpoint methods should compile");
    let mut budget = ExecutionBudget::unbounded();
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let result = run_linked_array_test_code_with_budget(&vm, code, &mut budget)
        .expect("heap array endpoint methods should run");
    assert_eq!(result, OwnedValue::String("wyrm".to_owned()));
}

#[test]
fn runs_compiled_array_remove_at_method() {
    let source = r#"
fn main() {
    let values = [10, 20, 30];
    let removed = values.remove_at(1);
    let missing = values.remove_at(5);
    if option::unwrap_or(removed, 0) == 20
        && option::unwrap_or(missing, 99) == 99
        && values.len() == 2
        && values[0] == 10
        && values[1] == 30
    {
        return values[1];
    }
    return 0;
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("array remove_at method should compile");
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let result = run_linked_array_test_code(&vm, code).expect("array remove_at method should run");
    assert_eq!(
        result,
        OwnedValue::Scalar(vela_common::ScalarValue::I64(30))
    );
}

#[test]
fn managed_heap_execution_runs_array_remove_at_method() {
    let source = r#"
fn main() {
    let tags = ["daily", "quest", "raid"];
    let removed = tags.remove_at(0);
    let missing = tags.remove_at(9);
    if option::unwrap_or(removed, "missing") == "daily"
        && option::unwrap_or(missing, "none") == "none"
        && tags.join("|") == "quest|raid"
    {
        return tags[0];
    }
    return "";
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("heap array remove_at method should compile");
    let mut budget = ExecutionBudget::unbounded();
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let result = run_linked_array_test_code_with_budget(&vm, code, &mut budget)
        .expect("heap array remove_at method should run");
    assert_eq!(result, OwnedValue::String("quest".to_owned()));
}

#[test]
fn runs_compiled_array_insert_method() {
    let source = r#"
fn main() {
    let values = [10, 30];
    values.insert(1, 20);
    values.insert(3, 40);
    if values.len() == 4
        && values[0] == 10
        && values[1] == 20
        && values[2] == 30
        && values[3] == 40
    {
        return values[1];
    }
    return 0;
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("array insert method should compile");
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let result = run_linked_array_test_code(&vm, code).expect("array insert method should run");
    assert_eq!(
        result,
        OwnedValue::Scalar(vela_common::ScalarValue::I64(20))
    );
}

#[test]
fn managed_heap_execution_runs_array_insert_method() {
    let source = r#"
fn main() {
    let tags = ["daily", "raid"];
    tags.insert(1, "quest");
    tags.insert(tags.len(), "boss");
    if tags.join("|") == "daily|quest|raid|boss" {
        return tags[1];
    }
    return "";
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("heap array insert method should compile");
    let mut budget = ExecutionBudget::unbounded();
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let result = run_linked_array_test_code_with_budget(&vm, code, &mut budget)
        .expect("heap array insert method should run");
    assert_eq!(result, OwnedValue::String("quest".to_owned()));
}

#[test]
fn array_insert_rejects_out_of_bounds_indexes() {
    let source = r#"
fn main() {
    let values = [10];
    values.insert(2, 20);
    return values.len();
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("array insert error source should compile");

    let error = run_linked_array_test_code(&Vm::new(), code)
        .expect_err("array insert should reject sparse indexes");
    assert!(matches!(error.kind(), VmErrorKind::IndexOutOfBounds { .. }));
}

#[test]
fn runs_compiled_array_extend_method() {
    let source = r#"
fn main() {
    let values = [10, 20];
    values.extend([30, 40]);
    let empty = [];
    values.extend(empty);
    if values.len() == 4
        && values[0] == 10
        && values[1] == 20
        && values[2] == 30
        && values[3] == 40
    {
        return values[3];
    }
    return 0;
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("array extend method should compile");
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let result = run_linked_array_test_code(&vm, code).expect("array extend method should run");
    assert_eq!(
        result,
        OwnedValue::Scalar(vela_common::ScalarValue::I64(40))
    );
}

#[test]
fn managed_heap_execution_runs_array_extend_method() {
    let source = r#"
fn main() {
    let tags = ["daily"];
    let more = ["quest", "raid"];
    tags.extend(more);
    tags.extend(["boss"]);
    if tags.join("|") == "daily|quest|raid|boss" {
        return tags[2];
    }
    return "";
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("heap array extend method should compile");
    let mut budget = ExecutionBudget::unbounded();
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let result = run_linked_array_test_code_with_budget(&vm, code, &mut budget)
        .expect("heap array extend method should run");
    assert_eq!(result, OwnedValue::String("raid".to_owned()));
}

#[test]
fn array_extend_rejects_non_array_arguments() {
    let source = r#"
fn main() {
    let values = [10];
    values.extend(20);
    return values.len();
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("array extend error source should compile");

    let error = run_linked_array_test_code(&Vm::new(), code)
        .expect_err("array extend should reject non-array args");
    assert!(matches!(error.kind(), VmErrorKind::TypeMismatch { .. }));
}

#[test]
fn runs_compiled_array_clear_method() {
    let source = r#"
fn main() {
    let values = [10, 20, 30];
    values.clear();
    values.push(40);
    if values.len() == 1 && values[0] == 40 {
        return values[0];
    }
    return 0;
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("array clear method should compile");
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let result = run_linked_array_test_code(&vm, code).expect("array clear method should run");
    assert_eq!(
        result,
        OwnedValue::Scalar(vela_common::ScalarValue::I64(40))
    );
}

#[test]
fn managed_heap_execution_runs_array_clear_method() {
    let source = r#"
fn main() {
    let tags = ["daily", "quest"];
    tags.clear();
    tags.push("raid");
    if tags.len() == 1 {
        return tags[0];
    }
    return "";
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("heap array clear method should compile");
    let mut budget = ExecutionBudget::unbounded();
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let result = run_linked_array_test_code_with_budget(&vm, code, &mut budget)
        .expect("heap array clear method should run");
    assert_eq!(result, OwnedValue::String("raid".to_owned()));
}
