use super::*;
use crate::owned_value::OwnedValue;
use crate::value::Value as RuntimeValue;

fn run_host_method_program(
    program: &UnlinkedProgram,
    entry: &str,
    args: &[OwnedValue],
    host: &mut HostExecution<'_>,
) -> VmResult<OwnedValue> {
    let mut budget = ExecutionBudget::unbounded();
    run_linked_test_program_with_host_budget(&Vm::new(), program, entry, args, host, &mut budget)
}

fn run_host_method_program_runtime(
    program: &UnlinkedProgram,
    entry: &str,
    args: &[RuntimeValue],
    host: &mut HostExecution<'_>,
    heap: &mut HeapExecution<'_>,
    budget: &mut ExecutionBudget,
) -> VmResult<RuntimeValue> {
    run_linked_test_program_runtime_with_host_heap_and_budget(
        &Vm::new(),
        program,
        entry,
        args,
        host,
        heap,
        budget,
    )
}

#[test]
fn compiled_source_host_method_call_writes_through() {
    let host_ref = player_ref(3);
    let method = HostMethodId::new(5);
    let program = compile_host_program_source(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    player.grant_exp(20);
    return 1;
}
"#,
        host_definition_registry(
            &[("Player", host_ref.type_id)],
            &[],
            &[TestHostMethod::new(
                "Player",
                "grant_exp",
                method,
                &["amount"],
            )],
        ),
    )
    .expect("compile host method source");
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    adapter.insert_method_return(method, HostValue::Int(12));
    let mut tx = HostAccess::new();

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            access: &mut tx,
            script_globals: None,
        };
        run_host_method_program(
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host,
        )
    };

    assert_eq!(result, Ok(OwnedValue::Int(1)));
    assert_eq!(
        adapter.method_calls(),
        &[(HostPath::new(host_ref), method, vec![HostValue::Int(20)])]
    );
}

#[test]
fn compiled_source_host_field_method_call_uses_host_path() {
    let host_ref = player_ref(3);
    let inventory = FieldId::new(8);
    let method = HostMethodId::new(9);
    let program = compile_host_program_source(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    player.inventory.add("gold", 100);
    return 1;
}
"#,
        host_definition_registry(
            &[
                ("Player", host_ref.type_id),
                ("Inventory", HostTypeId::new(8)),
            ],
            &[TestHostField::new("Player", "inventory", inventory).type_hint("Inventory")],
            &[TestHostMethod::new(
                "Inventory",
                "add",
                method,
                &["item", "amount"],
            )],
        ),
    )
    .expect("compile host field method source");
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    adapter.insert_method_return(method, HostValue::Int(12));
    let mut tx = HostAccess::new();

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            access: &mut tx,
            script_globals: None,
        };
        run_host_method_program(
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host,
        )
    };

    assert_eq!(result, Ok(OwnedValue::Int(1)));
    assert_eq!(
        adapter.method_calls(),
        &[(
            HostPath::new(host_ref).field(inventory),
            method,
            vec![HostValue::String("gold".into()), HostValue::Int(100)]
        )]
    );
}

#[test]
fn compiled_source_host_indexed_method_call_uses_host_path() {
    let host_ref = player_ref(3);
    let inventory = FieldId::new(8);
    let items = FieldId::new(9);
    let method = HostMethodId::new(10);
    let item_path = HostPath::new(host_ref)
        .field(inventory)
        .field(items)
        .key("gold");
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
    player.inventory.items[item_id].grant(20);
    return 1;
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
            ],
            &[TestHostMethod::new("Item", "grant", method, &["amount"])],
        ),
    )
    .expect("compile indexed host method source");
    let mut adapter = MockStateAdapter::new();
    adapter.insert_diagnostic_path_value(item_path.clone(), HostValue::Int(0));
    adapter.insert_method_return(method, HostValue::Null);
    let mut tx = HostAccess::new();

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            access: &mut tx,
            script_globals: None,
        };
        run_host_method_program(
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host,
        )
    };

    assert_eq!(result, Ok(OwnedValue::Int(1)));
    assert_eq!(
        adapter.method_calls(),
        &[(item_path, method, vec![HostValue::Int(20)])]
    );
}

#[test]
fn call_host_method_writes_through_and_updates_adapter() {
    let host_ref = player_ref(3);
    let method = HostMethodId::new(8);
    let mut code = UnlinkedCodeObject::new("main", 3).with_params(vec!["player".into()]);
    let target = code.intern_host_target(HostTargetPlan::new(host_ref.type_id));
    let gold = code.push_constant(Constant::String("gold".into()));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(1),
            constant: gold,
        },
    ));
    let cache_site = code.push_cache_site(CacheSiteKind::HostPathCall, InstructionOffset(1));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::HostCall {
            dst: Some(Register(2)),
            root: Register(0),
            target,
            dynamic_args: Vec::new(),
            method,
            args: vec![Register(1)],
            cache_site,
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(2),
    }));
    let mut program = UnlinkedProgram::new();
    program.insert_function(code);
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    adapter.insert_method_return(method, HostValue::Int(12));
    let mut tx = HostAccess::new();

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            access: &mut tx,
            script_globals: None,
        };
        run_host_method_program(
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host,
        )
    };

    assert_eq!(result, Ok(OwnedValue::Int(12)));
    assert_eq!(
        adapter.method_calls(),
        &[(
            HostPath::new(host_ref),
            method,
            vec![HostValue::String("gold".into())]
        )]
    );
}

#[test]
fn heap_execution_converts_heap_string_for_host_method_call() {
    let host_ref = player_ref(3);
    let method = HostMethodId::new(8);
    let mut code = UnlinkedCodeObject::new("main", 3).with_params(vec!["player".into()]);
    let target = code.intern_host_target(HostTargetPlan::new(host_ref.type_id));
    let gold = code.push_constant(Constant::String("gold".into()));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(1),
            constant: gold,
        },
    ));
    let cache_site = code.push_cache_site(CacheSiteKind::HostPathCall, InstructionOffset(1));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::HostCall {
            dst: Some(Register(2)),
            root: Register(0),
            target,
            dynamic_args: Vec::new(),
            method,
            args: vec![Register(1)],
            cache_site,
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(2),
    }));
    let mut program = UnlinkedProgram::new();
    program.insert_function(code);
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    adapter.insert_method_return(method, HostValue::Null);
    let mut tx = HostAccess::new();
    let mut heap = ScriptHeap::new();
    let mut heap_execution = HeapExecution::new(&mut heap);
    let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX);

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            access: &mut tx,
            script_globals: None,
        };
        run_host_method_program_runtime(
            &program,
            "main",
            &[RuntimeValue::HostRef(host_ref)],
            &mut host,
            &mut heap_execution,
            &mut budget,
        )
    };

    assert_eq!(result, Ok(RuntimeValue::Null));
}

#[test]
fn compiled_source_host_method_call_returns_adapter_value() {
    let host_ref = player_ref(3);
    let method = HostMethodId::new(5);
    let program = compile_host_program_source(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    return player.grant_exp(20);
}
"#,
        host_definition_registry(
            &[("Player", host_ref.type_id)],
            &[],
            &[TestHostMethod::new(
                "Player",
                "grant_exp",
                method,
                &["amount"],
            )],
        ),
    )
    .expect("compile host method return source");
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    adapter.insert_method_return(method, HostValue::String("accepted".into()));
    let mut tx = HostAccess::new();

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            access: &mut tx,
            script_globals: None,
        };
        run_host_method_program(
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host,
        )
    };

    assert_eq!(result, Ok(OwnedValue::String("accepted".into())));
}
