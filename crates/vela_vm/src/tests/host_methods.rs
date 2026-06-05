use super::*;

#[test]
fn compiled_source_host_method_call_records_patch_tx() {
    let host_ref = player_ref(3);
    let method = HostMethodId::new(5);
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main(player) {
    player.grant_exp(20);
    return 1;
}
"#,
        &CompilerOptions::new().with_host_method("grant_exp", method),
    )
    .expect("compile host method source");
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    adapter.insert_method_return(method, HostValue::Int(12));
    let mut tx = PatchTx::new();

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };
        Vm::new().run_program_runtime_with_host(
            &program,
            "main",
            &[Value::HostRef(host_ref)],
            &mut host,
        )
    };

    assert_eq!(result, Ok(Value::Int(1)));
    assert!(adapter.method_calls().is_empty());
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(
        tx.patches()[0].op,
        PatchOp::CallHostMethod {
            method,
            args: vec![HostValue::Int(20)]
        }
    );
    tx.apply(&mut adapter).expect("apply host method patch");
    assert_eq!(
        adapter.method_calls(),
        &[(HostPath::new(host_ref), method, vec![HostValue::Int(20)])]
    );
}

#[test]
fn compiled_source_host_field_method_call_records_path_patch_tx() {
    let host_ref = player_ref(3);
    let inventory = FieldId::new(8);
    let method = HostMethodId::new(9);
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main(player) {
    player.inventory.add("gold", 100);
    return 1;
}
"#,
        &CompilerOptions::new()
            .with_host_field("inventory", inventory)
            .with_host_method("add", method),
    )
    .expect("compile host field method source");
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    adapter.insert_method_return(method, HostValue::Int(12));
    let mut tx = PatchTx::new();

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };
        Vm::new().run_program_runtime_with_host(
            &program,
            "main",
            &[Value::HostRef(host_ref)],
            &mut host,
        )
    };

    assert_eq!(result, Ok(Value::Int(1)));
    assert!(adapter.method_calls().is_empty());
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(
        tx.patches()[0].op,
        PatchOp::CallHostMethod {
            method,
            args: vec![HostValue::String("gold".into()), HostValue::Int(100)]
        }
    );
    tx.apply(&mut adapter).expect("apply host method patch");
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
fn compiled_source_host_indexed_method_call_records_path_patch_tx() {
    let host_ref = player_ref(3);
    let inventory = FieldId::new(8);
    let items = FieldId::new(9);
    let method = HostMethodId::new(10);
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
    player.inventory.items[item_id].grant(20);
    return 1;
}
"#,
        &CompilerOptions::new()
            .with_host_field("inventory", inventory)
            .with_host_field("items", items)
            .with_host_method("grant", method),
    )
    .expect("compile indexed host method source");
    let mut adapter = MockStateAdapter::new();
    adapter.insert_value(item_path.clone(), HostValue::Int(0));
    adapter.insert_method_return(method, HostValue::Null);
    let mut tx = PatchTx::new();

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };
        Vm::new().run_program_runtime_with_host(
            &program,
            "main",
            &[Value::HostRef(host_ref)],
            &mut host,
        )
    };

    assert_eq!(result, Ok(Value::Int(1)));
    assert!(adapter.method_calls().is_empty());
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(tx.patches()[0].path, item_path);
    assert_eq!(
        tx.patches()[0].op,
        PatchOp::CallHostMethod {
            method,
            args: vec![HostValue::Int(20)]
        }
    );
    tx.apply(&mut adapter)
        .expect("apply indexed host method patch");
    assert_eq!(
        adapter.method_calls(),
        &[(item_path, method, vec![HostValue::Int(20)])]
    );
}

#[test]
fn call_host_method_records_patch_and_applies_later() {
    let host_ref = player_ref(3);
    let method = HostMethodId::new(8);
    let mut code = CodeObject::new("main", 3).with_params(vec!["player".into()]);
    let gold = code.push_constant(Constant::String("gold".into()));
    code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(1),
        constant: gold,
    }));
    code.push_instruction(Instruction::new(InstructionKind::CallHostMethod {
        dst: Some(Register(2)),
        root: Register(0),
        segments: Vec::new(),
        method,
        args: vec![Register(1)],
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(2),
    }));
    let mut program = Program::new();
    program.insert_function(code);
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    adapter.insert_method_return(method, HostValue::Int(12));
    let mut tx = PatchTx::new();

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };
        Vm::new().run_program_runtime_with_host(
            &program,
            "main",
            &[Value::HostRef(host_ref)],
            &mut host,
        )
    };

    assert_eq!(result, Ok(Value::Int(12)));
    assert!(adapter.method_calls().is_empty());
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(
        tx.patches()[0].op,
        PatchOp::CallHostMethod {
            method,
            args: vec![HostValue::String("gold".into())]
        }
    );
    tx.apply(&mut adapter).expect("apply method call");
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
    let mut code = CodeObject::new("main", 3).with_params(vec!["player".into()]);
    let gold = code.push_constant(Constant::String("gold".into()));
    code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(1),
        constant: gold,
    }));
    code.push_instruction(Instruction::new(InstructionKind::CallHostMethod {
        dst: Some(Register(2)),
        root: Register(0),
        segments: Vec::new(),
        method,
        args: vec![Register(1)],
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(2),
    }));
    let mut program = Program::new();
    program.insert_function(code);
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    adapter.insert_method_return(method, HostValue::Null);
    let mut tx = PatchTx::new();
    let mut heap = ScriptHeap::new();
    let mut heap_execution = HeapExecution::new(&mut heap);
    let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX, usize::MAX);

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };
        Vm::new().run_program_runtime_with_host_heap_and_budget(
            &program,
            "main",
            &[Value::HostRef(host_ref)],
            &mut host,
            &mut heap_execution,
            &mut budget,
        )
    };

    assert_eq!(result, Ok(Value::Null));
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(
        tx.patches()[0].op,
        PatchOp::CallHostMethod {
            method,
            args: vec![HostValue::String("gold".into())]
        }
    );
}

#[test]
fn compiled_source_host_method_call_returns_copied_preview_value() {
    let host_ref = player_ref(3);
    let method = HostMethodId::new(5);
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main(player) {
    return player.grant_exp(20);
}
"#,
        &CompilerOptions::new().with_host_method("grant_exp", method),
    )
    .expect("compile host method return source");
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    adapter.insert_method_return(method, HostValue::String("accepted".into()));
    let mut tx = PatchTx::new();

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };
        Vm::new().run_program_runtime_with_host(
            &program,
            "main",
            &[Value::HostRef(host_ref)],
            &mut host,
        )
    };

    assert_eq!(result, Ok(Value::String("accepted".into())));
    assert!(adapter.method_calls().is_empty());
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(
        tx.patches()[0].op,
        PatchOp::CallHostMethod {
            method,
            args: vec![HostValue::Int(20)]
        }
    );
}
