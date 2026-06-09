use super::*;
use crate::value::Value as RuntimeValue;
use vela_bytecode::CacheSiteKind;
use vela_host::resolved::HostMutationOp;
use vela_host::target::HostTargetPlan;

#[test]
fn heap_execution_enforces_memory_budget_for_bytecode_allocations() {
    let code = compile_function_source(
        SourceId::new(1),
        "fn main() { return \"this string is too large\"; }",
        "main",
    )
    .expect("compile string source");
    let mut heap = ScriptHeap::new();
    let mut heap_execution = HeapExecution::new(&mut heap);
    let mut budget = ExecutionBudget::new(u64::MAX, 8, usize::MAX);

    let error = Vm::new()
        .run_with_heap_and_budget(&code, &mut heap_execution, &mut budget)
        .expect_err("string allocation should exceed memory budget");

    assert_eq!(
        error.kind(),
        VmErrorKind::BudgetExceeded {
            budget: ExecutionBudgetKind::MemoryBytes,
            limit: 8,
        }
    );
    assert_eq!(heap.live_object_count(), 0);
    assert_eq!(budget.memory_bytes_allocated(), 0);
}

#[test]
fn runs_compiled_if_then_branch_source() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    if 2 < 3 {
        return 10;
    } else {
        return 20;
    }
}
"#,
        "main",
    )
    .expect("compile if source");

    assert_eq!(Vm::new().run(&code), Ok(OwnedValue::Int(10)));
}

#[test]
fn runs_compiled_if_else_branch_source() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    if 3 < 2 {
        return 10;
    } else {
        return 20;
    }
}
"#,
        "main",
    )
    .expect("compile if source");

    assert_eq!(Vm::new().run(&code), Ok(OwnedValue::Int(20)));
}

#[test]
fn runs_compiled_comparison_and_remainder_source() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    if 10 % 4 == 2 {
        if 3 >= 3 {
            if 2 <= 5 {
                if 5 != 6 {
                    return 1;
                }
            }
        }
    }
    return 0;
}
"#,
        "main",
    )
    .expect("compile operator source");

    assert_eq!(Vm::new().run(&code), Ok(OwnedValue::Int(1)));
}

#[test]
fn reads_host_field_through_host_access() {
    let (program, host_ref) = host_read_program();
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    let result = Vm::new().run_program_with_host(
        &program,
        "main",
        &[OwnedValue::HostRef(host_ref)],
        &mut host,
    );

    assert_eq!(result, Ok(OwnedValue::Int(9)));
}

#[test]
fn set_host_field_writes_through_and_updates_adapter() {
    let host_ref = player_ref(3);
    let mut code = CodeObject::new("main", 3).with_params(vec!["player".into()]);
    let ten = code.push_constant(Constant::Int(10));
    code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(1),
        constant: ten,
    }));
    code.push_instruction(Instruction::new(InstructionKind::SetHostField {
        root: Register(0),
        field: level_field(),
        src: Register(1),
    }));
    code.push_instruction(Instruction::new(InstructionKind::GetHostField {
        dst: Register(2),
        root: Register(0),
        field: level_field(),
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(2),
    }));
    let mut program = Program::new();
    program.insert_function(code);
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    let mut tx = HostAccess::new();

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            access: &mut tx,
            script_globals: None,
        };
        Vm::new().run_program_with_host(
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host,
        )
    };

    assert_eq!(result, Ok(OwnedValue::Int(10)));
    assert_eq!(
        adapter.read_diagnostic_path(&level_path(host_ref)),
        Ok(HostValue::Int(10))
    );
    assert_eq!(
        adapter.read_diagnostic_path(&level_path(host_ref)),
        Ok(HostValue::Int(10))
    );
}

#[test]
fn collapsed_host_mutate_and_read_execute_through_target_plan() {
    let host_ref = player_ref(3);
    let mut code = CodeObject::new("main", 3).with_params(vec!["player".into()]);
    let one = code.push_constant(Constant::Int(1));
    let target =
        code.intern_host_target(HostTargetPlan::new(host_ref.type_id).field(level_field()));
    let mutate_cache = code.push_cache_site(CacheSiteKind::HostPathMutate, InstructionOffset(1));
    let read_cache = code.push_cache_site(CacheSiteKind::HostPathRead, InstructionOffset(2));

    code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(1),
        constant: one,
    }));
    code.push_instruction(Instruction::new(InstructionKind::HostMutate {
        root: Register(0),
        target,
        dynamic_args: Vec::new(),
        op: HostMutationOp::Add,
        rhs: Register(1),
        cache_site: mutate_cache,
    }));
    code.push_instruction(Instruction::new(InstructionKind::HostRead {
        dst: Register(2),
        root: Register(0),
        target,
        dynamic_args: Vec::new(),
        cache_site: read_cache,
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(2),
    }));

    let mut program = Program::new();
    program.insert_function(code);
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    let mut tx = HostAccess::new();

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            access: &mut tx,
            script_globals: None,
        };
        Vm::new().run_program_with_host(
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host,
        )
    };

    assert_eq!(result, Ok(OwnedValue::Int(10)));
    assert_eq!(
        adapter.read_diagnostic_path(&level_path(host_ref)),
        Ok(HostValue::Int(10))
    );
}

#[test]
fn heap_execution_converts_heap_string_for_host_field_write() {
    let host_ref = player_ref(3);
    let mut code = CodeObject::new("main", 2).with_params(vec!["player".into()]);
    let gold = code.push_constant(Constant::String("gold".into()));
    code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(1),
        constant: gold,
    }));
    code.push_instruction(Instruction::new(InstructionKind::SetHostField {
        root: Register(0),
        field: level_field(),
        src: Register(1),
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(1),
    }));
    let mut program = Program::new();
    program.insert_function(code);
    let mut adapter = host_adapter(host_ref, HostValue::String("old".into()));
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
        Vm::new().run_program_runtime_with_host_heap_and_budget(
            &program,
            "main",
            &[RuntimeValue::HostRef(host_ref)],
            &mut host,
            &mut heap_execution,
            &mut budget,
        )
    };

    assert!(matches!(result, Ok(RuntimeValue::HeapRef(_))));
}

#[test]
fn repeated_host_writes_write_through_without_mutation_budget() {
    let host_ref = player_ref(3);
    let mut code = CodeObject::new("main", 3).with_params(vec!["player".into()]);
    let ten = code.push_constant(Constant::Int(10));
    let eleven = code.push_constant(Constant::Int(11));
    code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(1),
        constant: ten,
    }));
    code.push_instruction(Instruction::new(InstructionKind::SetHostField {
        root: Register(0),
        field: level_field(),
        src: Register(1),
    }));
    code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(2),
        constant: eleven,
    }));
    code.push_instruction(Instruction::new(InstructionKind::SetHostField {
        root: Register(0),
        field: level_field(),
        src: Register(2),
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(2),
    }));
    let mut program = Program::new();
    program.insert_function(code);
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    let mut tx = HostAccess::new();
    let mut budget = ExecutionBudget::new(100, usize::MAX, usize::MAX);

    let value = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            access: &mut tx,
            script_globals: None,
        };
        Vm::new()
            .run_program_with_host_and_budget(
                &program,
                "main",
                &[OwnedValue::HostRef(host_ref)],
                &mut host,
                &mut budget,
            )
            .expect("host writes should not have a host-write count budget")
    };

    assert_eq!(value, OwnedValue::Int(11));
    assert_eq!(
        adapter.read_diagnostic_path(&level_path(host_ref)),
        Ok(HostValue::Int(11))
    );
}

#[test]
fn add_host_field_writes_through_and_updates_adapter() {
    let host_ref = player_ref(3);
    let mut code = CodeObject::new("main", 3).with_params(vec!["player".into()]);
    let one = code.push_constant(Constant::Int(1));
    code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(1),
        constant: one,
    }));
    code.push_instruction(Instruction::new(InstructionKind::AddHostField {
        root: Register(0),
        field: level_field(),
        rhs: Register(1),
    }));
    code.push_instruction(Instruction::new(InstructionKind::GetHostField {
        dst: Register(2),
        root: Register(0),
        field: level_field(),
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(2),
    }));
    let mut program = Program::new();
    program.insert_function(code);
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    let mut tx = HostAccess::new();

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            access: &mut tx,
            script_globals: None,
        };
        Vm::new().run_program_with_host(
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host,
        )
    };

    assert_eq!(result, Ok(OwnedValue::Int(10)));
    assert_eq!(
        adapter.read_diagnostic_path(&level_path(host_ref)),
        Ok(HostValue::Int(10))
    );
}

#[test]
fn host_field_read_rejects_stale_generation() {
    let (program, _host_ref) = host_read_program();
    let fresh_ref = player_ref(3);
    let stale_ref = player_ref(2);
    let mut adapter = host_adapter(fresh_ref, HostValue::Int(9));
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    let error = Vm::new()
        .run_program_with_host(
            &program,
            "main",
            &[OwnedValue::HostRef(stale_ref)],
            &mut host,
        )
        .expect_err("stale host read");

    assert_eq!(
        error.kind(),
        VmErrorKind::Host(vela_host::error::HostErrorKind::StaleGeneration {
            expected: 2,
            actual: 3
        })
    );
}

#[test]
fn host_field_read_error_keeps_instruction_source_span() {
    let host_ref = player_ref(3);
    let span = Span::new(SourceId::new(7), 20, 32);
    let mut code = CodeObject::new("main", 2).with_params(vec!["player".into()]);
    code.push_instruction(
        Instruction::new(InstructionKind::GetHostField {
            dst: Register(1),
            root: Register(0),
            field: level_field(),
        })
        .with_span(span),
    );
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(1),
    }));
    let mut program = Program::new();
    program.insert_function(code);
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    adapter.deny_diagnostic_path_read(level_path(host_ref));
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    let error = Vm::new()
        .run_program_with_host(
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host,
        )
        .expect_err("denied host read");

    assert_eq!(error.source_span, Some(span));
    assert_eq!(
        error.kind(),
        VmErrorKind::Host(HostErrorKind::PermissionDenied {
            path: level_path(host_ref),
            action: "read"
        })
    );
}

#[test]
fn runtime_errors_include_script_call_stack() {
    let leaf_error_span = Span::new(SourceId::new(1), 80, 86);
    let leaf_call_span = Span::new(SourceId::new(1), 44, 50);
    let middle_call_span = Span::new(SourceId::new(1), 18, 26);
    let mut program = Program::new();

    let mut main = CodeObject::new("main", 1);
    main.push_instruction(
        Instruction::new(InstructionKind::CallFunction {
            dst: Register(0),
            name: "middle".to_owned(),
            args: Vec::new(),
        })
        .with_span(middle_call_span),
    );
    main.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(0),
    }));
    program.insert_function(main);

    let mut middle = CodeObject::new("middle", 1);
    middle.push_instruction(
        Instruction::new(InstructionKind::CallFunction {
            dst: Register(0),
            name: "leaf".to_owned(),
            args: Vec::new(),
        })
        .with_span(leaf_call_span),
    );
    middle.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(0),
    }));
    program.insert_function(middle);

    let mut leaf = CodeObject::new("leaf", 3);
    let ten = leaf.push_constant(Constant::Int(10));
    let zero = leaf.push_constant(Constant::Int(0));
    leaf.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(0),
        constant: ten,
    }));
    leaf.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(1),
        constant: zero,
    }));
    leaf.push_instruction(
        Instruction::new(InstructionKind::Div {
            dst: Register(2),
            lhs: Register(0),
            rhs: Register(1),
        })
        .with_span(leaf_error_span),
    );
    leaf.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(2),
    }));
    program.insert_function(leaf);

    let error = Vm::new()
        .run_program(&program, "main", &[])
        .expect_err("division by zero should fail");

    assert_eq!(error.kind(), VmErrorKind::DivisionByZero);
    assert_eq!(error.source_span, Some(leaf_error_span));
    assert_eq!(
        error
            .call_stack
            .iter()
            .map(|frame| frame.function.as_str())
            .collect::<Vec<_>>(),
        ["leaf", "middle", "main"]
    );
    assert!(error.call_stack[0].call_site.is_some());
    assert!(error.call_stack[1].call_site.is_some());
    assert_eq!(error.call_stack[2].call_site, None);
    assert_eq!(
        error
            .call_stack
            .iter()
            .map(|frame| frame.bytecode_offset)
            .collect::<Vec<_>>(),
        [Some(InstructionOffset(0)), Some(InstructionOffset(0)), None]
    );
}
