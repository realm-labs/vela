use super::*;

#[test]
fn compiled_source_mutates_host_field_through_patch_tx() {
    let host_ref = player_ref(3);
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main(player) {
    player.level = 10;
    player.level += 1;
    return player.level;
}
"#,
        &CompilerOptions::new().with_host_field("level", level_field()),
    )
    .expect("compile host field source");
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    let mut tx = PatchTx::new();

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };
        Vm::new().run_program_with_host(
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host,
        )
    };

    assert_eq!(result, Ok(OwnedValue::Int(11)));
    assert_eq!(
        adapter.read_path(&level_path(host_ref)),
        Ok(HostValue::Int(11))
    );
    assert_eq!(tx.patches().len(), 2);
    assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Int(10)));
    assert_eq!(tx.patches()[1].op, PatchOp::Add(HostValue::Int(1)));
    assert_eq!(
        adapter.read_path(&level_path(host_ref)),
        Ok(HostValue::Int(11))
    );
}

#[test]
fn compiled_source_host_field_read_error_keeps_source_span() {
    let source = r#"
fn main(player) {
    return player.level;
}
"#;
    let host_ref = player_ref(3);
    let program = compile_program_source_with_options(
        SourceId::new(1),
        source,
        &CompilerOptions::new().with_host_field("level", level_field()),
    )
    .expect("compile host field source");
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    adapter.deny_read(level_path(host_ref));
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    let error = Vm::new()
        .run_program_with_host(
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host,
        )
        .expect_err("host read should fail");

    let span = error.source_span.expect("host read error source span");
    assert_eq!(span.source, SourceId::new(1));
    assert_eq!(
        &source[span.start as usize..span.end as usize],
        "player.level"
    );
}

#[test]
fn compiled_source_mutates_nested_host_field_through_patch_tx() {
    let host_ref = player_ref(3);
    let stats = FieldId::new(8);
    let level = FieldId::new(9);
    let stats_level = HostPath::new(host_ref).field(stats).field(level);
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main(player) {
    player.stats.level += 2;
    return player.stats.level;
}
"#,
        &CompilerOptions::new()
            .with_host_field("stats", stats)
            .with_host_field("level", level),
    )
    .expect("compile nested host field source");
    let mut adapter = MockStateAdapter::new();
    adapter.insert_value(stats_level.clone(), HostValue::Int(9));
    let mut tx = PatchTx::new();

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };
        Vm::new().run_program_with_host(
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host,
        )
    };

    assert_eq!(result, Ok(OwnedValue::Int(11)));
    assert_eq!(adapter.read_path(&stats_level), Ok(HostValue::Int(11)));
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(tx.patches()[0].path, stats_level);
    assert_eq!(tx.patches()[0].op, PatchOp::Add(HostValue::Int(2)));
    assert_eq!(adapter.read_path(&stats_level), Ok(HostValue::Int(11)));
}

#[test]
fn compiled_source_subtracts_nested_host_field_through_patch_tx() {
    let host_ref = player_ref(3);
    let stats = FieldId::new(8);
    let level = FieldId::new(9);
    let stats_level = HostPath::new(host_ref).field(stats).field(level);
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main(player) {
    player.stats.level -= 2;
    return player.stats.level;
}
"#,
        &CompilerOptions::new()
            .with_host_field("stats", stats)
            .with_host_field("level", level),
    )
    .expect("compile nested host subtraction source");
    let mut adapter = MockStateAdapter::new();
    adapter.insert_value(stats_level.clone(), HostValue::Int(9));
    let mut tx = PatchTx::new();

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };
        Vm::new().run_program_with_host(
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host,
        )
    };

    assert_eq!(result, Ok(OwnedValue::Int(7)));
    assert_eq!(adapter.read_path(&stats_level), Ok(HostValue::Int(7)));
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(tx.patches()[0].path, stats_level);
    assert_eq!(tx.patches()[0].op, PatchOp::Sub(HostValue::Int(2)));
    assert_eq!(adapter.read_path(&stats_level), Ok(HostValue::Int(7)));
}

#[test]
fn compiled_source_writes_host_numeric_compound_assignments_through_patch_tx() {
    let host_ref = player_ref(3);
    let stats = FieldId::new(8);
    let level = FieldId::new(9);
    let stats_level = HostPath::new(host_ref).field(stats).field(level);
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main(player) {
    player.stats.level *= 3;
    player.stats.level /= 2;
    player.stats.level %= 5;
    return player.stats.level;
}
"#,
        &CompilerOptions::new()
            .with_host_field("stats", stats)
            .with_host_field("level", level),
    )
    .expect("compile nested host numeric compound source");
    let mut adapter = MockStateAdapter::new();
    adapter.insert_value(stats_level.clone(), HostValue::Int(4));
    let mut tx = PatchTx::new();

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };
        Vm::new().run_program_with_host(
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host,
        )
    };

    assert_eq!(result, Ok(OwnedValue::Int(1)));
    assert_eq!(adapter.read_path(&stats_level), Ok(HostValue::Int(1)));
    assert_eq!(tx.patches().len(), 3);
    assert_eq!(tx.patches()[0].op, PatchOp::Mul(HostValue::Int(3)));
    assert_eq!(tx.patches()[1].op, PatchOp::Div(HostValue::Int(2)));
    assert_eq!(tx.patches()[2].op, PatchOp::Rem(HostValue::Int(5)));
    assert_eq!(adapter.read_path(&stats_level), Ok(HostValue::Int(1)));
}

#[test]
fn compiled_source_pushes_host_path_through_patch_tx() {
    let host_ref = player_ref(3);
    let inventory = FieldId::new(8);
    let rewards = FieldId::new(9);
    let reward_path = HostPath::new(host_ref).field(inventory).field(rewards);
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main(player) {
    player.inventory.rewards.push("gold");
    return player.inventory.rewards.len();
}
"#,
        &CompilerOptions::new()
            .with_host_field("inventory", inventory)
            .with_host_field("rewards", rewards),
    )
    .expect("compile host path push source");
    let mut adapter = MockStateAdapter::new();
    adapter.insert_value(
        reward_path.clone(),
        HostValue::Array(vec![HostValue::String("xp".into())]),
    );
    let mut tx = PatchTx::new();

    let error = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };
        Vm::new()
            .run_program_with_host(
                &program,
                "main",
                &[OwnedValue::HostRef(host_ref)],
                &mut host,
            )
            .expect_err("reading host array length should reject complex host conversion")
    };

    assert_eq!(
        error.kind,
        VmErrorKind::TypeMismatch {
            operation: "host complex value conversion"
        }
    );
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(tx.patches()[0].path, reward_path.clone());
    assert_eq!(
        tx.patches()[0].op,
        PatchOp::Push(HostValue::String("gold".into()))
    );
    assert_eq!(
        adapter.read_path(&reward_path),
        Ok(HostValue::Array(vec![
            HostValue::String("xp".into()),
            HostValue::String("gold".into())
        ]))
    );
}

#[test]
fn compiled_source_removes_host_path_through_patch_tx() {
    let host_ref = player_ref(3);
    let inventory = FieldId::new(8);
    let items = FieldId::new(9);
    let item_key = Symbol::new(NonZeroU32::new(1).expect("non-zero symbol"));
    let item_path = HostPath::new(host_ref)
        .field(inventory)
        .field(items)
        .key(item_key);
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main(player) {
    let item_id = "gold";
    player.inventory.items[item_id].remove();
    return 1;
}
"#,
        &CompilerOptions::new()
            .with_host_field("inventory", inventory)
            .with_host_field("items", items),
    )
    .expect("compile host path remove source");
    let mut adapter = MockStateAdapter::new();
    adapter.insert_value(item_path.clone(), HostValue::String("gold".into()));
    let mut tx = PatchTx::new();

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };
        Vm::new().run_program_with_host(
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host,
        )
    };

    assert_eq!(result, Ok(OwnedValue::Int(1)));
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(tx.patches()[0].path, item_path);
    assert_eq!(tx.patches()[0].op, PatchOp::Remove);
    assert!(matches!(
        adapter.read_path(&item_path),
        Err(error)
            if error.kind == (HostErrorKind::MissingPath {
                path: item_path.clone()
            })
    ));
}

#[test]
fn compiled_source_mutates_indexed_host_field_through_patch_tx() {
    let host_ref = player_ref(3);
    let inventory = FieldId::new(8);
    let items = FieldId::new(9);
    let count = FieldId::new(10);
    let item_key = Symbol::new(NonZeroU32::new(1).expect("non-zero symbol"));
    let item_count = HostPath::new(host_ref)
        .field(inventory)
        .field(items)
        .key(item_key)
        .field(count);
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main(player) {
    let item_id = "gold";
    player.inventory.items[item_id].count += 1;
    return player.inventory.items[item_id].count;
}
"#,
        &CompilerOptions::new()
            .with_host_field("inventory", inventory)
            .with_host_field("items", items)
            .with_host_field("count", count),
    )
    .expect("compile indexed host field source");
    let mut adapter = MockStateAdapter::new();
    adapter.insert_value(item_count.clone(), HostValue::Int(4));
    let mut tx = PatchTx::new();

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };
        Vm::new().run_program_with_host(
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host,
        )
    };

    assert_eq!(result, Ok(OwnedValue::Int(5)));
    assert_eq!(adapter.read_path(&item_count), Ok(HostValue::Int(5)));
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(tx.patches()[0].path, item_count);
    assert_eq!(tx.patches()[0].op, PatchOp::Add(HostValue::Int(1)));
    assert_eq!(adapter.read_path(&item_count), Ok(HostValue::Int(5)));
}

#[test]
fn bytecode_mutates_host_variant_field_through_patch_tx() {
    let host_ref = player_ref(3);
    let quest_progress = FieldId::new(8);
    let count = FieldId::new(9);
    let quest_count = HostPath::new(host_ref)
        .field(quest_progress)
        .variant_field(count);
    let mut code = CodeObject::new("main", 3).with_params(vec!["player".into()]);
    let one = code.push_constant(Constant::Int(1));
    let segments = vec![
        HostPathSegment::Field(quest_progress),
        HostPathSegment::VariantField(count),
    ];
    code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(1),
        constant: one,
    }));
    code.push_instruction(Instruction::new(InstructionKind::AddHostPath {
        root: Register(0),
        segments: segments.clone(),
        rhs: Register(1),
    }));
    code.push_instruction(Instruction::new(InstructionKind::GetHostPath {
        dst: Register(2),
        root: Register(0),
        segments,
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(2),
    }));
    let mut program = Program::new();
    program.insert_function(code);
    let mut adapter = MockStateAdapter::new();
    adapter.insert_value(quest_count.clone(), HostValue::Int(4));
    let mut tx = PatchTx::new();

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };
        Vm::new().run_program_with_host(
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host,
        )
    };

    assert_eq!(result, Ok(OwnedValue::Int(5)));
    assert_eq!(adapter.read_path(&quest_count), Ok(HostValue::Int(5)));
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(tx.patches()[0].path, quest_count);
    assert_eq!(tx.patches()[0].op, PatchOp::Add(HostValue::Int(1)));
    assert_eq!(adapter.read_path(&quest_count), Ok(HostValue::Int(5)));
}

#[test]
fn compiled_source_context_time_and_emit_records_patch_tx() {
    let ctx_ref = HostRef::new(HostTypeId::new(9), HostObjectId::new(11), 1);
    let now_field = FieldId::new(6);
    let tick_field = FieldId::new(7);
    let emit_method = HostMethodId::new(8);
    let log_method = HostMethodId::new(9);
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main(ctx) {
    let stamp = ctx.now + ctx.tick;
    ctx.emit("player.level_checked", stamp);
    ctx.log("info", "player.level_checked", stamp);
    return stamp;
}
"#,
        &CompilerOptions::new()
            .with_host_field("now", now_field)
            .with_host_field("tick", tick_field)
            .with_host_method("emit", emit_method)
            .with_host_method("log", log_method),
    )
    .expect("compile context source");
    let mut adapter = MockStateAdapter::new();
    adapter.insert_value(
        HostPath::new(ctx_ref).field(now_field),
        HostValue::Int(1000),
    );
    adapter.insert_value(HostPath::new(ctx_ref).field(tick_field), HostValue::Int(42));
    adapter.insert_method_return(emit_method, HostValue::Null);
    adapter.insert_method_return(log_method, HostValue::Null);
    let mut tx = PatchTx::new();
    let mut budget = ExecutionBudget::new(10_000, 1024 * 1024, 64, 1024);

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };
        Vm::new().run_program_with_host_managed_heap_and_budget(
            &program,
            "main",
            &[OwnedValue::HostRef(ctx_ref)],
            &mut host,
            &mut budget,
        )
    };

    assert_eq!(result, Ok(OwnedValue::Int(1042)));
    assert_eq!(tx.patches().len(), 2);
    assert_eq!(
        tx.patches()[0].op,
        PatchOp::CallHostMethod {
            method: emit_method,
            args: vec![
                HostValue::String("player.level_checked".into()),
                HostValue::Int(1042)
            ]
        }
    );
    assert_eq!(
        tx.patches()[1].op,
        PatchOp::CallHostMethod {
            method: log_method,
            args: vec![
                HostValue::String("info".into()),
                HostValue::String("player.level_checked".into()),
                HostValue::Int(1042)
            ]
        }
    );
    assert_eq!(
        adapter.method_calls(),
        &[
            (
                HostPath::new(ctx_ref),
                emit_method,
                vec![
                    HostValue::String("player.level_checked".into()),
                    HostValue::Int(1042)
                ]
            ),
            (
                HostPath::new(ctx_ref),
                log_method,
                vec![
                    HostValue::String("info".into()),
                    HostValue::String("player.level_checked".into()),
                    HostValue::Int(1042)
                ]
            )
        ]
    );
}

#[test]
fn host_field_write_conversion_error_records_no_patch() {
    let host_ref = player_ref(3);
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main(player) {
    let callback = || 1;
    player.level = callback;
    return 0;
}
"#,
        &CompilerOptions::new().with_host_field("level", level_field()),
    )
    .expect("compile host closure write source");
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    let mut tx = PatchTx::new();
    let mut budget = ExecutionBudget::new(10_000, 1024 * 1024, 64, 1024);

    let error = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };
        Vm::new()
            .run_program_with_host_managed_heap_and_budget(
                &program,
                "main",
                &[OwnedValue::HostRef(host_ref)],
                &mut host,
                &mut budget,
            )
            .expect_err("closure values cannot be written to host state")
    };

    assert_eq!(
        error.kind,
        VmErrorKind::TypeMismatch {
            operation: "set_host_field"
        }
    );
    assert!(tx.patches().is_empty());
    assert_eq!(
        adapter.read_path(&level_path(host_ref)),
        Ok(HostValue::Int(9))
    );
}
