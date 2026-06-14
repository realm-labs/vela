use super::*;

#[test]
fn compiler_lowers_for_in_loops() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let total = 0;
    for value in [1, 2, 3] {
        total += value;
    }
    return total;
}
"#,
        "main",
    )
    .expect("for-in loop should compile");
    assert!(
        code.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            UnlinkedInstructionKind::IterInit { .. }
        ))
    );
    assert!(
        code.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            UnlinkedInstructionKind::IterNext { .. }
        ))
    );
}

#[test]
fn compiler_lowers_direct_range_for_in_to_i64_range_next() {
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
    .expect("range for-in loop should compile");
    assert!(code.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::I64RangeNext { .. }
    )));
    assert!(
        !code.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            UnlinkedInstructionKind::IterInit { .. }
        ))
    );
}

#[test]
fn compiler_lowers_proven_i64_scalar_loop_ops_to_typed_bytecode() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let total = 0;
    for value in 0..200 {
        if value % 3 == 0 {
            total += value * 2;
            continue;
        }
        if value > 180 {
            break;
        }
        total += (value * 5) % 17;
    }
    return total;
}
"#,
        "main",
    )
    .expect("proven i64 scalar loop should compile");

    assert!(code.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::I64RemImm { imm: 3, .. }
    )));
    assert!(code.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::I64MulImm { imm: 2, .. }
    )));
    assert!(code.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::I64CmpImmJumpIfFalse {
            op: crate::I64CompareOp::Greater,
            imm: 180,
            ..
        }
    )));
    assert!(
        code.instructions
            .iter()
            .any(|instruction| matches!(instruction.kind, UnlinkedInstructionKind::I64Add { .. }))
    );
}

#[test]
fn compiler_keeps_dynamic_numeric_ops_generic() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn add(left, right) {
    return left + right;
}
"#,
        "add",
    )
    .expect("dynamic add should compile");

    assert!(
        code.instructions
            .iter()
            .any(|instruction| { matches!(instruction.kind, UnlinkedInstructionKind::Add { .. }) })
    );
    assert!(!code.instructions.iter().any(|instruction| {
        matches!(
            instruction.kind,
            UnlinkedInstructionKind::I64Add { .. } | UnlinkedInstructionKind::I64AddImm { .. }
        )
    }));
}

#[test]
fn compiler_lowers_for_in_patterns() {
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
    .expect("for-in pattern should compile");
    let main = program.function("main").expect("main function");
    assert!(main.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::EnumTagEqual { .. }
    )));
    assert!(main.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::GetEnumSlot { ref field, .. } if field == "amount"
    )));
}
#[test]
fn compiler_lowers_break_and_continue() {
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
    .expect("break and continue should compile");
    assert!(
        code.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            UnlinkedInstructionKind::IterNext { .. }
        ))
    );
    assert!(
        code.instructions
            .iter()
            .filter(|instruction| matches!(instruction.kind, UnlinkedInstructionKind::Jump { .. }))
            .count()
            >= 3
    );
}
#[test]
fn compiler_rejects_break_and_continue_outside_loop() {
    let break_error = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    break;
}
"#,
        "main",
    )
    .expect_err("break outside loop should be rejected");
    assert_eq!(
        break_error.kind,
        CompileErrorKind::UnsupportedSyntax("break outside loop")
    );
    let continue_error = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    continue;
}
"#,
        "main",
    )
    .expect_err("continue outside loop should be rejected");
    assert_eq!(
        continue_error.kind,
        CompileErrorKind::UnsupportedSyntax("continue outside loop")
    );
}
#[test]
fn compiler_rejects_top_level_mutation_as_syntax_before_codegen() {
    let error = compile_program_source(
        SourceId::new(1),
        r#"
player.level = 10;
fn main(player) { return player.level; }
"#,
    )
    .expect_err("top-level mutation should not reach bytecode generation");
    let CompileErrorKind::SyntaxDiagnostics(diagnostics) = error.kind else {
        panic!("expected syntax diagnostics");
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("expected item"))
    );
}
#[test]
fn compiler_rejects_top_level_const_side_effects_from_hir() {
    let error = compile_program_source(
        SourceId::new(1),
        r#"
const BAD = register_event("monster.kill");
fn main() { return 1; }
"#,
    )
    .expect_err("side-effecting const initializer should fail before bytecode generation");
    let CompileErrorKind::SemanticDiagnostics(diagnostics) = error.kind else {
        panic!("expected semantic diagnostics");
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code.as_deref() == Some("hir::top_level_side_effect") })
    );
}
#[test]
fn compiler_rejects_generic_type_hints_before_codegen() {
    let error = compile_program_source(
        SourceId::new(1),
        r#"
fn main(values: Player<i64>) {
    return values;
}
"#,
    )
    .expect_err("generic type hints should fail in syntax validation");
    let CompileErrorKind::SyntaxDiagnostics(diagnostics) = error.kind else {
        panic!("expected syntax diagnostics");
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code.as_deref() == Some("syntax::generic_type_hint") })
    );
}
