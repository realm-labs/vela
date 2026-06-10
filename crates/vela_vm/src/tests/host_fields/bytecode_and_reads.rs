use super::*;
use crate::value::Value as RuntimeValue;
use vela_bytecode::{CacheSiteId, CacheSiteKind, HostTargetPlanId};
use vela_host::resolved::HostMutationOp;
use vela_host::target::HostTargetPlan;

fn level_target(code: &mut UnlinkedCodeObject, host_ref: HostRef) -> HostTargetPlanId {
    code.intern_host_target(HostTargetPlan::new(host_ref.type_id).field(level_field()))
}

fn host_cache_site(
    code: &mut UnlinkedCodeObject,
    kind: CacheSiteKind,
    instruction_offset: usize,
) -> CacheSiteId {
    code.push_cache_site(kind, InstructionOffset(instruction_offset))
}

fn script_function_id(name: &str) -> vela_def::FunctionId {
    let mut segments = name.split("::").collect::<Vec<_>>();
    let function = segments.pop().unwrap_or(name);
    vela_def::FunctionId::from_def_id(
        vela_def::DefPath::function("script", segments, function).id(),
    )
}

#[test]
fn heap_execution_enforces_memory_budget_for_bytecode_allocations() {
    let code = compile_function_source(
        SourceId::new(1),
        "fn main() { return \"this string is too large\"; }",
        "main",
    )
    .expect("compile string source");
    let mut program = UnlinkedProgram::new();
    program.insert_function(code);
    let mut heap = ScriptHeap::new();
    let mut heap_execution = HeapExecution::new(&mut heap);
    let mut budget = ExecutionBudget::new(u64::MAX, 8, usize::MAX);

    let error = run_linked_test_program_runtime_with_heap_and_budget(
        &Vm::new(),
        &program,
        "main",
        &[],
        &mut heap_execution,
        &mut budget,
    )
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

    assert_eq!(run_linked_test_code(code), Ok(OwnedValue::Int(10)));
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

    assert_eq!(run_linked_test_code(code), Ok(OwnedValue::Int(20)));
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

    assert_eq!(run_linked_test_code(code), Ok(OwnedValue::Int(1)));
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
    let mut code = UnlinkedCodeObject::new("main", 3).with_params(vec!["player".into()]);
    let ten = code.push_constant(Constant::Int(10));
    let target = level_target(&mut code, host_ref);
    let write_cache = host_cache_site(&mut code, CacheSiteKind::HostPathWrite, 1);
    let read_cache = host_cache_site(&mut code, CacheSiteKind::HostPathRead, 2);
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(1),
            constant: ten,
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::HostWrite {
            root: Register(0),
            target,
            dynamic_args: Vec::new(),
            src: Register(1),
            cache_site: write_cache,
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
    let mut code = UnlinkedCodeObject::new("main", 3).with_params(vec!["player".into()]);
    let one = code.push_constant(Constant::Int(1));
    let target = level_target(&mut code, host_ref);
    let mutate_cache = host_cache_site(&mut code, CacheSiteKind::HostPathMutate, 1);
    let read_cache = host_cache_site(&mut code, CacheSiteKind::HostPathRead, 2);

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
    let mut code = UnlinkedCodeObject::new("main", 2).with_params(vec!["player".into()]);
    let gold = code.push_constant(Constant::String("gold".into()));
    let target = level_target(&mut code, host_ref);
    let write_cache = host_cache_site(&mut code, CacheSiteKind::HostPathWrite, 1);
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(1),
            constant: gold,
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::HostWrite {
            root: Register(0),
            target,
            dynamic_args: Vec::new(),
            src: Register(1),
            cache_site: write_cache,
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(1),
    }));
    let mut program = UnlinkedProgram::new();
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
    let mut code = UnlinkedCodeObject::new("main", 3).with_params(vec!["player".into()]);
    let ten = code.push_constant(Constant::Int(10));
    let eleven = code.push_constant(Constant::Int(11));
    let target = level_target(&mut code, host_ref);
    let first_write_cache = host_cache_site(&mut code, CacheSiteKind::HostPathWrite, 1);
    let second_write_cache = host_cache_site(&mut code, CacheSiteKind::HostPathWrite, 3);
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(1),
            constant: ten,
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::HostWrite {
            root: Register(0),
            target,
            dynamic_args: Vec::new(),
            src: Register(1),
            cache_site: first_write_cache,
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(2),
            constant: eleven,
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::HostWrite {
            root: Register(0),
            target,
            dynamic_args: Vec::new(),
            src: Register(2),
            cache_site: second_write_cache,
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(2),
    }));
    let mut program = UnlinkedProgram::new();
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
    let mut code = UnlinkedCodeObject::new("main", 3).with_params(vec!["player".into()]);
    let one = code.push_constant(Constant::Int(1));
    let target = level_target(&mut code, host_ref);
    let mutate_cache = host_cache_site(&mut code, CacheSiteKind::HostPathMutate, 1);
    let read_cache = host_cache_site(&mut code, CacheSiteKind::HostPathRead, 2);
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
    let mut code = UnlinkedCodeObject::new("main", 2).with_params(vec!["player".into()]);
    let target = level_target(&mut code, host_ref);
    let cache_site = host_cache_site(&mut code, CacheSiteKind::HostPathRead, 0);
    code.push_instruction(
        UnlinkedInstruction::new(UnlinkedInstructionKind::HostRead {
            dst: Register(1),
            root: Register(0),
            target,
            dynamic_args: Vec::new(),
            cache_site,
        })
        .with_span(span),
    );
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(1),
    }));
    let mut program = UnlinkedProgram::new();
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
    let mut program = UnlinkedProgram::new();

    let mut main = UnlinkedCodeObject::new("main", 1);
    main.push_instruction(
        UnlinkedInstruction::new(UnlinkedInstructionKind::CallFunction {
            dst: Register(0),
            target: script_function_id("middle"),
            name: "middle".to_owned(),
            args: Vec::new(),
        })
        .with_span(middle_call_span),
    );
    main.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(0),
    }));
    program.insert_function(main);

    let mut middle = UnlinkedCodeObject::new("middle", 1);
    middle.push_instruction(
        UnlinkedInstruction::new(UnlinkedInstructionKind::CallFunction {
            dst: Register(0),
            target: script_function_id("leaf"),
            name: "leaf".to_owned(),
            args: Vec::new(),
        })
        .with_span(leaf_call_span),
    );
    middle.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(0),
    }));
    program.insert_function(middle);

    let mut leaf = UnlinkedCodeObject::new("leaf", 3);
    let ten = leaf.push_constant(Constant::Int(10));
    let zero = leaf.push_constant(Constant::Int(0));
    leaf.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(0),
            constant: ten,
        },
    ));
    leaf.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(1),
            constant: zero,
        },
    ));
    leaf.push_instruction(
        UnlinkedInstruction::new(UnlinkedInstructionKind::Div {
            dst: Register(2),
            lhs: Register(0),
            rhs: Register(1),
        })
        .with_span(leaf_error_span),
    );
    leaf.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(2),
    }));
    program.insert_function(leaf);

    let linked = link_test_program(&program);
    let error = Vm::new()
        .run_linked_program(&linked, "main", &[])
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
