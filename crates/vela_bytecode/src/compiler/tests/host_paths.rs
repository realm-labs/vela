use super::*;

#[test]
fn compiler_lowers_configured_host_method_calls() {
    let method = HostMethodId::new(5);
    let code = compile_function_source_with_options(
        SourceId::new(1),
        r#"
fn main(player) {
    player.grant_exp(20);
    return 1;
}
"#,
        "main",
        &CompilerOptions::new().with_host_method("grant_exp", method),
    )
    .expect("host method call should compile");
    assert!(code.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        InstructionKind::CallHostMethod {
            method: lowered_method,
            ..
        } if lowered_method == method
    )));
}

#[test]
fn compiler_lowers_local_host_method_when_root_matches_native_module() {
    let method = HostMethodId::new(5);
    let code = compile_function_source_with_options(
        SourceId::new(1),
        r#"
fn main(ctx) {
    ctx.emit("player.level_checked");
    return 1;
}
"#,
        "main",
        &CompilerOptions::new()
            .with_native_module_root("ctx")
            .with_host_method("emit", method),
    )
    .expect("local host method should shadow native module root");
    assert!(code.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        InstructionKind::CallHostMethod {
            method: lowered_method,
            ..
        } if lowered_method == method
    )));
}

#[test]
fn compiler_lowers_configured_host_method_calls_on_field_paths() {
    let inventory = FieldId::new(3);
    let method = HostMethodId::new(5);
    let code = compile_function_source_with_options(
        SourceId::new(1),
        r#"
fn main(player) {
    player.inventory.add("gold", 20);
    return 1;
}
"#,
        "main",
        &CompilerOptions::new()
            .with_host_field("inventory", inventory)
            .with_host_method("add", method),
    )
    .expect("host field method call should compile");
    assert!(code.instructions.iter().any(|instruction| matches!(
        &instruction.kind,
        InstructionKind::CallHostMethod {
            method: lowered_method,
            segments,
            ..
        } if *lowered_method == method
            && segments.as_slice() == [HostPathSegment::Field(inventory)]
    )));
}
#[test]
fn compiler_lowers_configured_host_method_calls_on_indexed_paths() {
    let inventory = FieldId::new(3);
    let items = FieldId::new(4);
    let method = HostMethodId::new(5);
    let code = compile_function_source_with_options(
        SourceId::new(1),
        r#"
fn main(player, item_id) {
    player.inventory.items[item_id].grant(20);
    return 1;
}
"#,
        "main",
        &CompilerOptions::new()
            .with_host_field("inventory", inventory)
            .with_host_field("items", items)
            .with_host_method("grant", method),
    )
    .expect("indexed host method call should compile");
    assert!(code.instructions.iter().any(|instruction| matches!(
        &instruction.kind,
        InstructionKind::CallHostMethod {
            method: lowered_method,
            segments,
            ..
        } if *lowered_method == method
            && matches!(
                segments.as_slice(),
                [
                    HostPathSegment::Field(first),
                    HostPathSegment::Field(second),
                    HostPathSegment::Value(_)
                ] if *first == inventory && *second == items
            )
    )));
}
#[test]
fn compiler_lowers_nested_host_field_paths() {
    let stats = FieldId::new(3);
    let level = FieldId::new(4);
    let code = compile_function_source_with_options(
        SourceId::new(1),
        r#"
fn main(player) {
    player.stats.level += 2;
    return player.stats.level;
}
"#,
        "main",
        &CompilerOptions::new()
            .with_host_field("stats", stats)
            .with_host_field("level", level),
    )
    .expect("nested host field path should compile");
    assert!(code.instructions.iter().any(|instruction| matches!(
        &instruction.kind,
        InstructionKind::AddHostPath {
            segments,
            ..
        } if segments.as_slice() == [
            HostPathSegment::Field(stats),
            HostPathSegment::Field(level)
        ]
    )));
    assert!(code.instructions.iter().any(|instruction| matches!(
        &instruction.kind,
        InstructionKind::GetHostPath {
            segments,
            ..
        } if segments.as_slice() == [
            HostPathSegment::Field(stats),
            HostPathSegment::Field(level)
        ]
    )));
}
#[test]
fn compiler_lowers_indexed_host_field_paths() {
    let inventory = FieldId::new(3);
    let items = FieldId::new(4);
    let count = FieldId::new(5);
    let code = compile_function_source_with_options(
        SourceId::new(1),
        r#"
fn main(player, item_id) {
    player.inventory.items[item_id].count += 1;
    return player.inventory.items[item_id].count;
}
"#,
        "main",
        &CompilerOptions::new()
            .with_host_field("inventory", inventory)
            .with_host_field("items", items)
            .with_host_field("count", count),
    )
    .expect("indexed host field path should compile");
    assert!(code.instructions.iter().any(|instruction| matches!(
        &instruction.kind,
        InstructionKind::AddHostPath {
            segments,
            ..
        } if matches!(
            segments.as_slice(),
            [
                HostPathSegment::Field(first),
                HostPathSegment::Field(second),
                HostPathSegment::Value(_),
                HostPathSegment::Field(third)
            ] if *first == inventory && *second == items && *third == count
        )
    )));
    assert!(code.instructions.iter().any(|instruction| matches!(
        &instruction.kind,
        InstructionKind::GetHostPath {
            segments,
            ..
        } if matches!(
            segments.as_slice(),
            [
                HostPathSegment::Field(first),
                HostPathSegment::Field(second),
                HostPathSegment::Value(_),
                HostPathSegment::Field(third)
            ] if *first == inventory && *second == items && *third == count
        )
    )));
}
#[test]
fn compiler_lowers_host_variant_field_paths() {
    let quest_progress = FieldId::new(3);
    let count = FieldId::new(4);
    let code = compile_function_source_with_options(
        SourceId::new(1),
        r#"
fn main(player) {
    player.quest_progress.count += 1;
    return player.quest_progress.count;
}
"#,
        "main",
        &CompilerOptions::new()
            .with_host_field("quest_progress", quest_progress)
            .with_host_variant_field("count", count),
    )
    .expect("host variant field path should compile");
    assert!(code.instructions.iter().any(|instruction| matches!(
        &instruction.kind,
        InstructionKind::AddHostPath {
            segments,
            ..
        } if segments.as_slice() == [
            HostPathSegment::Field(quest_progress),
            HostPathSegment::VariantField(count)
        ]
    )));
    assert!(code.instructions.iter().any(|instruction| matches!(
        &instruction.kind,
        InstructionKind::GetHostPath {
            segments,
            ..
        } if segments.as_slice() == [
            HostPathSegment::Field(quest_progress),
            HostPathSegment::VariantField(count)
        ]
    )));
}
#[test]
fn compiler_lowers_host_sub_assignments() {
    let stats = FieldId::new(3);
    let level = FieldId::new(4);
    let code = compile_function_source_with_options(
        SourceId::new(1),
        r#"
fn main(player) {
    player.stats.level -= 2;
    return player.stats.level;
}
"#,
        "main",
        &CompilerOptions::new()
            .with_host_field("stats", stats)
            .with_host_field("level", level),
    )
    .expect("host sub assignment should compile");
    assert!(code.instructions.iter().any(|instruction| matches!(
        &instruction.kind,
        InstructionKind::SubHostPath {
            segments,
            ..
        } if segments.as_slice() == [
            HostPathSegment::Field(stats),
            HostPathSegment::Field(level)
        ]
    )));
}
#[test]
fn compiler_lowers_host_numeric_compound_assignments() {
    let stats = FieldId::new(3);
    let level = FieldId::new(4);
    let code = compile_function_source_with_options(
        SourceId::new(1),
        r#"
fn main(player) {
    player.stats.level *= 3;
    player.stats.level /= 2;
    player.stats.level %= 5;
    return player.stats.level;
}
"#,
        "main",
        &CompilerOptions::new()
            .with_host_field("stats", stats)
            .with_host_field("level", level),
    )
    .expect("host numeric compound assignments should compile");
    assert!(code.instructions.iter().any(|instruction| matches!(
        &instruction.kind,
        InstructionKind::MulHostPath { segments, .. }
            if segments.as_slice() == [
                HostPathSegment::Field(stats),
                HostPathSegment::Field(level)
            ]
    )));
    assert!(code.instructions.iter().any(|instruction| matches!(
        &instruction.kind,
        InstructionKind::DivHostPath { segments, .. }
            if segments.as_slice() == [
                HostPathSegment::Field(stats),
                HostPathSegment::Field(level)
            ]
    )));
    assert!(code.instructions.iter().any(|instruction| matches!(
        &instruction.kind,
        InstructionKind::RemHostPath { segments, .. }
            if segments.as_slice() == [
                HostPathSegment::Field(stats),
                HostPathSegment::Field(level)
            ]
    )));
}
#[test]
fn compiler_lowers_host_path_push_calls() {
    let inventory = FieldId::new(3);
    let rewards = FieldId::new(4);
    let code = compile_function_source_with_options(
        SourceId::new(1),
        r#"
fn main(player) {
    player.inventory.rewards.push("gold");
    return 1;
}
"#,
        "main",
        &CompilerOptions::new()
            .with_host_field("inventory", inventory)
            .with_host_field("rewards", rewards),
    )
    .expect("host path push should compile");
    assert!(code.instructions.iter().any(|instruction| matches!(
        &instruction.kind,
        InstructionKind::PushHostPath {
            segments,
            ..
        } if segments.as_slice() == [
            HostPathSegment::Field(inventory),
            HostPathSegment::Field(rewards)
        ]
    )));
}
#[test]
fn compiler_lowers_host_path_remove_calls() {
    let inventory = FieldId::new(3);
    let items = FieldId::new(4);
    let code = compile_function_source_with_options(
        SourceId::new(1),
        r#"
fn main(player) {
    let item_id = "gold";
    player.inventory.items[item_id].remove();
    return 1;
}
"#,
        "main",
        &CompilerOptions::new()
            .with_host_field("inventory", inventory)
            .with_host_field("items", items),
    )
    .expect("host path remove should compile");
    assert!(
        code.instructions
            .iter()
            .any(|instruction| match &instruction.kind {
                InstructionKind::RemoveHostPath { segments, .. } => matches!(
                    segments.as_slice(),
                    [
                        HostPathSegment::Field(first),
                        HostPathSegment::Field(second),
                        HostPathSegment::Value(_)
                    ] if *first == inventory && *second == items
                ),
                _ => false,
            })
    );
}
