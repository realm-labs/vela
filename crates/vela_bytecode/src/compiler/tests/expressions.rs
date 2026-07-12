use super::*;

#[test]
fn compiler_never_specializes_conflicting_record_shapes_at_cfg_join() {
    let code = compile_test_function(
        SourceId::new(1),
        r#"
struct Left { x: i64 }
struct Right { a: i64, x: i64 }
fn main(flag) {
    let value = if flag { Left { x: 1 } } else { Right { a: 2, x: 3 } };
    return value.x;
}
"#,
        "main",
    )
    .expect("conflicting record-shape join should compile through the generic path");
    assert!(code.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::GetRecordField { ref field, .. } if field == "x"
    )));
    assert!(!code.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::GetRecordSlot { ref field, .. } if field == "x"
    )));
}

#[test]
fn compiler_never_uses_one_predecessor_immediate_after_cfg_join() {
    let code = compile_test_function(
        SourceId::new(2),
        r#"
fn main(flag) -> i64 {
    let step: i64 = if flag { 2 } else { 100 };
    let total: i64 = 0;
    for value in 0..3 {
        total = total + step;
    }
    return total;
}
"#,
        "main",
    )
    .expect("conflicting immediate join should compile");
    assert!(!code.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::I64AddImm { imm: 2 | 100, .. }
            | UnlinkedInstructionKind::I64SubImm { imm: 2 | 100, .. }
            | UnlinkedInstructionKind::I64MulImm { imm: 2 | 100, .. }
    )));
}

#[test]
fn compiler_lowers_unary_operators() {
    let code = compile_test_function(
        SourceId::new(1),
        r#"
fn main() {
    let value = 5;
    return !false == true && -value < 0;
}
"#,
        "main",
    )
    .expect("unary operators should compile");
    assert!(
        code.instructions
            .iter()
            .any(|instruction| { matches!(instruction.kind, UnlinkedInstructionKind::Not { .. }) })
    );
    assert!(
        code.instructions.iter().any(|instruction| {
            matches!(instruction.kind, UnlinkedInstructionKind::Negate { .. })
        })
    );
}

#[test]
fn compiler_evaluates_non_fusible_i64_immediate_lhs_once() {
    for (name, expression) in [
        ("division", "effectful() / 2"),
        ("zero remainder", "effectful() % 0"),
    ] {
        let source = format!(
            "fn effectful() -> i64 {{ return 8; }}\nfn main() -> i64 {{ return {expression}; }}"
        );
        let code = compile_test_function(SourceId::new(1), &source, "main")
            .unwrap_or_else(|error| panic!("{name} should compile: {error:?}"));
        assert_eq!(
            code.instructions
                .iter()
                .filter(|instruction| matches!(
                    instruction.kind,
                    UnlinkedInstructionKind::CallFunction { .. }
                ))
                .count(),
            1,
            "{name} must evaluate its left operand exactly once: {:?}",
            code.instructions
        );
    }
}

#[test]
fn compiler_materializes_negated_equality_before_not() {
    let code = compile_test_function(
        SourceId::new(1),
        r#"
fn main() {
    let label = "tick";
    return !(label != "tick");
}
"#,
        "main",
    )
    .expect("negated equality should compile");

    assert!(code.instructions.iter().any(|instruction| {
        matches!(instruction.kind, UnlinkedInstructionKind::NotEqual { .. })
    }));
    assert!(
        code.instructions
            .iter()
            .any(|instruction| { matches!(instruction.kind, UnlinkedInstructionKind::Not { .. }) })
    );
}

#[test]
fn compiler_rejects_static_record_equality_without_partial_eq() {
    let error = compile_test_program(
        SourceId::new(1),
        r#"
struct Reward { amount: i64 }

fn main() {
    let left = Reward { amount: 1 };
    let right = Reward { amount: 1 };
    return left == right;
}
"#,
    )
    .expect_err("known record equality without PartialEq should be a compile error");

    assert_eq!(
        semantic_diagnostic_codes(error),
        ["compiler::missing_comparison_trait"]
    );
}

#[test]
fn compiler_accepts_static_record_equality_with_derived_partial_eq() {
    compile_test_program(
        SourceId::new(1),
        r#"
#[derive(PartialEq)]
struct Reward { amount: i64 }

fn main() {
    let left = Reward { amount: 1 };
    let right = Reward { amount: 1 };
    return left == right;
}
"#,
    )
    .expect("known record equality with derived PartialEq should compile");
}

#[test]
fn compiler_rejects_static_record_ordering_without_partial_ord() {
    let error = compile_test_program(
        SourceId::new(1),
        r#"
struct Score { value: i64 }

fn main() {
    let left = Score { value: 1 };
    let right = Score { value: 2 };
    return left < right;
}
"#,
    )
    .expect_err("known record ordering without PartialOrd should be a compile error");

    assert_eq!(
        semantic_diagnostic_codes(error),
        ["compiler::missing_comparison_trait"]
    );
}

#[test]
fn compiler_accepts_static_record_ordering_with_derived_partial_ord() {
    compile_test_program(
        SourceId::new(1),
        r#"
#[derive(PartialEq, PartialOrd)]
struct Score { value: i64 }

fn main() {
    let left = Score { value: 1 };
    let right = Score { value: 2 };
    return left < right;
}
"#,
    )
    .expect("known record ordering with derived PartialOrd should compile");
}

#[test]
fn compiler_lowers_identity_comparison_operators() {
    let code = compile_test_function(
        SourceId::new(1),
        r#"
fn main(left, right) {
    return left === right || left !== right;
}
"#,
        "main",
    )
    .expect("identity comparisons should compile");

    assert!(code.instructions.iter().any(|instruction| {
        matches!(
            instruction.kind,
            UnlinkedInstructionKind::IdentityEqual { .. }
        )
    }));
    assert!(code.instructions.iter().any(|instruction| {
        matches!(
            instruction.kind,
            UnlinkedInstructionKind::IdentityNotEqual { .. }
        )
    }));
}

#[test]
fn compiler_rejects_static_non_reference_identity_comparison() {
    let error = compile_test_function(
        SourceId::new(1),
        r#"
fn main() {
    return 1 === 1;
}
"#,
        "main",
    )
    .expect_err("static scalar identity comparison should be a compile error");

    assert_eq!(
        semantic_diagnostic_codes(error),
        ["compiler::invalid_identity_comparison"]
    );
}

#[test]
fn compiler_materializes_negated_identity_equality_before_not() {
    let code = compile_test_function(
        SourceId::new(1),
        r#"
fn main(left, right) {
    return !(left === right);
}
"#,
        "main",
    )
    .expect("negated identity comparison should compile");

    assert!(code.instructions.iter().any(|instruction| {
        matches!(
            instruction.kind,
            UnlinkedInstructionKind::IdentityEqual { .. }
        )
    }));
    assert!(
        code.instructions
            .iter()
            .any(|instruction| matches!(instruction.kind, UnlinkedInstructionKind::Not { .. }))
    );
}

#[test]
fn compiler_lowers_logical_short_circuit_operators() {
    let code = compile_test_function(
        SourceId::new(1),
        r#"
fn main() {
    return false && fail() || true;
}
"#,
        "main",
    )
    .expect("logical operators should compile");
    assert!(code.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::JumpIfFalse { .. }
    )));
    assert!(
        code.instructions.iter().any(|instruction| {
            matches!(instruction.kind, UnlinkedInstructionKind::Jump { .. })
        })
    );
    assert!(
        code.instructions.iter().any(|instruction| {
            matches!(instruction.kind, UnlinkedInstructionKind::Truthy { .. })
        })
    );
    assert!(
        !code
            .instructions
            .iter()
            .any(|instruction| { matches!(instruction.kind, UnlinkedInstructionKind::Not { .. }) })
    );
    assert!(code.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::CallNative { ref name, .. } if name == "fail"
    )));
}
#[test]
fn compiler_lowers_block_and_if_expression_values() {
    let code = compile_test_function(
        SourceId::new(1),
        r#"
fn main() {
    let value = {
        let base = 2;
        base + 3
    };
    return if value > 4 {
        value
    } else {
        0
    };
}
"#,
        "main",
    )
    .expect("block and if expression values should compile");
    assert!(code.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::JumpIfFalse { .. }
    )));
    assert!(
        code.instructions
            .iter()
            .any(|instruction| matches!(instruction.kind, UnlinkedInstructionKind::Move { .. }))
    );
}
#[test]
fn compiler_lowers_if_expression_without_else_to_unit() {
    let code = compile_test_function(
        SourceId::new(1),
        r#"
fn main() {
    let value = if false {
        1;
    };
    return value;
}
"#,
        "main",
    )
    .expect("if expression without else should compile");
    assert!(code.constants.contains(&Constant::Unit));
}
#[test]
fn compiler_lowers_returning_block_initializers() {
    let code = compile_test_function(
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
    .expect("returning block initializer should compile");
    assert!(
        code.instructions
            .iter()
            .any(|instruction| matches!(instruction.kind, UnlinkedInstructionKind::Return { .. }))
    );
}
#[test]
fn compiler_lowers_returning_expression_operands() {
    let code = compile_test_function(
        SourceId::new(1),
        r#"
fn main(kind) {
    log({
        return 7;
    });
    if kind == "if" {
        return if true {
            return 1;
        } else {
            return 2;
        };
    }
    return match kind {
        "match" => { return 3; },
        _ => { return 4; },
    };
}
"#,
        "main",
    )
    .expect("returning expression operands should compile");
    assert!(
        code.instructions
            .iter()
            .any(|instruction| matches!(instruction.kind, UnlinkedInstructionKind::Return { .. }))
    );
}
#[test]
fn compiler_lowers_returning_if_and_match_initializers() {
    compile_test_function(
        SourceId::new(1),
        r#"
fn main(flag) {
    let ignored = if flag {
        return 7;
    } else {
        return 8;
    };
    return 0;
}
"#,
        "main",
    )
    .expect("returning if initializer should compile");
    compile_test_function(
        SourceId::new(2),
        r#"
fn main(value) {
    let ignored = match value {
        1 => { return 10; },
        _ => { return 11; },
    };
    return 0;
}
"#,
        "main",
    )
    .expect("returning match initializer should compile");
}
#[test]
fn compiler_lowers_match_expression_values() {
    let code = compile_test_function(
        SourceId::new(1),
        r#"
fn main() {
    let damage = Damage::Physical { amount: 7 };
    let value = match damage {
        Damage::Magical { amount } => amount + 100,
        Damage::Physical { amount } => {
            amount + 1;
        },
        _ => 0,
    };
    return value;
}
"#,
        "main",
    )
    .expect("match expression values should compile");
    assert!(code.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::EnumTagEqual { .. }
    )));
    assert!(
        code.instructions
            .iter()
            .any(|instruction| matches!(instruction.kind, UnlinkedInstructionKind::Move { .. }))
    );
}
#[test]
fn compiler_lowers_literal_match_patterns() {
    let code = compile_test_function(
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
    .expect("literal match patterns should compile");
    assert!(
        code.instructions
            .iter()
            .any(|instruction| matches!(instruction.kind, UnlinkedInstructionKind::Equal { .. }))
    );
    assert!(
        code.instructions
            .iter()
            .filter(|instruction| matches!(
                instruction.kind,
                UnlinkedInstructionKind::JumpIfFalse { .. }
            ))
            .count()
            >= 2
    );
}
#[test]
fn compiler_lowers_binding_match_patterns() {
    let code = compile_test_function(
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
    .expect("binding match patterns should compile");
    assert!(
        code.instructions
            .iter()
            .any(|instruction| matches!(instruction.kind, UnlinkedInstructionKind::Move { .. }))
    );
    assert!(code.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::Add { .. }
            | UnlinkedInstructionKind::I64AddImm { .. }
            | UnlinkedInstructionKind::BinaryIntLiteral {
                op: crate::BinaryLiteralOp::Add,
                ..
            }
    )));
}
#[test]
fn compiler_lowers_match_guards() {
    let code = compile_test_function(
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
    .expect("match guards should compile");
    assert!(
        code.instructions
            .iter()
            .filter(|instruction| matches!(
                instruction.kind,
                UnlinkedInstructionKind::JumpIfFalse { .. }
            ))
            .count()
            >= 2
    );
    assert!(code.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::Less { .. }
            | UnlinkedInstructionKind::I64CmpImm {
                op: crate::I64CompareOp::Less,
                ..
            }
            | UnlinkedInstructionKind::BinaryIntLiteral {
                op: crate::BinaryLiteralOp::Less,
                ..
            }
    )));
}
#[test]
fn compiler_lowers_record_variant_field_patterns() {
    let code = compile_test_function(
        SourceId::new(1),
        r#"
enum Reward {
    Grant { kind, amount }
}
fn main() {
    let reward = Reward::Grant { kind: "xp", amount: 7 };
    return match reward {
        Reward::Grant { kind: "gold", amount } => amount,
        Reward::Grant { kind: "xp", amount } => amount + 1,
        _ => 0,
    };
}
"#,
        "main",
    )
    .expect("record variant field patterns should compile");
    assert!(
        code.instructions
            .iter()
            .any(|instruction| matches!(instruction.kind, UnlinkedInstructionKind::Equal { .. }))
    );
    assert!(
        code.instructions
            .iter()
            .filter(|instruction| {
                matches!(
                    instruction.kind,
                    UnlinkedInstructionKind::GetEnumSlot { .. }
                )
            })
            .count()
            >= 2
    );
}
#[test]
fn compiler_lowers_tuple_variant_constructors_and_patterns() {
    let code = compile_test_function(
        SourceId::new(1),
        r#"
enum Damage {
    Physical(amount, bonus),
    Magical(amount),
}
fn main() {
    let damage = Damage::Physical(7, 2);
    return match damage {
        Damage::Physical(amount, bonus) => amount + bonus,
        _ => 0,
    };
}
"#,
        "main",
    )
    .expect("tuple variant constructor and pattern should compile");
    assert!(
        code.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            UnlinkedInstructionKind::MakeEnum { .. }
        ))
    );
    assert!(
        code.instructions
            .iter()
            .filter(|instruction| {
                matches!(
                    instruction.kind,
                    UnlinkedInstructionKind::GetEnumSlot { .. }
                )
            })
            .count()
            >= 2
    );
}

#[test]
fn compiler_lowers_unit_and_tuple_expressions() {
    let unit = compile_test_function(
        SourceId::new(1),
        "fn unit_value() { return (); }",
        "unit_value",
    )
    .expect("unit expression should compile");
    assert!(unit.constants.contains(&Constant::Unit));

    let tuple = compile_test_function(
        SourceId::new(1),
        r#"fn pair() { return (1, "xp"); }"#,
        "pair",
    )
    .expect("tuple expression should compile");
    assert!(
        tuple.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            UnlinkedInstructionKind::MakeTuple { .. }
        ))
    );
}

#[test]
fn compiler_lowers_tuple_destructuring_patterns() {
    let code = compile_test_function(
        SourceId::new(1),
        r#"
fn main() {
    let (amount, label) = (3, "xp");
    return amount;
}
"#,
        "main",
    )
    .expect("tuple let destructuring should compile");
    assert!(code.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::GuardTupleArity { arity: 2, .. }
    )));
    assert!(code.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::GetTupleField { index: 0, .. }
    )));

    let code = compile_test_function(
        SourceId::new(1),
        r#"
fn main() {
    let pair = (2, 5);
    return match pair {
        (left, right) => left + right,
        _ => 0,
    };
}
"#,
        "main",
    )
    .expect("tuple match destructuring should compile");
    assert!(code.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::TupleArityEqual { arity: 2, .. }
    )));
}

#[test]
fn compiler_lowers_tuple_projection_field_reads() {
    let code = compile_test_function(
        SourceId::new(1),
        r#"
fn main() {
    let pair = (2, 5);
    return pair.0 + pair.1;
}
"#,
        "main",
    )
    .expect("tuple projection should compile");
    assert!(code.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::GetTupleField { index: 0, .. }
    )));
    assert!(code.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::GetTupleField { index: 1, .. }
    )));
}

mod assignments_and_access;
