use super::*;

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

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(16)));
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
        Ok(Value::Int(7))
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

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(3)));
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
        Ok(Value::Iterator(IteratorState::from_values(vec![
            Value::Int(2),
            Value::Int(3),
            Value::Int(5),
        ])))
    });

    assert_eq!(vm.run(&code), Ok(Value::Int(10)));
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
    return total;
}
"#,
        "main",
    )
    .expect("compile range for-in source");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(15)));
}
