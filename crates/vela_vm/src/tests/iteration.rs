use super::*;
use crate::owned_value::OwnedValue;

#[test]
fn runs_compiled_for_in_source() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let total = 0;
    for value in [1, 2, 3] {
        total += value;
    }
    let rewards = { "gold": 4, "xp": 6 };
    for reward in rewards {
        total += reward;
    }
    return total;
}
"#,
        "main",
    )
    .expect("compile for-in source");

    assert_eq!(
        run_linked_test_code(code),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(16)))
    );
}

#[test]
fn runs_compiled_for_in_string_chars() {
    let code = compile_function_source(
        SourceId::new(1),
        r##"
fn main() {
    let total = 0;
    for ch in "a奖励" {
        if ch == 'a' {
            total += 1;
        }
        if ch == '奖' {
            total += 10;
        }
        if ch == '励' {
            total += 100;
        }
    }
    return total;
}
"##,
        "main",
    )
    .expect("compile string for-in source");

    assert_eq!(
        run_linked_test_code(code),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(111)))
    );
}

#[test]
fn runs_compiled_for_in_variant_patterns() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
enum Reward {
    Grant { amount },
    Skip { amount },
}

fn main() {
    let total = 0;
    let rewards = [
        Reward::Grant { amount: 2 },
        Reward::Skip { amount: 100 },
        Reward::Grant { amount: 5 },
    ];
    for Reward::Grant { amount } in rewards {
        total += amount;
    }
    return total;
}
"#,
    )
    .expect("compile for-in variant patterns");
    let mut budget = ExecutionBudget::unbounded();

    assert_eq!(
        run_linked_test_program_with_budget(&Vm::new(), &program, "main", &[], &mut budget),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(7)))
    );
}

#[test]
fn runs_compiled_indexed_for_in_source() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let total = 0;
    for index, value in [2, 3, 5] {
        total += index * 10 + value;
    }
    return total;
}
"#,
        "main",
    )
    .expect("compile indexed for-in source");

    assert_eq!(
        run_linked_test_code(code),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(40)))
    );
}

#[test]
fn indexed_for_in_preserves_source_index_for_pattern_skips() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
enum Reward {
    Grant { amount },
    Skip { amount },
}

fn main() {
    let total = 0;
    let rewards = [
        Reward::Grant { amount: 2 },
        Reward::Skip { amount: 100 },
        Reward::Grant { amount: 5 },
    ];
    for index, Reward::Grant { amount } in rewards {
        total += index + amount;
    }
    return total;
}
"#,
    )
    .expect("compile indexed for-in pattern source");
    let mut budget = ExecutionBudget::unbounded();

    assert_eq!(
        run_linked_test_program_with_budget(&Vm::new(), &program, "main", &[], &mut budget),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(9)))
    );
}

#[test]
fn runs_compiled_statement_attributes_as_metadata() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    #[trace("setup")]
    let total = 1;
    #[audit]
    total += 2;
    return total;
}
"#,
        "main",
    )
    .expect("compile statement attributes");

    assert_eq!(
        run_linked_test_code(code),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(3)))
    );
}

#[test]
fn runs_compiled_for_in_over_native_iterator() {
    let program = compile_standard_program_source_with_native_functions(
        SourceId::new(1),
        r#"
fn main() {
    let total = 0;
    for value in game::values() {
        total += value;
    }
    return total;
}
"#,
        &["game::values"],
    )
    .expect("compile native iterator for-in source");
    let mut vm = Vm::new();
    vm.register_standard_natives();
    vm.register_native("game::values", |_| {
        Ok(OwnedValue::Array(vec![
            OwnedValue::Scalar(vela_common::ScalarValue::I64(2)),
            OwnedValue::Scalar(vela_common::ScalarValue::I64(3)),
            OwnedValue::Scalar(vela_common::ScalarValue::I64(5)),
        ]))
    });
    let mut budget = ExecutionBudget::unbounded();

    assert_eq!(
        run_linked_test_program_with_budget(&vm, &program, "main", &[], &mut budget),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(10)))
    );
}

#[test]
fn runs_compiled_range_for_in_source() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let total = 0;
    for value in 1..4 {
        total += value;
    }
    for value in 4..=5 {
        total += value;
    }
    for value in 2..2 {
        total += 1000;
    }
    for value in 3..=2 {
        total += 1000;
    }
    let count = 0;
    for value in 9223372036854775807..=9223372036854775807 {
        count += 1;
    }
    total += count;
    return total;
}
"#,
        "main",
    )
    .expect("compile range for-in source");

    assert_eq!(
        run_linked_test_code(code),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(16)))
    );
}

#[test]
fn explicit_sequence_methods_create_iterators() {
    let code = compile_function_source(
        SourceId::new(1),
        r##"
fn main() {
    let total = 0;
    for value in [2, 3, 5].iter() {
        total += value;
    }
    for value in {"gold": 7, "xp": 11}.iter() {
        total += value;
    }
    for value in (1..4).iter() {
        total += value;
    }
    for ch in "a奖励".chars() {
        if ch == 'a' {
            total += 100;
        }
        if ch == '奖' {
            total += 1000;
        }
        if ch == '励' {
            total += 10000;
        }
    }
    for byte in "AZ".bytes() {
        total += 1;
    }
    return total;
}
"##,
        "main",
    )
    .expect("compile explicit iterator source");

    assert_eq!(
        run_linked_test_code(code),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(11136)))
    );
}

#[test]
fn iterator_terminal_methods_consume_cursor() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let iter = [2, 3, 5].iter();
    let first = iter.next().unwrap_or(0);
    let remaining = iter.collect_array();
    let exhausted = iter.next().unwrap_or(99);
    let range_count = (1..=4).iter().count();
    return first * 1000 + remaining.len() * 100 + exhausted + range_count;
}
"#,
        "main",
    )
    .expect("compile iterator terminal source");

    assert_eq!(
        run_linked_test_code(code),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2303)))
    );
}

#[test]
fn iterator_lazy_adapters_collect_without_intermediate_arrays() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let values = [1, 2, 3, 4, 5]
        .iter()
        .filter(|value| value > 2)
        .map(|value| value + 10)
        .take(2)
        .collect_array();
    return values.len() * 100 + values[0] * 10 + values[1];
}
"#,
        "main",
    )
    .expect("compile lazy iterator collect source");

    assert_eq!(
        run_linked_test_code(code),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(344)))
    );
}

#[test]
fn iterator_array_sources_read_current_values_without_growth_snapshot() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let values = [1, 2];
    let iter = values.iter();
    values[0] = 9;
    values.push(100);
    let first = iter.next().unwrap_or(0);
    let second = iter.next().unwrap_or(0);
    let third = iter.next().unwrap_or(77);
    return first * 100 + second * 10 + third;
}
"#,
        "main",
    )
    .expect("compile lazy array source iterator");

    assert_eq!(
        run_linked_test_code(code),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(997)))
    );
}

#[test]
fn iterator_map_sources_snapshot_keys_but_read_current_values() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let rewards = { "a": 1, "b": 2 };
    let iter = rewards.iter();
    rewards.set("a", 9);
    rewards.set("c", 100);
    let first = iter.next().unwrap_or(0);
    let second = iter.next().unwrap_or(0);
    let third = iter.next().unwrap_or(77);
    return first * 100 + second * 10 + third;
}
"#,
        "main",
    )
    .expect("compile lazy map source iterator");

    assert_eq!(
        run_linked_test_code(code),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(997)))
    );
}

#[test]
fn map_key_views_snapshot_keys_without_growth() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let rewards = { "a": 1, "b": 2 };
    let keys = rewards.keys();
    rewards.set("c", 3);
    let collected = keys.collect_array();
    if collected.len() == 2 && collected[0] == "a" && collected[1] == "b" {
        return 1;
    }
    return 0;
}
"#,
        "main",
    )
    .expect("compile lazy map key view");

    assert_eq!(
        run_linked_test_code(code),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );
}

#[test]
fn map_entry_views_snapshot_keys_but_read_current_values() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let rewards = { "a": 1, "b": 2 };
    let entries = rewards.entries();
    rewards.set("a", 9);
    rewards.set("c", 100);
    let collected = entries.collect_array();
    if collected.len() == 2 && collected[0].key == "a" && collected[1].key == "b" {
        return collected[0].value * 100 + collected[1].value * 10 + 77;
    }
    return 0;
}
"#,
        "main",
    )
    .expect("compile lazy map entry view");

    assert_eq!(
        run_linked_test_code(code),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(997)))
    );
}

#[test]
fn iterator_lazy_adapters_drive_for_in_and_consume_source() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let iter = [1, 2, 3, 4, 5].iter();
    let pipeline = iter
        .filter(|value| value > 1)
        .map(|value| value * 10)
        .skip(1)
        .take(2);
    let total = 0;
    for value in pipeline {
        total += value;
    }
    let exhausted = iter.next().unwrap_or(99);
    return total * 100 + exhausted;
}
"#,
        "main",
    )
    .expect("compile lazy iterator for-in source");

    assert_eq!(
        run_linked_test_code(code),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(7099)))
    );
}

#[test]
fn iterator_callback_terminals_short_circuit_and_leave_remainder() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let iter = [1, 2, 3, 4].iter().skip(1);
    let has_three = iter.any(|value| value == 3);
    let next = iter.next().unwrap_or(0);
    let found = [1, 5, 9].iter().find(|value| value > 5).unwrap_or(0);
    let all_large = [7, 8, 9].iter().all(|value| value > 6);
    let total = next * 100 + found * 10;
    if has_three {
        total += 1000;
    }
    if all_large {
        total += 1;
    }
    return total;
}
"#,
        "main",
    )
    .expect("compile iterator terminal callbacks source");

    assert_eq!(
        run_linked_test_code(code),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1491)))
    );
}
