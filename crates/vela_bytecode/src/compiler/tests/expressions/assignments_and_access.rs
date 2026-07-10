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
            .any(|instruction| matches!(instruction.kind, UnlinkedInstructionKind::I64Sub { .. }))
    );
    assert!(
        code.instructions
            .iter()
            .any(|instruction| matches!(instruction.kind, UnlinkedInstructionKind::I64Mul { .. }))
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
            >= 1
    );
    assert!(code.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::GetStringKeyIndex { .. }
    )));
}

#[test]
fn compiler_resolves_index_read_operands_from_hir() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let values = [2, 4, 8];
    let index = 1;
    return values[index];
}
"#,
        "main",
    )
    .expect("HIR-backed index read should compile");
    assert!(
        code.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            UnlinkedInstructionKind::GetIndex { .. }
        ))
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
fn compiler_resolves_index_assignment_operands_from_hir() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let values = [2, 4, 8];
    let index = 1;
    values[index] += 5;
    return values[index];
}
"#,
        "main",
    )
    .expect("HIR-backed index assignment should compile");
    assert!(
        code.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            UnlinkedInstructionKind::SetIndex { .. }
        ))
    );
    assert!(
        code.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            UnlinkedInstructionKind::GetIndex { .. }
        ))
    );
}

#[test]
fn compiler_lowers_literal_string_map_index_writes() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let rewards = { "xp": 6 };
    rewards["xp"] += 4;
    rewards["gold"] = rewards["xp"] + 2;
    return rewards["xp"] + rewards["gold"];
}
"#,
        "main",
    )
    .expect("literal string map index writes should compile");
    assert!(code.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::GetStringKeyIndex { .. }
    )));
    assert!(code.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::SetStringKeyIndex { .. }
    )));
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
fn compiler_resolves_record_field_write_receiver_from_hir() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
struct Counter {
    value: i64,
}

fn main(counter: Counter) {
    counter.value = counter.value + 1;
    return counter.value;
}
"#,
        "main",
    )
    .expect("record field write should compile through HIR field facts");
    assert!(code.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::SetRecordSlot { .. }
    )));
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
fn compiler_resolves_indexed_record_field_write_from_hir_index() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let players = [
        Player { level: 2, exp: 5 },
        Player { level: 7, exp: 1 },
    ];
    let index = 1;
    players[index].level += 3;
    return players[index].level;
}
"#,
        "main",
    )
    .expect("HIR-backed indexed record field write should compile");
    assert!(
        code.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            UnlinkedInstructionKind::SetIndex { .. }
        ))
    );
    assert!(code.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::SetRecordSlot { .. }
    )));
    assert!(!code.instructions.iter().any(|instruction| {
        matches!(
            instruction.kind,
            UnlinkedInstructionKind::GetRecordField { .. }
                | UnlinkedInstructionKind::SetRecordField { .. }
        )
    }));
}

#[test]
fn compiler_resolves_indexed_record_read_shape_from_hir() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let players = [
        Player { level: 2, exp: 5 },
        Player { level: 7, exp: 1 },
    ];
    return players[0].level;
}
"#,
        "main",
    )
    .expect("indexed record read shape should compile through HIR index facts");
    assert!(code.instructions.iter().any(|instruction| {
        matches!(
            instruction.kind,
            UnlinkedInstructionKind::GetRecordSlot {
                ref field,
                ..
            } if field == "level"
        )
    }));
    assert!(!code.instructions.iter().any(|instruction| {
        matches!(
            instruction.kind,
            UnlinkedInstructionKind::GetRecordField { .. }
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
    Physical { amount: i64, element: String },
    Magical { amount: i64 },
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
    item_id: String,
    count: i64,
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
    item_id: String,
    count: i64,
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
