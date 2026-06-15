use super::*;

#[test]
fn runs_compiled_array_contains_method() {
    let source = r#"
fn main() {
    let values = [10, 20, 30];
    if values.contains(20)
        && !values.contains(99)
        && ![].contains("missing")
    {
        return 1;
    }
    return 0;
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("array contains method should compile");
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let result = run_linked_array_test_code(&vm, code).expect("array contains method should run");
    assert_eq!(result, OwnedValue::Scalar(vela_common::ScalarValue::I64(1)));
}

#[test]
fn managed_heap_execution_runs_array_contains_method() {
    let source = r#"
fn main() {
    let tags = ["daily", "quest", "raid"];
    if tags.contains("quest")
        && !tags.contains("bonus")
    {
        return tags.join(",");
    }
    return "";
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("heap array contains method should compile");
    let mut budget = ExecutionBudget::unbounded();
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let result = run_linked_array_test_code_with_budget(&vm, code, &mut budget)
        .expect("heap array contains method should run");
    assert_eq!(result, OwnedValue::String("daily,quest,raid".to_owned()));
}

#[test]
fn runs_compiled_array_index_of_method() {
    let source = r#"
fn main() {
    let values = [10, 20, 30, 20];
    if option::unwrap_or(values.index_of(20), -1) == 1
        && option::unwrap_or(values.index_of(99), -1) == -1
        && option::unwrap_or([].index_of("missing"), -1) == -1
    {
        return 1;
    }
    return 0;
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("array index_of method should compile");
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let result = run_linked_array_test_code(&vm, code).expect("array index_of method should run");
    assert_eq!(result, OwnedValue::Scalar(vela_common::ScalarValue::I64(1)));
}

#[test]
fn managed_heap_execution_runs_array_index_of_method() {
    let source = r#"
fn main() {
    let tags = ["daily", "quest", "raid"];
    if option::unwrap_or(tags.index_of("quest"), -1) == 1
        && option::unwrap_or(tags.index_of("bonus"), -1) == -1
    {
        return tags[option::unwrap_or(tags.index_of("raid"), 0)];
    }
    return "";
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("heap array index_of method should compile");
    let mut budget = ExecutionBudget::unbounded();
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let result = run_linked_array_test_code_with_budget(&vm, code, &mut budget)
        .expect("heap array index_of method should run");
    assert_eq!(result, OwnedValue::String("raid".to_owned()));
}

#[test]
fn managed_heap_execution_runs_array_scalar_lookup_methods() {
    let source = r#"
fn main() {
    let values = [1, 2, 3, 5, 8, 13];
    if values.contains(8)
        && !values.contains(21)
        && option::unwrap_or(values.index_of(13), -1) == 5
        && option::unwrap_or(values.index_of(21), -1) == -1
    {
        return values.len();
    }
    return 0;
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("heap array scalar lookup source should compile");
    let mut budget = ExecutionBudget::unbounded();
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let result = run_linked_array_test_code_with_budget(&vm, code, &mut budget)
        .expect("heap array scalar lookup methods should run");
    assert_eq!(result, OwnedValue::Scalar(vela_common::ScalarValue::I64(6)));
}

#[test]
fn array_lookup_methods_use_value_key_identity_for_objects() {
    let source = r#"
fn main() {
    let rewards = [Reward { item_id: "gold", count: 2 }];
    let expected = Reward { item_id: "gold", count: 2 };
    if !rewards.contains(expected) && option::unwrap_or(rewards.index_of(expected), -1) == -1 {
        return 1;
    }
    return 0;
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("array contains source should compile");

    let mut vm = Vm::new();
    vm.register_standard_natives();
    let result =
        run_linked_array_test_code(&vm, code).expect("record lookup should use key identity");
    assert_eq!(result, OwnedValue::Scalar(vela_common::ScalarValue::I64(1)));

    let source = r#"
fn main() {
    let nested = [["daily", "quest"], ["raid"]];
    return option::unwrap_or(nested.index_of(["raid"]), -1);
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("array index_of source should compile");

    let mut vm = Vm::new();
    vm.register_standard_natives();
    assert_eq!(
        run_linked_array_test_code(&vm, code),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(-1)))
    );
}

#[test]
fn array_distinct_uses_value_key_identity_for_objects() {
    let source = r#"
fn main() {
    let nested = [["daily", "quest"], ["daily", "quest"], ["raid"]];
    return nested.distinct().len();
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("array distinct source should compile");

    assert_eq!(
        run_linked_array_test_code(&Vm::new(), code),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(3)))
    );
}

#[test]
fn array_predicate_search_uses_partial_eq_but_lookup_and_distinct_use_value_key() {
    let source = r#"
struct Reward { code: String, amount: i64 }
impl PartialEq for Reward {
    fn eq(self, other: Reward) -> bool {
        return self.code == other.code;
    }
}

fn main() {
    let rewards = [
        Reward { code: "xp", amount: 10 },
        Reward { code: "xp", amount: 99 },
        Reward { code: "gold", amount: 1 },
    ];
    let expected_xp = Reward { code: "xp", amount: 0 };
    let expected_gold = Reward { code: "gold", amount: 999 };
    let unique = rewards.distinct();
    let found = rewards.find(|reward| reward == expected_gold);
    let found_reward = option::unwrap_or(found, Reward { code: "missing", amount: -1 });
    if rewards.any(|reward| reward == expected_xp)
        && found_reward.amount == 1
        && !rewards.contains(expected_xp)
        && option::unwrap_or(rewards.index_of(expected_gold), -1) == -1
        && unique.len() == 3
        && unique[0].amount == 10
        && unique[1].amount == 99
        && unique[2].code == "gold"
    {
        return 1;
    }
    return 0;
}
"#;
    let program = compile_program_source(SourceId::new(1), source)
        .expect("array PartialEq method source should compile");

    let mut vm = Vm::new();
    vm.register_standard_natives();
    let result = run_linked_array_test_program(&vm, &program, "main")
        .expect("array key and predicate equality methods should run");
    assert_eq!(result, OwnedValue::Scalar(vela_common::ScalarValue::I64(1)));
}

#[test]
fn runs_compiled_array_distinct_method() {
    let source = r#"
fn main() {
    let unique = [3, 1, 3, 2, 1].distinct();
    if unique.len() == 3
        && unique[0] == 3
        && unique[1] == 1
        && unique[2] == 2
    {
        return "ok";
    }
    return "";
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("array distinct source should compile");

    let result = run_linked_array_test_code(&Vm::new(), code).expect("array distinct should run");
    assert_eq!(result, OwnedValue::String("ok".to_owned()));
}

#[test]
fn managed_heap_execution_runs_array_distinct_method() {
    let source = r#"
fn main() {
    let scores = [3, 1, 3, 2, 1];
    let tags = ["raid", "quest", "raid", "daily", "quest"];
    let unique_scores = scores.distinct();
    let unique_tags = tags.distinct();
    if scores.len() == 5
        && unique_scores.len() == 3
        && unique_scores[0] == 3
        && unique_scores[1] == 1
        && unique_scores[2] == 2
        && tags.len() == 5
        && unique_tags.join(",") == "raid,quest,daily"
    {
        return unique_scores.sum();
    }
    return 0;
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("heap array distinct source should compile");
    let mut budget = ExecutionBudget::unbounded();

    let result = run_linked_array_test_code_with_budget(&Vm::new(), code, &mut budget)
        .expect("heap array distinct should run");
    assert_eq!(result, OwnedValue::Scalar(vela_common::ScalarValue::I64(6)));
}

#[test]
fn runs_compiled_array_reverse_method() {
    let source = r#"
fn main() {
    let rewards = [
        Reward { item_id: "gold", count: 2 },
        Reward { item_id: "xp", count: 1 },
    ];
    let reversed = [1, 2, 3].reverse();
    let reversed_rewards = rewards.reverse();
    if reversed[0] == 3
        && reversed[2] == 1
        && rewards[0].item_id == "gold"
        && reversed_rewards[0].item_id == "xp"
    {
        return reversed_rewards[1].count;
    }
    return 0;
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("array reverse source should compile");

    let result = run_linked_array_test_code(&Vm::new(), code).expect("array reverse should run");
    assert_eq!(result, OwnedValue::Scalar(vela_common::ScalarValue::I64(2)));
}

#[test]
fn managed_heap_execution_runs_array_reverse_method() {
    let source = r#"
fn main() {
    let tags = ["daily", "quest", "raid"];
    let scores = [9, 2, 5, 2, 8];
    let nested = [["daily", "quest"], ["raid"]];
    let reversed_tags = tags.reverse();
    let reversed_scores = scores.reverse();
    let reversed_nested = nested.reverse();
    if tags.join(",") == "daily,quest,raid"
        && reversed_tags.join(",") == "raid,quest,daily"
        && reversed_scores[0] == 8
        && reversed_scores[4] == 9
        && reversed_nested[0].join("|") == "raid"
    {
        return reversed_scores.sum();
    }
    return 0;
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("heap array reverse source should compile");
    let mut budget = ExecutionBudget::unbounded();

    let result = run_linked_array_test_code_with_budget(&Vm::new(), code, &mut budget)
        .expect("heap array reverse should run");
    assert_eq!(
        result,
        OwnedValue::Scalar(vela_common::ScalarValue::I64(26))
    );
}

#[test]
fn runs_compiled_array_slice_method() {
    let source = r#"
fn main() {
    let rewards = [
        Reward { item_id: "gold", count: 2 },
        Reward { item_id: "xp", count: 1 },
        Reward { item_id: "gem", count: 3 },
    ];
    let middle = [10, 20, 30, 40].slice(1, 3);
    let reward_slice = rewards.slice(0, 2);
    let empty = rewards.slice(2, 2);
    if middle[0] == 20
        && middle[1] == 30
        && reward_slice[1].item_id == "xp"
        && rewards.len() == 3
        && empty.is_empty()
    {
        return reward_slice[0].count;
    }
    return 0;
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("array slice source should compile");

    let result = run_linked_array_test_code(&Vm::new(), code).expect("array slice should run");
    assert_eq!(result, OwnedValue::Scalar(vela_common::ScalarValue::I64(2)));
}

#[test]
fn managed_heap_execution_runs_array_slice_method() {
    let source = r#"
fn main() {
    let tags = ["daily", "quest", "raid", "bonus"];
    let scores = [9, 2, 5, 2, 8, 1, 9, 3];
    let nested = [["daily", "quest"], ["raid"], ["bonus"]];
    let tag_slice = tags.slice(1, 3);
    let score_slice = scores.slice(2, 6);
    let nested_slice = nested.slice(0, 2);
    if tags.join(",") == "daily,quest,raid,bonus"
        && tag_slice.join("|") == "quest|raid"
        && score_slice.sum() == 16
        && nested_slice[0].join("|") == "daily|quest"
    {
        return score_slice[3];
    }
    return 0;
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("heap array slice source should compile");
    let mut budget = ExecutionBudget::unbounded();

    let result = run_linked_array_test_code_with_budget(&Vm::new(), code, &mut budget)
        .expect("heap array slice should run");
    assert_eq!(result, OwnedValue::Scalar(vela_common::ScalarValue::I64(1)));
}

#[test]
fn array_slice_rejects_out_of_bounds_ranges() {
    let source = r#"
fn main() {
    return [1, 2].slice(0, 3);
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("array slice bounds source should compile");

    let error = run_linked_array_test_code(&Vm::new(), code)
        .expect_err("array slice should reject out of bounds index");
    assert_eq!(
        error.kind(),
        VmErrorKind::IndexOutOfBounds { index: 3, len: 2 }
    );
}

#[test]
fn runs_compiled_array_join_method() {
    let source = r#"
fn main() {
    let path = ["quest", "stage", "done"].join(".");
    if path == "quest.stage.done" && [].join(",") == "" {
        return path;
    }
    return "";
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("array join method should compile");

    let result =
        run_linked_array_test_code(&Vm::new(), code).expect("array join method should run");
    assert_eq!(result, OwnedValue::String("quest.stage.done".to_owned()));
}

#[test]
fn managed_heap_execution_runs_array_join_method() {
    let source = r#"
fn main() {
    let tags = ["boar", "wolf", "wyrm"];
    return tags.join("|");
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("heap array join method should compile");
    let mut budget = ExecutionBudget::unbounded();

    let result = run_linked_array_test_code_with_budget(&Vm::new(), code, &mut budget)
        .expect("heap array join method should run");
    assert_eq!(result, OwnedValue::String("boar|wolf|wyrm".to_owned()));
}

#[test]
fn array_join_rejects_non_string_values() {
    let source = r#"
fn main() {
    return ["boar", 1].join(",");
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("array join type error source should compile");

    let error = run_linked_array_test_code(&Vm::new(), code)
        .expect_err("array join should reject non-string values");
    assert_eq!(
        error.kind(),
        VmErrorKind::TypeMismatch {
            operation: "method join"
        }
    );
}
