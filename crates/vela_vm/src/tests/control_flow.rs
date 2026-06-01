use super::*;

#[test]
fn runs_compiled_break_continue_source() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let total = 0;
    for value in [1, 2, 3, 4, 5] {
        if value == 2 {
            continue;
        }
        if value == 5 {
            break;
        }
        total += value;
    }
    return total;
}
"#,
        "main",
    )
    .expect("compile break and continue source");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(8)));
}

#[test]
fn runs_compiled_block_and_if_expression_values() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let value = {
        let base = 2;
        base + 3;
    };
    let selected = if value > 4 {
        value;
    } else {
        0;
    };
    return selected;
}
"#,
        "main",
    )
    .expect("compile block and if expression values");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(5)));
}

#[test]
fn runs_compiled_if_expression_without_else_as_null() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let missing = if false {
        3;
    };
    let value = if true {
        7;
    };
    if missing == null {
        return value;
    }
    return 0;
}
"#,
        "main",
    )
    .expect("compile no-else if expression");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(7)));
}

#[test]
fn runs_compiled_if_expression_without_else_false_branch_as_null() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let value = if false {
        7;
    };
    if value == null {
        return 1;
    }
    return 0;
}
"#,
        "main",
    )
    .expect("compile no-else if expression");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(1)));
}

#[test]
fn runs_compiled_returning_block_initializer() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let ignored = {
        return 7;
    };
    return 0;
}
"#,
        "main",
    )
    .expect("compile returning block initializer");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(7)));
}

#[test]
fn runs_compiled_returning_expression_operands() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn block_arg() {
    log({
        return 7;
    });
    return 0;
}

fn if_value(flag) {
    return if flag {
        return 1;
    } else {
        return 2;
    };
}

fn match_value(value) {
    return match value {
        1 => { return 10; },
        _ => { return 11; },
    };
}
"#,
    )
    .expect("compile returning expression operands");

    assert_eq!(
        Vm::new().run_program(&program, "block_arg", &[]),
        Ok(Value::Int(7))
    );
    assert_eq!(
        Vm::new().run_program(&program, "if_value", &[Value::Bool(true)]),
        Ok(Value::Int(1))
    );
    assert_eq!(
        Vm::new().run_program(&program, "if_value", &[Value::Bool(false)]),
        Ok(Value::Int(2))
    );
    assert_eq!(
        Vm::new().run_program(&program, "match_value", &[Value::Int(1)]),
        Ok(Value::Int(10))
    );
    assert_eq!(
        Vm::new().run_program(&program, "match_value", &[Value::Int(9)]),
        Ok(Value::Int(11))
    );
}

#[test]
fn runs_compiled_returning_if_and_match_initializers() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn if_case(flag) {
    let ignored = if flag {
        return 7;
    } else {
        return 8;
    };
    return 0;
}

fn match_case(value) {
    let ignored = match value {
        1 => { return 10; },
        _ => { return 11; },
    };
    return 0;
}
"#,
    )
    .expect("compile returning if and match initializers");

    assert_eq!(
        Vm::new().run_program(&program, "if_case", &[Value::Bool(true)]),
        Ok(Value::Int(7))
    );
    assert_eq!(
        Vm::new().run_program(&program, "if_case", &[Value::Bool(false)]),
        Ok(Value::Int(8))
    );
    assert_eq!(
        Vm::new().run_program(&program, "match_case", &[Value::Int(1)]),
        Ok(Value::Int(10))
    );
    assert_eq!(
        Vm::new().run_program(&program, "match_case", &[Value::Int(2)]),
        Ok(Value::Int(11))
    );
}

#[test]
fn runs_compiled_match_expression_values() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let damage = Damage.Physical { amount: 7 };
    let value = match damage {
        Damage.Magical { amount } => amount + 100,
        Damage.Physical { amount } => {
            amount + 1;
        },
        _ => 0,
    };
    return value;
}
"#,
        "main",
    )
    .expect("compile match expression values");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(8)));
}

#[test]
fn runs_compiled_literal_match_patterns() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let value = 2;
    return match value {
        1 => 10,
        2 => 20,
        _ => 0,
    };
}
"#,
        "main",
    )
    .expect("compile literal match patterns");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(20)));
}

#[test]
fn managed_heap_execution_runs_string_literal_match_patterns() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    let label = "xp";
    return match label {
        "gold" => 1,
        "xp" => 2,
        _ => 0,
    };
}
"#,
    )
    .expect("compile heap string literal match patterns");
    let mut budget = ExecutionBudget::unbounded();

    assert_eq!(
        Vm::new()
            .run_program_with_managed_heap_and_budget(&program, "main", &[], &mut budget)
            .expect("run heap string literal match patterns"),
        Value::Int(2)
    );
    assert_eq!(budget.memory_bytes_allocated(), 0);
}

#[test]
fn runs_compiled_binding_match_patterns() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let value = 7;
    return match value {
        bound => bound + 1,
    };
}
"#,
        "main",
    )
    .expect("compile binding match patterns");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(8)));
}

#[test]
fn binding_match_assignment_does_not_mutate_scrutinee() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let value = 7;
    match value {
        bound => {
            bound = 100;
        }
    }
    return value;
}
"#,
        "main",
    )
    .expect("compile binding match assignment");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(7)));
}

#[test]
fn runs_compiled_match_guards() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let value = 7;
    return match value {
        bound if bound < 5 => 10,
        bound if bound == 7 => bound + 1,
        _ => 0,
    };
}
"#,
        "main",
    )
    .expect("compile match guards");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(8)));
}

#[test]
fn match_guards_can_read_record_pattern_bindings() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let damage = Damage.Physical { amount: 7 };
    return match damage {
        Damage.Physical { amount } if amount > 10 => 100,
        Damage.Physical { amount } if amount == 7 => amount + 1,
        _ => 0,
    };
}
"#,
        "main",
    )
    .expect("compile tuple variant literal pattern");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(8)));
}

#[test]
fn runs_compiled_record_variant_field_patterns() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
enum Reward {
    Grant { kind, amount }
}

fn main() {
    let reward = Reward.Grant { kind: "xp", amount: 7 };
    return match reward {
        Reward.Grant { kind: "gold", amount } => amount,
        Reward.Grant { kind: "xp", amount } => amount + 1,
        _ => 0,
    };
}
"#,
        "main",
    )
    .expect("compile record variant field patterns");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(8)));
}

#[test]
fn managed_heap_execution_runs_nested_record_variant_field_patterns() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
enum Reward {
    Grant { payload }
}

enum Payload {
    Xp(amount)
    Gold(amount)
}

fn main() {
    let reward = Reward.Grant { payload: Payload.Xp(7) };
    return match reward {
        Reward.Grant { payload: Payload.Gold(amount) } => amount,
        Reward.Grant { payload: Payload.Xp(amount) } => amount + 1,
        _ => 0,
    };
}
"#,
    )
    .expect("compile nested record variant field patterns");
    let mut budget = ExecutionBudget::new(10_000, 32_000, 32, 32);

    assert_eq!(
        Vm::new().run_program_with_managed_heap_and_budget(&program, "main", &[], &mut budget),
        Ok(Value::Int(8))
    );
}

#[test]
fn runs_compiled_tuple_variant_constructor_and_patterns() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
enum Damage {
    Physical(amount, bonus),
    Magical(amount),
}

fn main() {
    let damage = Damage.Physical(7, 2);
    return match damage {
        Damage.Physical(amount, bonus) => amount + bonus,
        _ => 0,
    };
}
"#,
        "main",
    )
    .expect("compile tuple variant constructor and pattern");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(9)));
}

#[test]
fn managed_heap_execution_runs_tuple_variant_literal_patterns() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
enum Damage {
    Typed(kind, amount),
}

fn main() {
    let damage = Damage.Typed("fire", 7);
    return match damage {
        Damage.Typed("frost", amount) => amount + 100,
        Damage.Typed("fire", amount) => amount + 1,
        _ => 0,
    };
}
"#,
        "main",
    )
    .expect("compile guarded record pattern");

    let mut budget = ExecutionBudget::new(10_000, 32_000, 32, 32);
    assert_eq!(
        Vm::new().run_with_managed_heap_and_budget(&code, &mut budget),
        Ok(Value::Int(8))
    );
}
