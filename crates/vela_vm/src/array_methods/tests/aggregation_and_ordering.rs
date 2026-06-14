use super::*;

#[test]
fn runs_compiled_array_sum_methods() {
    let source = r#"
fn main() {
    let values = [1, 2, 3, 4];
    let empty = [];
    let direct = values.sum();
    let weighted = values.sum(|value| value * 2);
    if direct == 10 && weighted == 20 && empty.sum() == 0 {
        return 1;
    }
    return 0;
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("array sum methods should compile");

    let result =
        run_linked_array_test_code(&Vm::new(), code).expect("array sum methods should run");
    assert_eq!(result, OwnedValue::Scalar(vela_common::ScalarValue::I64(1)));
}

#[test]
fn managed_heap_execution_runs_array_sum_methods() {
    let source = r#"
fn main() {
    let values = [1, 2, 3, 4];
    let direct = values.sum();
    let incremented = values.sum(|value| value + 1);
    if direct == 10 && incremented == 14 {
        return values.sum(|value| value * 3);
    }
    return 0;
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("heap array sum methods should compile");
    let mut budget = ExecutionBudget::unbounded();

    let result = run_linked_array_test_code_with_budget(&Vm::new(), code, &mut budget)
        .expect("heap array sum methods should run");
    assert_eq!(
        result,
        OwnedValue::Scalar(vela_common::ScalarValue::I64(30))
    );
}

#[test]
fn array_sum_rejects_non_numeric_values() {
    let source = r#"
fn main() {
    return ["boar"].sum();
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("array sum type error source should compile");

    let error = run_linked_array_test_code(&Vm::new(), code)
        .expect_err("array sum should reject non-numeric values");
    assert_eq!(
        error.kind(),
        VmErrorKind::TypeMismatch {
            operation: "method sum"
        }
    );
}

#[test]
fn runs_compiled_array_group_by_method() {
    let source = r#"
fn main() {
    let values = [1, 2, 3, 4, 5];
    let groups = values.group_by(|value| if value % 2 == 0 { "even" } else { "odd" });
    if groups.len() == 2
        && groups["odd"].len() == 3
        && groups["odd"][1] == 3
        && groups["even"].sum() == 6
    {
        return groups["odd"][2];
    }
    return 0;
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("array group_by method should compile");

    let result =
        run_linked_array_test_code(&Vm::new(), code).expect("array group_by method should run");
    assert_eq!(result, OwnedValue::Scalar(vela_common::ScalarValue::I64(5)));
}

#[test]
fn managed_heap_execution_runs_array_group_by_method() {
    let source = r#"
fn main() {
    let names = ["boar", "bat", "wolf", "wyrm"];
    let groups = names.group_by(|name| if name.starts_with("w") { "w" } else { "b" });
    if groups.len() == 2
        && groups["b"].len() == 2
        && groups["w"][0] == "wolf"
        && groups["w"][1] == "wyrm"
    {
        return groups["b"][1];
    }
    return "";
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("heap array group_by method should compile");
    let mut budget = ExecutionBudget::unbounded();

    let result = run_linked_array_test_code_with_budget(&Vm::new(), code, &mut budget)
        .expect("heap array group_by method should run");
    assert_eq!(result, OwnedValue::String("bat".to_owned()));
}

#[test]
fn array_group_by_accepts_value_keyed_numeric_keys() {
    let source = r#"
fn main() {
    let groups = [1, 2, 3, 4].group_by(|value| value % 2);
    if groups.len() == 2
        && groups[0].sum() == 6
        && groups[1].sum() == 4
    {
        return groups[0][1];
    }
    return 0;
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("array group_by numeric key source should compile");

    let result = run_linked_array_test_code(&Vm::new(), code)
        .expect("array group_by should accept numeric value keys");
    assert_eq!(result, OwnedValue::Scalar(vela_common::ScalarValue::I64(4)));
}

#[test]
fn array_group_by_accepts_value_keyed_identity_keys() {
    let source = r#"
struct Bucket {
    id: i64
}

fn main() {
    let even = Bucket { id: 0 };
    let odd = Bucket { id: 1 };
    let groups = [1, 2, 3, 4].group_by(|value| if value % 2 == 0 { even } else { odd });
    if groups.len() == 2
        && groups[even].sum() == 6
        && groups[odd].sum() == 4
    {
        return groups[even][1];
    }
    return 0;
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("array group_by identity key source should compile");

    let result = run_linked_array_test_code(&Vm::new(), code)
        .expect("array group_by should accept identity value keys");
    assert_eq!(result, OwnedValue::Scalar(vela_common::ScalarValue::I64(4)));
}

#[test]
fn runs_compiled_array_sort_by_method() {
    let source = r#"
fn main() {
    let values = [21, 11, 10, 12];
    let sorted = values.sort_by(|value| value % 10);
    if sorted[0] == 10
        && sorted[1] == 21
        && sorted[2] == 11
        && sorted[3] == 12
        && values[0] == 21
    {
        return sorted[2];
    }
    return 0;
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("array sort_by method should compile");

    let result =
        run_linked_array_test_code(&Vm::new(), code).expect("array sort_by method should run");
    assert_eq!(
        result,
        OwnedValue::Scalar(vela_common::ScalarValue::I64(11))
    );
}

#[test]
fn runs_compiled_array_sort_method() {
    let source = r#"
fn main() {
    let values = [4, 1, 3, 1];
    let sorted = values.sort();
    if sorted[0] == 1
        && sorted[1] == 1
        && sorted[2] == 3
        && sorted[3] == 4
        && values[0] == 4
    {
        return sorted[2];
    }
    return 0;
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("array sort method should compile");

    let result =
        run_linked_array_test_code(&Vm::new(), code).expect("array sort method should run");
    assert_eq!(result, OwnedValue::Scalar(vela_common::ScalarValue::I64(3)));
}

#[test]
fn runs_compiled_array_extrema_methods() {
    let source = r#"
fn main() {
    let values = [4, 1, 3, 1];
    let empty = [];
    if values.min().unwrap_or(0) == 1
        && values.max().unwrap_or(0) == 4
        && empty.min().unwrap_or(99) == 99
        && values[0] == 4
    {
        return values.max().unwrap_or(0);
    }
    return 0;
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("array extrema methods should compile");

    let result =
        run_linked_array_test_code(&Vm::new(), code).expect("array extrema methods should run");
    assert_eq!(result, OwnedValue::Scalar(vela_common::ScalarValue::I64(4)));
}

#[test]
fn managed_heap_execution_runs_array_sort_by_method() {
    let source = r#"
fn main() {
    let names = ["wyrm", "boar", "bat", "wolf"];
    let sorted = names.sort_by(|name| name);
    if sorted[0] == "bat"
        && sorted[1] == "boar"
        && sorted[2] == "wolf"
        && sorted[3] == "wyrm"
    {
        return sorted[1];
    }
    return "";
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("heap array sort_by method should compile");
    let mut budget = ExecutionBudget::unbounded();

    let result = run_linked_array_test_code_with_budget(&Vm::new(), code, &mut budget)
        .expect("heap array sort_by method should run");
    assert_eq!(result, OwnedValue::String("boar".to_owned()));
}

#[test]
fn managed_heap_execution_runs_array_sort_method() {
    let source = r#"
fn main() {
    let names = ["wyrm", "boar", "bat", "wolf"];
    let scores = [9, 2, 5, 2, 8, 1, 9, 3];
    let sorted = names.sort();
    let sorted_scores = scores.sort();
    if sorted[0] == "bat"
        && sorted[1] == "boar"
        && sorted[2] == "wolf"
        && sorted[3] == "wyrm"
        && sorted_scores[0] == 1
        && sorted_scores[7] == 9
    {
        return sorted_scores[7];
    }
    return 0;
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("heap array sort method should compile");
    let mut budget = ExecutionBudget::unbounded();

    let result = run_linked_array_test_code_with_budget(&Vm::new(), code, &mut budget)
        .expect("heap array sort method should run");
    assert_eq!(result, OwnedValue::Scalar(vela_common::ScalarValue::I64(9)));
}

#[test]
fn managed_heap_execution_runs_array_extrema_methods() {
    let source = r#"
fn main() {
    let names = ["wyrm", "boar", "bat", "wolf"];
    let scores = [9, 2, 5, 2, 8, 1, 9, 3];
    if names.min().unwrap_or("") == "bat"
        && names.max().unwrap_or("") == "wyrm"
        && scores.min().unwrap_or(0) == 1
        && scores.max().unwrap_or(0) == 9
    {
        return scores.max().unwrap_or(0);
    }
    return 0;
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("heap array extrema methods should compile");
    let mut budget = ExecutionBudget::unbounded();

    let result = run_linked_array_test_code_with_budget(&Vm::new(), code, &mut budget)
        .expect("heap array extrema methods should run");
    assert_eq!(result, OwnedValue::Scalar(vela_common::ScalarValue::I64(9)));
}

#[test]
fn array_sort_by_rejects_mixed_key_domains() {
    let source = r#"
fn main() {
    return [1, "two"].sort_by(|value| value);
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("array sort_by type error source should compile");

    let error = run_linked_array_test_code(&Vm::new(), code)
        .expect_err("array sort_by should reject mixed key domains");
    assert_eq!(
        error.kind(),
        VmErrorKind::TypeMismatch {
            operation: "method sort_by"
        }
    );
}

#[test]
fn array_sort_rejects_mixed_scalar_domains() {
    let source = r#"
fn main() {
    return [1, "two"].sort();
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("array sort type error source should compile");

    let error = run_linked_array_test_code(&Vm::new(), code)
        .expect_err("array sort should reject mixed scalar domains");
    assert_eq!(
        error.kind(),
        VmErrorKind::TypeMismatch {
            operation: "method sort"
        }
    );
}

#[test]
fn array_extrema_reject_mixed_scalar_domains() {
    let min_source = r#"
fn main() {
    return [1, "two"].min();
}
"#;
    let min_code = compile_function_source(SourceId::new(1), min_source, "main")
        .expect("array min type error source should compile");

    let min_error = run_linked_array_test_code(&Vm::new(), min_code)
        .expect_err("array min should reject mixed scalar domains");
    assert_eq!(
        min_error.kind(),
        VmErrorKind::TypeMismatch {
            operation: "method min"
        }
    );

    let max_source = r#"
fn main() {
    return [1, "two"].max();
}
"#;
    let max_code = compile_function_source(SourceId::new(1), max_source, "main")
        .expect("array max type error source should compile");

    let max_error = run_linked_array_test_code(&Vm::new(), max_code)
        .expect_err("array max should reject mixed scalar domains");
    assert_eq!(
        max_error.kind(),
        VmErrorKind::TypeMismatch {
            operation: "method max"
        }
    );
}
