use super::*;
use vela_host::resolved::HostMutationOp;

fn exec_compiled_host_field(
    program: &UnlinkedProgram,
    entry: &str,
    args: &[OwnedValue],
    host: &mut HostExecution<'_>,
) -> VmResult<OwnedValue> {
    let mut budget = ExecutionBudget::unbounded();
    exec_compiled_host_field_with_budget(program, entry, args, host, &mut budget)
}

fn exec_compiled_host_field_with_budget(
    program: &UnlinkedProgram,
    entry: &str,
    args: &[OwnedValue],
    host: &mut HostExecution<'_>,
    budget: &mut ExecutionBudget,
) -> VmResult<OwnedValue> {
    run_linked_test_program_with_host_budget(&Vm::new(), program, entry, args, host, budget)
}

#[test]
fn compiled_source_mutates_host_field_through_host_access() {
    let host_ref = player_ref(3);
    let program = compile_host_program_source(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    player.level = 10;
    player.level += 1;
    return player.level;
}
"#,
        host_definition_registry(
            &[("Player", host_ref.type_id)],
            &[TestHostField::new("Player", "level", level_field())],
            &[],
        ),
    )
    .expect("compile host field source");
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    let mut tx = HostAccess::new();

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            access: &mut tx,
            script_globals: None,
        };
        exec_compiled_host_field(
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host,
        )
    };

    assert_eq!(result, Ok(OwnedValue::Int(11)));
    assert_eq!(
        adapter.read_diagnostic_path(&level_path(host_ref)),
        Ok(HostValue::Int(11))
    );
    assert_eq!(
        adapter.read_diagnostic_path(&level_path(host_ref)),
        Ok(HostValue::Int(11))
    );
}

#[test]
fn compiled_source_host_field_read_error_keeps_source_span() {
    let source = r#"
fn main(player: Player) {
    return player.level;
}
"#;
    let host_ref = player_ref(3);
    let program = compile_host_program_source(
        SourceId::new(1),
        source,
        host_definition_registry(
            &[("Player", host_ref.type_id)],
            &[TestHostField::new("Player", "level", level_field()).readonly()],
            &[],
        ),
    )
    .expect("compile host field source");
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    adapter.deny_diagnostic_path_read(level_path(host_ref));
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    let error = exec_compiled_host_field(
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
fn compiled_source_mutates_nested_host_field_through_host_access() {
    let host_ref = player_ref(3);
    let stats = FieldId::new(8);
    let level = FieldId::new(9);
    let stats_level = HostPath::new(host_ref).field(stats).field(level);
    let program = compile_host_program_source(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    player.stats.level += 2;
    return player.stats.level;
}
"#,
        host_definition_registry(
            &[("Player", host_ref.type_id), ("Stats", HostTypeId::new(8))],
            &[
                TestHostField::new("Player", "stats", stats).type_hint("Stats"),
                TestHostField::new("Stats", "level", level),
            ],
            &[],
        ),
    )
    .expect("compile nested host field source");
    let mut adapter = MockStateAdapter::new();
    adapter.insert_diagnostic_path_value(stats_level.clone(), HostValue::Int(9));
    let mut tx = HostAccess::new();

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            access: &mut tx,
            script_globals: None,
        };
        exec_compiled_host_field(
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host,
        )
    };

    assert_eq!(result, Ok(OwnedValue::Int(11)));
    assert_eq!(
        adapter.read_diagnostic_path(&stats_level),
        Ok(HostValue::Int(11))
    );
    assert_eq!(
        adapter.read_diagnostic_path(&stats_level),
        Ok(HostValue::Int(11))
    );
}

#[test]
fn compiled_source_subtracts_nested_host_field_through_host_access() {
    let host_ref = player_ref(3);
    let stats = FieldId::new(8);
    let level = FieldId::new(9);
    let stats_level = HostPath::new(host_ref).field(stats).field(level);
    let program = compile_host_program_source(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    player.stats.level -= 2;
    return player.stats.level;
}
"#,
        host_definition_registry(
            &[("Player", host_ref.type_id), ("Stats", HostTypeId::new(8))],
            &[
                TestHostField::new("Player", "stats", stats).type_hint("Stats"),
                TestHostField::new("Stats", "level", level),
            ],
            &[],
        ),
    )
    .expect("compile nested host subtraction source");
    let mut adapter = MockStateAdapter::new();
    adapter.insert_diagnostic_path_value(stats_level.clone(), HostValue::Int(9));
    let mut tx = HostAccess::new();

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            access: &mut tx,
            script_globals: None,
        };
        exec_compiled_host_field(
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host,
        )
    };

    assert_eq!(result, Ok(OwnedValue::Int(7)));
    assert_eq!(
        adapter.read_diagnostic_path(&stats_level),
        Ok(HostValue::Int(7))
    );
    assert_eq!(
        adapter.read_diagnostic_path(&stats_level),
        Ok(HostValue::Int(7))
    );
}

#[test]
fn compiled_source_writes_host_numeric_compound_assignments_through_host_access() {
    let host_ref = player_ref(3);
    let stats = FieldId::new(8);
    let level = FieldId::new(9);
    let stats_level = HostPath::new(host_ref).field(stats).field(level);
    let program = compile_host_program_source(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    player.stats.level *= 3;
    player.stats.level /= 2;
    player.stats.level %= 5;
    return player.stats.level;
}
"#,
        host_definition_registry(
            &[("Player", host_ref.type_id), ("Stats", HostTypeId::new(8))],
            &[
                TestHostField::new("Player", "stats", stats).type_hint("Stats"),
                TestHostField::new("Stats", "level", level),
            ],
            &[],
        ),
    )
    .expect("compile nested host numeric compound source");
    let mut adapter = MockStateAdapter::new();
    adapter.insert_diagnostic_path_value(stats_level.clone(), HostValue::Int(4));
    let mut tx = HostAccess::new();

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            access: &mut tx,
            script_globals: None,
        };
        exec_compiled_host_field(
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host,
        )
    };

    assert_eq!(result, Ok(OwnedValue::Int(1)));
    assert_eq!(
        adapter.read_diagnostic_path(&stats_level),
        Ok(HostValue::Int(1))
    );
    assert_eq!(
        adapter.read_diagnostic_path(&stats_level),
        Ok(HostValue::Int(1))
    );
}

#[test]
fn compiled_source_rejects_host_path_push_without_collection_adapter() {
    let host_ref = player_ref(3);
    let inventory = FieldId::new(8);
    let rewards = FieldId::new(9);
    let reward_path = HostPath::new(host_ref).field(inventory).field(rewards);
    let program = compile_host_program_source(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    player.inventory.rewards.push("gold");
    return 1;
}
"#,
        host_definition_registry(
            &[
                ("Player", host_ref.type_id),
                ("Inventory", HostTypeId::new(8)),
            ],
            &[
                TestHostField::new("Player", "inventory", inventory).type_hint("Inventory"),
                TestHostField::new("Inventory", "rewards", rewards),
            ],
            &[],
        ),
    )
    .expect("compile host path push source");
    let mut adapter = MockStateAdapter::new();
    adapter.insert_diagnostic_path_value(reward_path.clone(), HostValue::Int(0));
    let mut tx = HostAccess::new();

    let error = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            access: &mut tx,
            script_globals: None,
        };
        exec_compiled_host_field(
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host,
        )
        .expect_err("host push should reject scalar-only host values")
    };

    assert_eq!(
        error.kind(),
        VmErrorKind::Host(HostErrorKind::InvalidPush {
            path: reward_path.clone()
        })
    );
    assert_eq!(
        adapter.read_diagnostic_path(&reward_path),
        Ok(HostValue::Int(0))
    );
}

#[test]
fn compiled_source_removes_host_path_through_host_access() {
    let host_ref = player_ref(3);
    let inventory = FieldId::new(8);
    let items = FieldId::new(9);
    let item_path = HostPath::new(host_ref)
        .field(inventory)
        .field(items)
        .key("gold");
    let program = compile_host_program_source(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    let item_id = "gold";
    player.inventory.items[item_id].remove();
    return 1;
}
"#,
        host_definition_registry(
            &[
                ("Player", host_ref.type_id),
                ("Inventory", HostTypeId::new(8)),
            ],
            &[
                TestHostField::new("Player", "inventory", inventory).type_hint("Inventory"),
                TestHostField::new("Inventory", "items", items),
            ],
            &[],
        ),
    )
    .expect("compile host path remove source");
    let mut adapter = MockStateAdapter::new();
    adapter.insert_diagnostic_path_value(item_path.clone(), HostValue::String("gold".into()));
    let mut tx = HostAccess::new();

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            access: &mut tx,
            script_globals: None,
        };
        exec_compiled_host_field(
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host,
        )
    };

    assert_eq!(result, Ok(OwnedValue::Int(1)));
    assert!(matches!(
        adapter.read_diagnostic_path(&item_path),
        Err(error)
            if error.kind == (HostErrorKind::MissingPath {
                path: item_path.clone()
            })
    ));
}

#[test]
fn compiled_source_mutates_indexed_host_field_through_host_access() {
    let host_ref = player_ref(3);
    let inventory = FieldId::new(8);
    let items = FieldId::new(9);
    let count = FieldId::new(10);
    let item_count = HostPath::new(host_ref)
        .field(inventory)
        .field(items)
        .key("gold")
        .field(count);
    let options = CompilerOptions::new().with_host_index_capability(
        "Items",
        HostIndexCapabilityInfo {
            value_type: Some("Item".into()),
            ..HostIndexCapabilityInfo::default()
        },
    );
    let program = compile_host_program_source_with_options(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    let item_id = "gold";
    player.inventory.items[item_id].count += 1;
    return player.inventory.items[item_id].count;
}
"#,
        &options,
        host_definition_registry(
            &[
                ("Player", host_ref.type_id),
                ("Inventory", HostTypeId::new(8)),
                ("Items", HostTypeId::new(9)),
                ("Item", HostTypeId::new(10)),
            ],
            &[
                TestHostField::new("Player", "inventory", inventory).type_hint("Inventory"),
                TestHostField::new("Inventory", "items", items).type_hint("Items"),
                TestHostField::new("Item", "count", count),
            ],
            &[],
        ),
    )
    .expect("compile indexed host field source");
    let mut adapter = MockStateAdapter::new();
    adapter.insert_diagnostic_path_value(item_count.clone(), HostValue::Int(4));
    let mut tx = HostAccess::new();

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            access: &mut tx,
            script_globals: None,
        };
        exec_compiled_host_field(
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host,
        )
    };

    assert_eq!(result, Ok(OwnedValue::Int(5)));
    assert_eq!(
        adapter.read_diagnostic_path(&item_count),
        Ok(HostValue::Int(5))
    );
    assert_eq!(
        adapter.read_diagnostic_path(&item_count),
        Ok(HostValue::Int(5))
    );
}

#[test]
fn bytecode_mutates_host_variant_field_through_host_access() {
    let host_ref = player_ref(3);
    let quest_progress = FieldId::new(8);
    let count = FieldId::new(9);
    let quest_count = HostPath::new(host_ref)
        .field(quest_progress)
        .variant_field(count);
    let mut code = UnlinkedCodeObject::new("main", 3).with_params(vec!["player".into()]);
    let one = code.push_constant(Constant::Int(1));
    let target = code.intern_host_target(
        HostTargetPlan::new(host_ref.type_id)
            .field(quest_progress)
            .variant_field(count),
    );
    let mutate_cache = code.push_cache_site(CacheSiteKind::HostPathMutate, InstructionOffset(1));
    let read_cache = code.push_cache_site(CacheSiteKind::HostPathRead, InstructionOffset(2));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(1),
            constant: one,
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::HostMutate {
            root: Register(0),
            target,
            dynamic_args: Vec::new(),
            op: HostMutationOp::Add,
            rhs: Register(1),
            cache_site: mutate_cache,
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::HostRead {
            dst: Register(2),
            root: Register(0),
            target,
            dynamic_args: Vec::new(),
            cache_site: read_cache,
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(2),
    }));
    let mut program = UnlinkedProgram::new();
    program.insert_function(code);
    let mut adapter = MockStateAdapter::new();
    adapter.insert_diagnostic_path_value(quest_count.clone(), HostValue::Int(4));
    let mut tx = HostAccess::new();

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            access: &mut tx,
            script_globals: None,
        };
        exec_compiled_host_field(
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host,
        )
    };

    assert_eq!(result, Ok(OwnedValue::Int(5)));
    assert_eq!(
        adapter.read_diagnostic_path(&quest_count),
        Ok(HostValue::Int(5))
    );
    assert_eq!(
        adapter.read_diagnostic_path(&quest_count),
        Ok(HostValue::Int(5))
    );
}

#[test]
fn compiled_source_context_time_and_emit_writes_through() {
    let ctx_ref = HostRef::new(HostTypeId::new(9), HostObjectId::new(11), 1);
    let now_field = FieldId::new(6);
    let tick_field = FieldId::new(7);
    let emit_method = HostMethodId::new(8);
    let log_method = HostMethodId::new(9);
    let program = compile_host_program_source(
        SourceId::new(1),
        r#"
fn main(ctx: Ctx) {
    let stamp = ctx.now + ctx.tick;
    ctx.emit("player.level_checked", stamp);
    ctx.log("info", "player.level_checked", stamp);
    return stamp;
}
"#,
        host_definition_registry(
            &[("Ctx", ctx_ref.type_id)],
            &[
                TestHostField::new("Ctx", "now", now_field),
                TestHostField::new("Ctx", "tick", tick_field),
            ],
            &[
                TestHostMethod::new("Ctx", "emit", emit_method, &["event", "value"]),
                TestHostMethod::new("Ctx", "log", log_method, &["level", "event", "value"]),
            ],
        ),
    )
    .expect("compile context source");
    let mut adapter = MockStateAdapter::new();
    adapter.insert_diagnostic_path_value(
        HostPath::new(ctx_ref).field(now_field),
        HostValue::Int(1000),
    );
    adapter
        .insert_diagnostic_path_value(HostPath::new(ctx_ref).field(tick_field), HostValue::Int(42));
    adapter.insert_method_return(emit_method, HostValue::Null);
    adapter.insert_method_return(log_method, HostValue::Null);
    let mut tx = HostAccess::new();
    let mut budget = ExecutionBudget::new(10_000, 1024 * 1024, 64);

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            access: &mut tx,
            script_globals: None,
        };
        exec_compiled_host_field_with_budget(
            &program,
            "main",
            &[OwnedValue::HostRef(ctx_ref)],
            &mut host,
            &mut budget,
        )
    };

    assert_eq!(result, Ok(OwnedValue::Int(1042)));
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
fn host_field_write_conversion_error_leaves_no_write() {
    let host_ref = player_ref(3);
    let program = compile_host_program_source(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    let callback = || 1;
    player.level = callback;
    return 0;
}
"#,
        host_definition_registry(
            &[("Player", host_ref.type_id)],
            &[TestHostField::new("Player", "level", level_field())],
            &[],
        ),
    )
    .expect("compile host closure write source");
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    let mut tx = HostAccess::new();
    let mut budget = ExecutionBudget::new(10_000, 1024 * 1024, 64);

    let error = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            access: &mut tx,
            script_globals: None,
        };
        exec_compiled_host_field_with_budget(
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host,
            &mut budget,
        )
        .expect_err("closure values cannot be written to host state")
    };

    assert_eq!(
        error.kind(),
        VmErrorKind::TypeMismatch {
            operation: "set_host_field"
        }
    );
    assert_eq!(
        adapter.read_diagnostic_path(&level_path(host_ref)),
        Ok(HostValue::Int(9))
    );
}
