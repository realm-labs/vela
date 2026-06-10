use super::*;
use crate::owned_value::OwnedValue;
use crate::value::Value as RuntimeValue;

#[test]
fn heap_safe_point_gc_preserves_caller_roots_during_nested_calls() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn allocate_garbage() {
    let temporary = "temporary";
    return 1;
}

fn main() {
    let player = Player { name: "outer", level: 1 };
    let ignored = allocate_garbage();
    let after = "after";
    return player.name;
}
"#,
    )
    .expect("compile nested heap source");
    let mut heap = ScriptHeap::new();
    heap.set_gc_config(heap::GcConfig {
        max_pause_micros: 500,
        heap_growth_factor: 1.0,
    });
    let mut heap_execution =
        HeapExecution::new(&mut heap).with_safe_point_gc_budget(GcBudget::unlimited());
    let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX);

    let result = run_linked_test_program_runtime_with_heap_and_budget(
        &Vm::new(),
        &program,
        "main",
        &[],
        &mut heap_execution,
        &mut budget,
    )
    .expect("run nested heap source");

    let RuntimeValue::HeapRef(result_ref) = result else {
        panic!("expected heap-backed field result");
    };
    assert_eq!(
        heap_execution.heap.get(result_ref),
        Some(&HeapValue::String("outer".into()))
    );
    assert_eq!(
        heap_execution
            .last_gc_step()
            .expect("safe-point GC should have run")
            .swept,
        1
    );
    assert_eq!(heap_execution.heap.live_object_count(), 3);
    assert_eq!(
        budget.memory_bytes_allocated(),
        heap_execution.heap.allocated_bytes()
    );
}

#[test]
fn managed_heap_execution_materializes_return_and_releases_budget() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    return Reward { item_id: "gold", count: 2 };
}
"#,
    )
    .expect("compile record return source");
    let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX);
    let mut fields = BTreeMap::new();
    fields.insert("count".into(), OwnedValue::Int(2));
    fields.insert("item_id".into(), OwnedValue::String("gold".into()));

    let result =
        run_linked_test_program_with_budget(&Vm::new(), &program, "main", &[], &mut budget)
            .expect("run managed heap source");

    assert_eq!(
        result,
        OwnedValue::Record {
            type_name: "Reward".into(),
            fields: ScriptFields::from_pairs("Reward", fields),
        }
    );
    assert_eq!(budget.memory_bytes_allocated(), 0);
}

#[test]
fn managed_heap_execution_preserves_path_proxy_slots() {
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(7), 3);
    let proxy = PathProxy::from_diagnostic_path(HostPath::new(host_ref).field(FieldId::new(2)));
    let expected = proxy.clone();
    let mut vm = Vm::new();
    vm.register_native("game::path", move |_| {
        Ok(OwnedValue::PathProxy(proxy.clone()))
    });
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn array_case() {
    let paths = [game::path()];
    return paths[0];
}

fn map_case() {
    let paths = {"level": game::path()};
    return paths["level"];
}
"#,
    )
    .expect("compile path proxy aggregate source");
    let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX);

    assert_eq!(
        run_linked_test_program_with_budget(&vm, &program, "array_case", &[], &mut budget),
        Ok(OwnedValue::PathProxy(expected.clone()))
    );
    assert_eq!(
        run_linked_test_program_with_budget(&vm, &program, "map_case", &[], &mut budget),
        Ok(OwnedValue::PathProxy(expected))
    );
    assert_eq!(budget.memory_bytes_allocated(), 0);
}

#[test]
fn linked_execution_rejects_missing_native_before_heap_execution() {
    let native = vela_def::FunctionId::new(0);
    let mut code = UnlinkedCodeObject::new("main", 2);
    let label = code.push_constant(Constant::String("allocated-before-error".into()));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(0),
            constant: label,
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallNative {
            dst: Some(Register(1)),
            name: "missing".into(),
            native,
            args: Vec::new(),
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(0),
    }));
    let mut program = UnlinkedProgram::new();
    program.insert_function(code);

    let error = Linker::new()
        .link_program(&program)
        .expect_err("missing native should fail during linking");

    assert!(matches!(
        error,
        vela_bytecode::LinkError::MissingNativeImplementation { name, id }
            if name == "missing" && id == native
    ));
}

#[test]
fn managed_heap_host_execution_materializes_return_and_updates_adapter() {
    let host_ref = player_ref(3);
    let mut code = UnlinkedCodeObject::new("main", 2).with_params(vec!["player".into()]);
    let gold = code.push_constant(Constant::String("gold".into()));
    let target =
        code.intern_host_target(HostTargetPlan::new(host_ref.type_id).field(level_field()));
    let cache_site = code.push_cache_site(CacheSiteKind::HostPathWrite, InstructionOffset(1));
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
            cache_site,
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(1),
    }));
    let mut program = UnlinkedProgram::new();
    program.insert_function(code);
    let mut adapter = host_adapter(host_ref, HostValue::String("old".into()));
    let mut tx = HostAccess::new();
    let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX);

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            access: &mut tx,
            script_globals: None,
        };
        run_linked_test_program_with_host_budget(
            &Vm::new(),
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host,
            &mut budget,
        )
        .expect("run managed host heap source")
    };

    assert_eq!(result, OwnedValue::String("gold".into()));
    assert_eq!(budget.memory_bytes_allocated(), 0);
}

#[test]
fn managed_heap_host_execution_rejects_map_for_host_write() {
    let host_ref = player_ref(3);
    let program = compile_host_program_source(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    player.level = {"class": "mage", score: 3};
    return 1;
}
"#,
        host_definition_registry(
            &[("Player", host_ref.type_id)],
            &[TestHostField::new("Player", "level", level_field())],
            &[],
        ),
    )
    .expect("compile host map write source");
    let mut adapter = host_adapter(host_ref, HostValue::Null);
    let mut tx = HostAccess::new();
    let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX);

    let error = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            access: &mut tx,
            script_globals: None,
        };
        run_linked_test_program_with_host_budget(
            &Vm::new(),
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host,
            &mut budget,
        )
        .expect_err("host map write should be rejected")
    };

    assert_eq!(
        error.kind(),
        VmErrorKind::TypeMismatch {
            operation: "set_host_field"
        }
    );
    assert_eq!(
        adapter.read_diagnostic_path(&level_path(host_ref)),
        Ok(HostValue::Null)
    );
    assert_eq!(budget.memory_bytes_allocated(), 0);
}

#[test]
fn managed_heap_host_execution_rejects_record_for_host_write() {
    let host_ref = player_ref(3);
    let program = compile_host_program_source(
        SourceId::new(1),
        r#"
struct Reward {
    item_id,
    count,
}

fn main(player: Player) {
    player.level = Reward { item_id: "gold", count: 2 };
    return player.level;
}
"#,
        host_definition_registry(
            &[("Player", host_ref.type_id)],
            &[TestHostField::new("Player", "level", level_field())],
            &[],
        ),
    )
    .expect("compile host record write source");
    let mut adapter = host_adapter(host_ref, HostValue::Null);
    let mut tx = HostAccess::new();
    let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX);

    let error = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            access: &mut tx,
            script_globals: None,
        };
        run_linked_test_program_with_host_budget(
            &Vm::new(),
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host,
            &mut budget,
        )
        .expect_err("host record write should be rejected")
    };

    assert_eq!(
        error.kind(),
        VmErrorKind::TypeMismatch {
            operation: "set_host_field"
        }
    );
    assert_eq!(
        adapter.read_diagnostic_path(&level_path(host_ref)),
        Ok(HostValue::Null)
    );
    assert_eq!(budget.memory_bytes_allocated(), 0);
}

#[test]
fn managed_heap_host_execution_rejects_enum_for_host_write() {
    let host_ref = player_ref(3);
    let program = compile_host_program_source(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    player.level = Damage::Physical { amount: 7 };
    return player.level;
}
"#,
        host_definition_registry(
            &[("Player", host_ref.type_id)],
            &[TestHostField::new("Player", "level", level_field())],
            &[],
        ),
    )
    .expect("compile host enum write source");
    let mut adapter = host_adapter(host_ref, HostValue::Null);
    let mut tx = HostAccess::new();
    let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX);

    let error = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            access: &mut tx,
            script_globals: None,
        };
        run_linked_test_program_with_host_budget(
            &Vm::new(),
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host,
            &mut budget,
        )
        .expect_err("host enum write should be rejected")
    };

    assert_eq!(
        error.kind(),
        VmErrorKind::TypeMismatch {
            operation: "set_host_field"
        }
    );
    assert_eq!(
        adapter.read_diagnostic_path(&level_path(host_ref)),
        Ok(HostValue::Null)
    );
    assert_eq!(budget.memory_bytes_allocated(), 0);
}

#[test]
fn managed_heap_host_execution_converts_host_ref_for_host_write_and_readback() {
    let host_ref = player_ref(3);
    let target_ref = HostRef::new(HostTypeId::new(2), HostObjectId::new(11), 4);
    let program = compile_host_program_source(
        SourceId::new(1),
        r#"
fn main(player: Player, target) {
    player.level = target;
    return player.level;
}
"#,
        host_definition_registry(
            &[("Player", host_ref.type_id)],
            &[TestHostField::new("Player", "level", level_field())],
            &[],
        ),
    )
    .expect("compile host ref write source");
    let mut adapter = host_adapter(host_ref, HostValue::Null);
    let mut tx = HostAccess::new();
    let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX);

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            access: &mut tx,
            script_globals: None,
        };
        run_linked_test_program_with_host_budget(
            &Vm::new(),
            &program,
            "main",
            &[
                OwnedValue::HostRef(host_ref),
                OwnedValue::HostRef(target_ref),
            ],
            &mut host,
            &mut budget,
        )
        .expect("run managed host ref source")
    };

    assert_eq!(result, OwnedValue::HostRef(target_ref));
    assert_eq!(
        adapter.read_diagnostic_path(&level_path(host_ref)),
        Ok(HostValue::HostRef(target_ref))
    );
    assert_eq!(budget.memory_bytes_allocated(), 0);
}
