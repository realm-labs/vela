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

    assert_eq!(Vm::new().run(&code), Ok(OwnedValue::Int(16)));
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

    assert_eq!(
        Vm::new().run_program(&program, "main", &[]),
        Ok(OwnedValue::Int(7))
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

    assert_eq!(Vm::new().run(&code), Ok(OwnedValue::Int(40)));
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

    assert_eq!(
        Vm::new().run_program(&program, "main", &[]),
        Ok(OwnedValue::Int(9))
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

    assert_eq!(Vm::new().run(&code), Ok(OwnedValue::Int(3)));
}

#[test]
fn runs_compiled_for_in_over_native_iterator() {
    let code = compile_function_source(
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
        "main",
    )
    .expect("compile native iterator for-in source");
    let mut vm = Vm::new();
    vm.register_native("game::values", |_| {
        Ok(OwnedValue::Array(vec![
            OwnedValue::Int(2),
            OwnedValue::Int(3),
            OwnedValue::Int(5),
        ]))
    });

    assert_eq!(vm.run(&code), Ok(OwnedValue::Int(10)));
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

    assert_eq!(Vm::new().run(&code), Ok(OwnedValue::Int(16)));
}
