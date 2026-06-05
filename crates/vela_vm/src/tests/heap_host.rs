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
    let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX, usize::MAX);

    let result = Vm::new()
        .run_program_runtime_with_heap_and_budget(
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
    let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX, usize::MAX);
    let mut fields = BTreeMap::new();
    fields.insert("count".into(), OwnedValue::Int(2));
    fields.insert("item_id".into(), OwnedValue::String("gold".into()));

    let result = Vm::new()
        .run_program_with_managed_heap_and_budget(&program, "main", &[], &mut budget)
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
    let proxy = PathProxy::new(HostPath::new(host_ref).field(FieldId::new(2)));
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
    let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX, usize::MAX);

    assert_eq!(
        vm.run_program_with_managed_heap_and_budget(&program, "array_case", &[], &mut budget),
        Ok(OwnedValue::PathProxy(expected.clone()))
    );
    assert_eq!(
        vm.run_program_with_managed_heap_and_budget(&program, "map_case", &[], &mut budget),
        Ok(OwnedValue::PathProxy(expected))
    );
    assert_eq!(budget.memory_bytes_allocated(), 0);
}

#[test]
fn managed_heap_execution_releases_budget_after_errors() {
    let mut code = CodeObject::new("main", 2);
    let label = code.push_constant(Constant::String("allocated-before-error".into()));
    code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(0),
        constant: label,
    }));
    code.push_instruction(Instruction::new(InstructionKind::CallNative {
        dst: Some(Register(1)),
        name: "missing".into(),
        args: Vec::new(),
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(0),
    }));
    let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX, usize::MAX);

    let error = Vm::new()
        .run_with_managed_heap_and_budget(&code, &mut budget)
        .expect_err("missing native should fail");

    assert_eq!(
        error.kind,
        VmErrorKind::UnknownNative {
            name: "missing".into()
        }
    );
    assert_eq!(budget.memory_bytes_allocated(), 0);
}

#[test]
fn managed_heap_host_execution_materializes_return_and_records_patch() {
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
    let mut tx = PatchTx::new();
    let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX, usize::MAX);

    let result = {
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
            .expect("run managed host heap source")
    };

    assert_eq!(result, OwnedValue::String("gold".into()));
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(
        tx.patches()[0].op,
        PatchOp::Set(HostValue::String("gold".into()))
    );
    assert_eq!(budget.memory_bytes_allocated(), 0);
}

#[test]
fn managed_heap_host_execution_converts_map_for_host_write_and_overlay_read() {
    let host_ref = player_ref(3);
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main(player) {
    player.level = {"class": "mage", score: 3};
    return player.level.len();
}
"#,
        &CompilerOptions::new().with_host_field("level", level_field()),
    )
    .expect("compile host map write source");
    let mut adapter = host_adapter(host_ref, HostValue::Null);
    let mut tx = PatchTx::new();
    let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX, usize::MAX);

    let result = {
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
            .expect("run managed host map source")
    };

    let mut expected = BTreeMap::new();
    expected.insert("class".into(), HostValue::String("mage".into()));
    expected.insert("score".into(), HostValue::Int(3));
    assert_eq!(result, OwnedValue::Int(2));
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Map(expected)));
    assert_eq!(
        adapter.read_path(&level_path(host_ref)),
        Ok(HostValue::Null)
    );
    assert_eq!(budget.memory_bytes_allocated(), 0);
}

#[test]
fn managed_heap_host_execution_converts_record_for_host_write_and_overlay_read() {
    let host_ref = player_ref(3);
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
struct Reward {
    item_id,
    count,
}

fn main(player) {
    player.level = Reward { item_id: "gold", count: 2 };
    return player.level;
}
"#,
        &CompilerOptions::new().with_host_field("level", level_field()),
    )
    .expect("compile host record write source");
    let mut adapter = host_adapter(host_ref, HostValue::Null);
    let mut tx = PatchTx::new();
    let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX, usize::MAX);

    let result = {
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
            .expect("run managed host record source")
    };

    let mut expected_script_fields = BTreeMap::new();
    expected_script_fields.insert("count".into(), OwnedValue::Int(2));
    expected_script_fields.insert("item_id".into(), OwnedValue::String("gold".into()));
    let mut expected_host_fields = BTreeMap::new();
    expected_host_fields.insert("count".into(), HostValue::Int(2));
    expected_host_fields.insert("item_id".into(), HostValue::String("gold".into()));
    assert_eq!(
        result,
        OwnedValue::Record {
            type_name: "Reward".into(),
            fields: ScriptFields::from_pairs("Reward", expected_script_fields),
        }
    );
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(
        tx.patches()[0].op,
        PatchOp::Set(HostValue::Record {
            type_name: "Reward".into(),
            fields: expected_host_fields,
        })
    );
    assert_eq!(
        adapter.read_path(&level_path(host_ref)),
        Ok(HostValue::Null)
    );
    assert_eq!(budget.memory_bytes_allocated(), 0);
}

#[test]
fn managed_heap_host_execution_converts_enum_for_host_write_and_overlay_read() {
    let host_ref = player_ref(3);
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main(player) {
    player.level = Damage::Physical { amount: 7 };
    return player.level;
}
"#,
        &CompilerOptions::new().with_host_field("level", level_field()),
    )
    .expect("compile host enum write source");
    let mut adapter = host_adapter(host_ref, HostValue::Null);
    let mut tx = PatchTx::new();
    let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX, usize::MAX);

    let result = {
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
            .expect("run managed host enum source")
    };

    let mut expected_script_fields = BTreeMap::new();
    expected_script_fields.insert("amount".into(), OwnedValue::Int(7));
    let mut expected_host_fields = BTreeMap::new();
    expected_host_fields.insert("amount".into(), HostValue::Int(7));
    assert_eq!(
        result,
        OwnedValue::Enum {
            enum_name: "Damage".into(),
            variant: "Physical".into(),
            fields: ScriptFields::from_pairs("Damage::Physical", expected_script_fields),
        }
    );
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(
        tx.patches()[0].op,
        PatchOp::Set(HostValue::Enum {
            enum_name: "Damage".into(),
            variant: "Physical".into(),
            fields: expected_host_fields,
        })
    );
    assert_eq!(
        adapter.read_path(&level_path(host_ref)),
        Ok(HostValue::Null)
    );
    assert_eq!(budget.memory_bytes_allocated(), 0);
}

#[test]
fn managed_heap_host_execution_converts_host_ref_for_host_write_and_overlay_read() {
    let host_ref = player_ref(3);
    let target_ref = HostRef::new(HostTypeId::new(2), HostObjectId::new(11), 4);
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main(player, target) {
    player.level = target;
    return player.level;
}
"#,
        &CompilerOptions::new().with_host_field("level", level_field()),
    )
    .expect("compile host ref write source");
    let mut adapter = host_adapter(host_ref, HostValue::Null);
    let mut tx = PatchTx::new();
    let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX, usize::MAX);

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };
        Vm::new()
            .run_program_with_host_managed_heap_and_budget(
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
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(
        tx.patches()[0].op,
        PatchOp::Set(HostValue::HostRef(target_ref))
    );
    assert_eq!(
        adapter.read_path(&level_path(host_ref)),
        Ok(HostValue::Null)
    );
    assert_eq!(budget.memory_bytes_allocated(), 0);
}
