use super::*;

#[test]
fn runs_compiled_array_contains_method() {
    let source = r#"
fn main() {
    let values = [10, 20, 30];
    let rewards = [Reward { item_id: "gold", count: 2 }];
    let expected = Reward { item_id: "gold", count: 2 };
    if values.contains(20)
        && !values.contains(99)
        && rewards.contains(expected)
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

    let result = vm.run(&code).expect("array contains method should run");
    assert_eq!(result, Value::Int(1));
}

#[test]
fn managed_heap_execution_runs_array_contains_method() {
    let source = r#"
fn main() {
    let tags = ["daily", "quest", "raid"];
    let nested = [["daily", "quest"], ["raid"]];
    if tags.contains("quest")
        && !tags.contains("bonus")
        && nested.contains(["daily", "quest"])
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

    let result = vm
        .run_with_managed_heap_and_budget(&code, &mut budget)
        .expect("heap array contains method should run");
    assert_eq!(result, Value::String("daily,quest,raid".to_owned()));
}

#[test]
fn runs_compiled_array_index_of_method() {
    let source = r#"
fn main() {
    let values = [10, 20, 30, 20];
    let rewards = [Reward { item_id: "gold", count: 2 }];
    let expected = Reward { item_id: "gold", count: 2 };
    if option::unwrap_or(values.index_of(20), -1) == 1
        && option::unwrap_or(values.index_of(99), -1) == -1
        && option::unwrap_or(rewards.index_of(expected), -1) == 0
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

    let result = vm.run(&code).expect("array index_of method should run");
    assert_eq!(result, Value::Int(1));
}

#[test]
fn managed_heap_execution_runs_array_index_of_method() {
    let source = r#"
fn main() {
    let tags = ["daily", "quest", "raid"];
    let nested = [["daily", "quest"], ["raid"]];
    if option::unwrap_or(tags.index_of("quest"), -1) == 1
        && option::unwrap_or(tags.index_of("bonus"), -1) == -1
        && option::unwrap_or(nested.index_of(["raid"]), -1) == 1
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

    let result = vm
        .run_with_managed_heap_and_budget(&code, &mut budget)
        .expect("heap array index_of method should run");
    assert_eq!(result, Value::String("raid".to_owned()));
}

#[test]
fn runs_compiled_array_distinct_method() {
    let source = r#"
fn main() {
    let rewards = [
        Reward { item_id: "gold", count: 2 },
        Reward { item_id: "xp", count: 1 },
        Reward { item_id: "gold", count: 2 },
    ];
    let unique = [3, 1, 3, 2, 1].distinct();
    let unique_rewards = rewards.distinct();
    if unique.len() == 3
        && unique[0] == 3
        && unique[1] == 1
        && unique[2] == 2
        && rewards.len() == 3
        && unique_rewards.len() == 2
    {
        return unique_rewards[0].item_id;
    }
    return "";
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("array distinct source should compile");

    let result = Vm::new().run(&code).expect("array distinct should run");
    assert_eq!(result, Value::String("gold".to_owned()));
}

#[test]
fn managed_heap_execution_runs_array_distinct_method() {
    let source = r#"
fn main() {
    let scores = [3, 1, 3, 2, 1];
    let tags = ["raid", "quest", "raid", "daily", "quest"];
    let nested = [["daily", "quest"], ["daily", "quest"], ["raid"]];
    let unique_scores = scores.distinct();
    let unique_tags = tags.distinct();
    let unique_nested = nested.distinct();
    if scores.len() == 5
        && unique_scores.len() == 3
        && unique_scores[0] == 3
        && unique_scores[1] == 1
        && unique_scores[2] == 2
        && tags.len() == 5
        && unique_tags.join(",") == "raid,quest,daily"
        && unique_nested.len() == 2
    {
        return unique_scores.sum();
    }
    return 0;
}
"#;
    let code = compile_function_source(SourceId::new(1), source, "main")
        .expect("heap array distinct source should compile");
    let mut budget = ExecutionBudget::unbounded();

    let result = Vm::new()
        .run_with_managed_heap_and_budget(&code, &mut budget)
        .expect("heap array distinct should run");
    assert_eq!(result, Value::Int(6));
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

    let result = Vm::new().run(&code).expect("array reverse should run");
    assert_eq!(result, Value::Int(2));
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

    let result = Vm::new()
        .run_with_managed_heap_and_budget(&code, &mut budget)
        .expect("heap array reverse should run");
    assert_eq!(result, Value::Int(26));
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

    let result = Vm::new().run(&code).expect("array slice should run");
    assert_eq!(result, Value::Int(2));
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

    let result = Vm::new()
        .run_with_managed_heap_and_budget(&code, &mut budget)
        .expect("heap array slice should run");
    assert_eq!(result, Value::Int(1));
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

    let error = Vm::new()
        .run(&code)
        .expect_err("array slice should reject out of bounds index");
    assert_eq!(
        error.kind,
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

    let result = Vm::new().run(&code).expect("array join method should run");
    assert_eq!(result, Value::String("quest.stage.done".to_owned()));
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

    let result = Vm::new()
        .run_with_managed_heap_and_budget(&code, &mut budget)
        .expect("heap array join method should run");
    assert_eq!(result, Value::String("boar|wolf|wyrm".to_owned()));
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

    let error = Vm::new()
        .run(&code)
        .expect_err("array join should reject non-string values");
    assert_eq!(
        error.kind,
        VmErrorKind::TypeMismatch {
            operation: "method join"
        }
    );
}
