use super::*;

#[test]
fn runs_basic_arithmetic() {
    let mut code = CodeObject::new("calc", 5);
    let two = code.push_constant(Constant::Int(2));
    let three = code.push_constant(Constant::Int(3));
    let four = code.push_constant(Constant::Int(4));
    code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(0),
        constant: two,
    }));
    code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(1),
        constant: three,
    }));
    code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(2),
        constant: four,
    }));
    code.push_instruction(Instruction::new(InstructionKind::Mul {
        dst: Register(3),
        lhs: Register(1),
        rhs: Register(2),
    }));
    code.push_instruction(Instruction::new(InstructionKind::Add {
        dst: Register(4),
        lhs: Register(0),
        rhs: Register(3),
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(4),
    }));

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(14)));
}

#[test]
fn branches_on_false_conditions() {
    let mut code = CodeObject::new("branch", 3);
    let false_id = code.push_constant(Constant::Bool(false));
    let one = code.push_constant(Constant::Int(1));
    let two = code.push_constant(Constant::Int(2));
    code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(0),
        constant: false_id,
    }));
    code.push_instruction(Instruction::new(InstructionKind::JumpIfFalse {
        condition: Register(0),
        target: InstructionOffset(4),
    }));
    code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(1),
        constant: one,
    }));
    code.push_instruction(Instruction::new(InstructionKind::Jump {
        target: InstructionOffset(5),
    }));
    code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(1),
        constant: two,
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(1),
    }));

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(2)));
}

#[test]
fn calls_registered_native_functions() {
    let mut vm = Vm::new();
    vm.register_native("log", |args| {
        assert_eq!(args, [Value::String("level up".into())]);
        Ok(Value::Null)
    });

    let mut code = CodeObject::new("native", 2);
    code.push_constant(Constant::String("level up".into()));
    code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(0),
        constant: ConstantId(0),
    }));
    code.push_instruction(Instruction::new(InstructionKind::CallNative {
        dst: Some(Register(1)),
        name: "log".into(),
        args: vec![Register(0)],
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(1),
    }));

    assert_eq!(vm.run(&code), Ok(Value::Null));
}

#[test]
fn instruction_budget_stops_dispatch_before_next_instruction() {
    let mut code = CodeObject::new("budgeted", 2);
    let one = code.push_constant(Constant::Int(1));
    code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(0),
        constant: one,
    }));
    code.push_instruction(Instruction::new(InstructionKind::Move {
        dst: Register(1),
        src: Register(0),
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(1),
    }));
    let mut budget = ExecutionBudget::new(2, usize::MAX, usize::MAX, usize::MAX);

    let error = Vm::new()
        .run_with_budget(&code, &mut budget)
        .expect_err("third instruction exceeds budget");

    assert_eq!(
        error.kind,
        VmErrorKind::BudgetExceeded {
            budget: ExecutionBudgetKind::Instructions,
            limit: 2,
        }
    );
    assert_eq!(budget.instructions_executed(), 2);
    assert_eq!(budget.current_call_depth(), 0);
}

#[test]
fn call_depth_budget_stops_recursive_scripts() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn recurse() {
    return recurse();
}

fn main() {
    return recurse();
}
"#,
    )
    .expect("compile recursive source");
    let mut budget = ExecutionBudget::new(100, usize::MAX, 2, usize::MAX);

    let error = Vm::new()
        .run_program_runtime_with_budget(&program, "main", &[], &mut budget)
        .expect_err("recursive call exceeds call depth");

    assert_eq!(
        error.kind,
        VmErrorKind::BudgetExceeded {
            budget: ExecutionBudgetKind::CallDepth,
            limit: 2,
        }
    );
    assert_eq!(budget.current_call_depth(), 0);
}

#[test]
fn call_frame_registers_expose_heap_roots_for_gc() {
    let mut heap = ScriptHeap::new();
    let rooted = heap.allocate(HeapValue::String("rooted".into()));
    let garbage = heap.allocate(HeapValue::String("garbage".into()));
    let mut frame = CallFrame::new(2);
    frame
        .write(Register(0), Value::HeapRef(rooted))
        .expect("write heap root");

    let roots = frame.heap_roots();
    let root_slots = frame.heap_root_slots();
    let stats = heap.collect_full(&roots);

    assert_eq!(roots, vec![rooted]);
    assert_eq!(root_slots.len(), 1);
    assert_eq!(root_slots[0].register, Register(0));
    assert_eq!(root_slots[0].reference, rooted);
    assert_eq!(stats.marked, 1);
    assert_eq!(stats.swept, 1);
    assert!(heap.contains(rooted));
    assert!(!heap.contains(garbage));
}

#[test]
fn nested_values_expose_heap_roots_for_gc() {
    let mut heap = ScriptHeap::new();
    let rooted = heap.allocate(HeapValue::String("nested".into()));
    let garbage = heap.allocate(HeapValue::String("garbage".into()));
    let mut fields = BTreeMap::new();
    fields.insert("item".into(), Value::HeapRef(rooted));
    let mut frame = CallFrame::new(1);
    frame
        .write(
            Register(0),
            Value::Record {
                type_name: "Reward".into(),
                fields: ScriptFields::from_pairs("Reward", fields),
            },
        )
        .expect("write nested root");

    let stats = heap.collect_full(&frame.heap_roots());

    assert_eq!(stats.marked, 1);
    assert_eq!(stats.swept, 1);
    assert!(heap.contains(rooted));
    assert!(!heap.contains(garbage));
}

#[test]
fn record_slot_bytecode_reads_and_writes_by_slot() {
    let mut code = CodeObject::new("slot_record", 3);
    let count = code.push_constant(Constant::Int(2));
    let updated = code.push_constant(Constant::Int(5));
    code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(0),
        constant: count,
    }));
    code.push_instruction(Instruction::new(InstructionKind::MakeRecord {
        dst: Register(1),
        type_name: "Reward".into(),
        fields: vec![
            ("item_id".into(), Register(0)),
            ("count".into(), Register(0)),
        ],
    }));
    code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(0),
        constant: updated,
    }));
    code.push_instruction(Instruction::new(InstructionKind::SetRecordSlot {
        record: Register(1),
        field: "count".into(),
        slot: 0,
        src: Register(0),
    }));
    code.push_instruction(Instruction::new(InstructionKind::GetRecordSlot {
        dst: Register(2),
        record: Register(1),
        field: "count".into(),
        slot: 0,
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(2),
    }));

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(5)));
}

#[test]
fn enum_slot_bytecode_reads_by_slot() {
    let mut code = CodeObject::new("slot_enum", 3);
    let amount = code.push_constant(Constant::Int(7));
    code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(0),
        constant: amount,
    }));
    code.push_instruction(Instruction::new(InstructionKind::MakeEnum {
        dst: Register(1),
        enum_name: "Damage".into(),
        variant: "Physical".into(),
        fields: vec![("amount".into(), Register(0))],
    }));
    code.push_instruction(Instruction::new(InstructionKind::GetEnumSlot {
        dst: Register(2),
        value: Register(1),
        field: "amount".into(),
        slot: 0,
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(2),
    }));

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(7)));
}

#[test]
fn runs_compiled_arithmetic_source() {
    let code = compile_function_source(
        SourceId::new(1),
        "fn main() { let base = 2; return base + 3 * 4; }",
        "main",
    )
    .expect("compile arithmetic source");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(14)));
}

#[test]
fn runs_compiled_radix_ints_and_exponent_floats() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let base = 0x10 + 0b10;
    let scaled = 3.5e+1 / 2.5;
    if base == 18 && scaled == 14.0 {
        return scaled;
    }
    return 0.0;
}
"#,
        "main",
    )
    .expect("compile numeric literal source");

    assert_eq!(Vm::new().run(&code), Ok(Value::Float(14.0)));
}

#[test]
fn runs_compiled_large_int_comparisons_without_float_rounding() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let low = 9007199254740992;
    let high = 9007199254740993;
    if low < high && high > low && low <= high && high >= low {
        return 1;
    }
    return 0;
}
"#,
        "main",
    )
    .expect("compile large int comparison source");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(1)));
}

#[test]
fn runs_compiled_scalar_equality_source() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    if "tick" == "tick"
        && "tick" != "tock"
        && true == true
        && false != true
        && 7 == 7
        && 7 != 8
        && 7 != 7.0
        && null == null
        && null != false
    {
        return 1;
    }
    return 0;
}
"#,
        "main",
    )
    .expect("compile scalar equality source");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(1)));
}

#[test]
fn runs_compiled_shebang_source() {
    let code = compile_function_source(
        SourceId::new(1),
        "#!/usr/bin/env vela\nfn main() { return 7; }\n",
        "main",
    )
    .expect("compile shebang source");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(7)));
}

#[test]
fn runs_compiled_unicode_string_escapes() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"fn main() { return "\u{41}\u{7a}"; }"#,
        "main",
    )
    .expect("compile unicode escaped string source");

    assert_eq!(Vm::new().run(&code), Ok(Value::String("Az".into())));
}

#[test]
fn runs_compiled_unary_operator_source() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    if !false {
        return -5;
    }
    return 0;
}
"#,
        "main",
    )
    .expect("compile unary operator source");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(-5)));
}

#[test]
fn runs_compiled_logical_short_circuit_source() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn and_case() {
    return false && fail();
}

fn or_case() {
    return true || fail();
}

fn truthy_case() {
    return true && 5 && ("reward" || fail());
}
"#,
    )
    .expect("compile logical short-circuit source");

    assert_eq!(
        Vm::new().run_program_runtime(&program, "and_case", &[]),
        Ok(Value::Bool(false))
    );
    assert_eq!(
        Vm::new().run_program_runtime(&program, "or_case", &[]),
        Ok(Value::Bool(true))
    );
    assert_eq!(
        Vm::new().run_program_runtime(&program, "truthy_case", &[]),
        Ok(Value::Bool(true))
    );
}

#[test]
fn runs_long_compiled_logical_chains_without_recursive_lowering() {
    let and_chain = std::iter::repeat_n("true", 160)
        .collect::<Vec<_>>()
        .join(" && ");
    let or_chain = std::iter::once("false")
        .chain(std::iter::repeat_n("false", 158))
        .chain(std::iter::once("true"))
        .collect::<Vec<_>>()
        .join(" || ");
    let source = format!(
        r#"
fn and_case() {{
    return {and_chain};
}}

fn or_case() {{
    return {or_chain};
}}
"#
    );
    let program =
        compile_program_source(SourceId::new(1), &source).expect("compile long logical chains");

    assert_eq!(
        Vm::new().run_program_runtime(&program, "and_case", &[]),
        Ok(Value::Bool(true))
    );
    assert_eq!(
        Vm::new().run_program_runtime(&program, "or_case", &[]),
        Ok(Value::Bool(true))
    );
}

#[test]
fn runs_compiled_local_assignment_source() {
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
    .expect("compile local assignment source");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(20)));
}

#[test]
fn runs_compiled_index_read_source() {
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
    .expect("compile index read source");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(10)));
}

#[test]
fn managed_heap_execution_reads_heap_index_values() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn array_case() {
    let names = ["gold", "xp"];
    return names[1];
}

fn map_case() {
    let rewards = { "gold": 7 };
    return rewards["gold"];
}
"#,
    )
    .expect("compile heap index source");
    let mut budget = ExecutionBudget::unbounded();

    assert_eq!(
        Vm::new()
            .run_program_runtime_with_managed_heap_and_budget(
                &program,
                "array_case",
                &[],
                &mut budget
            )
            .expect("run heap array index"),
        Value::String("xp".into())
    );
    assert_eq!(
        Vm::new()
            .run_program_runtime_with_managed_heap_and_budget(
                &program,
                "map_case",
                &[],
                &mut budget
            )
            .expect("run heap map index"),
        Value::Int(7)
    );
    assert_eq!(budget.memory_bytes_allocated(), 0);
}

#[test]
fn runs_compiled_index_write_source() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let values = [2, 4, 8];
    let rewards = { "xp": 6 };
    values[1] = 10;
    values[2] += 5;
    rewards["xp"] += values[1];
    rewards["gold"] = 3;
    let copy = (values[0] = rewards["gold"]);
    return values[0] + values[1] + values[2] + rewards["xp"] + copy;
}
"#,
        "main",
    )
    .expect("compile index write source");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(45)));
}

#[test]
fn runs_compiled_record_field_write_source() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let reward = Reward { item_id: "gold", count: 2 };
    reward.count += 3;
    reward.item_id = "xp";
    return reward.count + reward.item_id.len();
}
"#,
        "main",
    )
    .expect("compile record field write source");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(7)));
}

#[test]
fn runs_compiled_nested_record_field_write_source() {
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
    .expect("compile nested record field write source");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(11)));
}

#[test]
fn runs_compiled_indexed_record_field_write_source() {
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
    .expect("compile indexed record field write source");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(14)));
}

#[test]
fn managed_heap_execution_writes_heap_index_values() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn array_case() {
    let names = ["gold", "xp"];
    names[0] = "silver";
    return names[0];
}

fn map_case() {
    let rewards = { "gold": 7 };
    rewards["gold"] += 5;
    rewards["xp"] = 3;
    return rewards["gold"] + rewards["xp"];
}
"#,
    )
    .expect("compile heap index write source");
    let mut budget = ExecutionBudget::unbounded();

    assert_eq!(
        Vm::new()
            .run_program_runtime_with_managed_heap_and_budget(
                &program,
                "array_case",
                &[],
                &mut budget
            )
            .expect("run heap array index write"),
        Value::String("silver".into())
    );
    assert_eq!(
        Vm::new()
            .run_program_runtime_with_managed_heap_and_budget(
                &program,
                "map_case",
                &[],
                &mut budget
            )
            .expect("run heap map index write"),
        Value::Int(15)
    );
    assert_eq!(budget.memory_bytes_allocated(), 0);
}

#[test]
fn managed_heap_execution_writes_heap_record_fields() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    let reward = Reward { item_id: "gold", count: 2 };
    reward.count += 5;
    reward.item_id = "xp";
    return reward.count + reward.item_id.len();
}
"#,
    )
    .expect("compile heap record field writes");
    let mut budget = ExecutionBudget::unbounded();

    assert_eq!(
        Vm::new().run_program_runtime_with_managed_heap_and_budget(
            &program,
            "main",
            &[],
            &mut budget
        ),
        Ok(Value::Int(9))
    );
    assert_eq!(budget.memory_bytes_allocated(), 0);
}
