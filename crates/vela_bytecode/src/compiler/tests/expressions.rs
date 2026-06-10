use super::*;

#[test]
fn compiler_lowers_unary_operators() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    return !false == true && -5 < 0;
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
fn compiler_inverts_negated_equality_without_not_instruction() {
    let code = compile_function_source(
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

    assert!(
        code.instructions.iter().any(|instruction| {
            matches!(instruction.kind, UnlinkedInstructionKind::Equal { .. })
        })
    );
    assert!(
        !code
            .instructions
            .iter()
            .any(|instruction| { matches!(instruction.kind, UnlinkedInstructionKind::Not { .. }) })
    );
}

#[test]
fn compiler_lowers_logical_short_circuit_operators() {
    let code = compile_function_source(
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
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let value = {
        let base = 2;
        base + 3;
    };
    return if value > 4 {
        value;
    } else {
        0;
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
fn compiler_lowers_if_expression_without_else_to_null() {
    let code = compile_function_source(
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
    assert!(code.constants.contains(&Constant::Null));
}
#[test]
fn compiler_lowers_returning_block_initializers() {
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
    .expect("returning block initializer should compile");
    assert!(
        code.instructions
            .iter()
            .any(|instruction| matches!(instruction.kind, UnlinkedInstructionKind::Return { .. }))
    );
}
#[test]
fn compiler_lowers_returning_expression_operands() {
    let code = compile_function_source(
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
    compile_function_source(
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
    compile_function_source(
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
    let code = compile_function_source(
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
    .expect("binding match patterns should compile");
    assert!(
        code.instructions
            .iter()
            .any(|instruction| matches!(instruction.kind, UnlinkedInstructionKind::Move { .. }))
    );
    assert!(
        code.instructions
            .iter()
            .any(|instruction| matches!(instruction.kind, UnlinkedInstructionKind::Add { .. }))
    );
}
#[test]
fn compiler_lowers_match_guards() {
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
    assert!(
        code.instructions
            .iter()
            .any(|instruction| matches!(instruction.kind, UnlinkedInstructionKind::Less { .. }))
    );
}
#[test]
fn compiler_lowers_record_variant_field_patterns() {
    let code = compile_function_source(
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
    let code = compile_function_source(
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
fn compiler_lowers_local_assignment_operators() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let value = 1;
    value += 4;
    value *= 3;
    value -= 5;
    value /= 2;
    value %= 5;
    let copy = (value = value + 10);
    return value + copy;
}
"#,
        "main",
    )
    .expect("local assignments should compile");
    assert!(
        code.instructions
            .iter()
            .any(|instruction| matches!(instruction.kind, UnlinkedInstructionKind::Add { .. }))
    );
    assert!(
        code.instructions
            .iter()
            .any(|instruction| matches!(instruction.kind, UnlinkedInstructionKind::Sub { .. }))
    );
    assert!(
        code.instructions
            .iter()
            .any(|instruction| matches!(instruction.kind, UnlinkedInstructionKind::Mul { .. }))
    );
    assert!(
        code.instructions
            .iter()
            .any(|instruction| matches!(instruction.kind, UnlinkedInstructionKind::Div { .. }))
    );
    assert!(
        code.instructions
            .iter()
            .any(|instruction| matches!(instruction.kind, UnlinkedInstructionKind::Rem { .. }))
    );
}
#[test]
fn compiler_lowers_index_reads() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let values = [2, 4, 8];
    let rewards = { "xp": 6 };
    return values[1] + rewards["xp"];
}
"#,
        "main",
    )
    .expect("index reads should compile");
    assert!(
        code.instructions
            .iter()
            .filter(|instruction| matches!(
                instruction.kind,
                UnlinkedInstructionKind::GetIndex { .. }
            ))
            .count()
            >= 2
    );
}
#[test]
fn compiler_keeps_call_result_index_reads_off_host_paths() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn values() {
    return [{ "name": "Damageable" }];
}
fn main() {
    return values()[0].name;
}
"#,
        "main",
    )
    .expect("call result index read should compile");
    assert!(
        code.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            UnlinkedInstructionKind::GetIndex { .. }
        ))
    );
    assert!(
        !code.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            UnlinkedInstructionKind::HostRead { .. }
        ))
    );
}
#[test]
fn compiler_lowers_index_writes() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let values = [2, 4, 8];
    values[1] = 10;
    values[2] += 5;
    return values[1] + values[2];
}
"#,
        "main",
    )
    .expect("index writes should compile");
    assert!(
        code.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            UnlinkedInstructionKind::SetIndex { .. }
        ))
    );
}
#[test]
fn compiler_lowers_record_field_writes() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let reward = Reward { item_id: "gold", count: 2 };
    reward.count += 3;
    reward.item_id = "xp";
    return reward.count;
}
"#,
        "main",
    )
    .expect("record field writes should compile");
    assert!(code.instructions.iter().any(|instruction| {
        matches!(
            instruction.kind,
            UnlinkedInstructionKind::SetRecordSlot {
                ref field,
                slot: 0,
                ..
            } if field == "count"
        )
    }));
    assert!(code.instructions.iter().any(|instruction| {
        matches!(
            instruction.kind,
            UnlinkedInstructionKind::SetRecordSlot {
                ref field,
                slot: 1,
                ..
            } if field == "item_id"
        )
    }));
    assert!(!code.instructions.iter().any(|instruction| {
        matches!(
            instruction.kind,
            UnlinkedInstructionKind::SetRecordField { .. }
        )
    }));
}
#[test]
fn compiler_lowers_nested_record_field_writes() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let player = Player {
        stats: Stats {
            level: 2,
            exp: 5,
        },
    };
    player.stats.level += 3;
    player.stats.exp = player.stats.level + 1;
    return player.stats.level + player.stats.exp;
}
"#,
        "main",
    )
    .expect("nested record field writes should compile");
    assert!(
        code.instructions
            .iter()
            .filter(|instruction| {
                matches!(
                    instruction.kind,
                    UnlinkedInstructionKind::SetRecordSlot { .. }
                )
            })
            .count()
            >= 3
    );
    assert!(!code.instructions.iter().any(|instruction| {
        matches!(
            instruction.kind,
            UnlinkedInstructionKind::GetRecordField { .. }
                | UnlinkedInstructionKind::SetRecordField { .. }
        )
    }));
}
#[test]
fn compiler_lowers_indexed_record_field_writes() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let players = [
        Player { level: 2, exp: 5 },
        Player { level: 7, exp: 1 },
    ];
    players[0].level += 3;
    players[1].exp = players[0].level + 4;
    return players[0].level + players[1].exp;
}
"#,
        "main",
    )
    .expect("indexed record field writes should compile");
    assert!(
        code.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            UnlinkedInstructionKind::SetIndex { .. }
        ))
    );
    assert!(code.instructions.iter().any(|instruction| {
        matches!(
            instruction.kind,
            UnlinkedInstructionKind::SetRecordSlot { .. }
        )
    }));
    assert!(!code.instructions.iter().any(|instruction| {
        matches!(
            instruction.kind,
            UnlinkedInstructionKind::GetRecordField { .. }
                | UnlinkedInstructionKind::SetRecordField { .. }
        )
    }));
}
#[test]
fn compiler_lowers_immediate_record_field_reads_to_slots() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    return Reward { item_id: "gold", count: 2 }.count;
}
"#,
        "main",
    )
    .expect("immediate record field read should compile");
    assert!(code.instructions.iter().any(|instruction| {
        matches!(
            instruction.kind,
            UnlinkedInstructionKind::GetRecordSlot {
                ref field,
                slot: 0,
                ..
            } if field == "count"
        )
    }));
}
#[test]
fn compiler_lowers_immediate_enum_field_reads_to_slots() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    return Damage::Physical { amount: 7 }.amount;
}
"#,
        "main",
    )
    .expect("immediate enum field read should compile");
    assert!(code.instructions.iter().any(|instruction| {
        matches!(
            instruction.kind,
            UnlinkedInstructionKind::GetEnumSlot {
                ref field,
                slot: 0,
                ..
            } if field == "amount"
        )
    }));
}
#[test]
fn compiler_lowers_typed_enum_variant_field_reads_to_slots() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
enum Damage {
    Physical { amount: int, element: string },
    Magical { amount: int },
}
fn main() {
    let damage = Damage::Physical { amount: 7, element: "slash" };
    return damage.amount;
}
"#,
    )
    .expect("typed enum variant field read should compile to slot bytecode");
    let main = program.function("main").expect("main function");
    assert!(main.instructions.iter().any(|instruction| {
        matches!(
            instruction.kind,
            UnlinkedInstructionKind::GetEnumSlot {
                ref field,
                slot: 0,
                ..
            } if field == "amount"
        )
    }));
    assert!(!main.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::GetEnumField { .. }
    )));
}
#[test]
fn compiler_lowers_typed_record_field_reads_to_slots() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
struct Reward {
    item_id: string,
    count: int,
}
fn make_reward() {
    return Reward { item_id: "gold", count: 2 };
}
fn main() {
    let reward: Reward = make_reward();
    return reward.count;
}
"#,
    )
    .expect("typed record field read should compile to slot bytecode");
    let main = program.function("main").expect("main function");
    assert!(main.instructions.iter().any(|instruction| {
        matches!(
            instruction.kind,
            UnlinkedInstructionKind::GetRecordSlot {
                ref field,
                slot: 0,
                ..
            } if field == "count"
        )
    }));
}
#[test]
fn compiler_lowers_typed_record_field_writes_to_slots() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
struct Reward {
    item_id: string,
    count: int,
}
fn make_reward() {
    return Reward { item_id: "gold", count: 2 };
}
fn main() {
    let reward: Reward = make_reward();
    reward.count += 3;
    reward.item_id = "xp";
    return reward.count;
}
"#,
    )
    .expect("typed record field writes should compile to slot bytecode");
    let main = program.function("main").expect("main function");
    assert!(main.instructions.iter().any(|instruction| {
        matches!(
            instruction.kind,
            UnlinkedInstructionKind::SetRecordSlot {
                ref field,
                slot: 0,
                ..
            } if field == "count"
        )
    }));
    assert!(main.instructions.iter().any(|instruction| {
        matches!(
            instruction.kind,
            UnlinkedInstructionKind::SetRecordSlot {
                ref field,
                slot: 1,
                ..
            } if field == "item_id"
        )
    }));
    assert!(!main.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::SetRecordField { .. }
    )));
}
