use super::*;
use crate::heap::{GcBudget, HeapSlot, HeapValue, ScriptHeap};
use std::collections::BTreeMap;
use std::num::NonZeroU32;
use std::sync::Arc;
use vela_bytecode::compiler::{
    CompilerOptions, compile_function_source, compile_module_sources, compile_program_source,
    compile_program_source_with_options,
};
use vela_bytecode::{Constant, ConstantId, HostPathSegment, Instruction, InstructionOffset};
use vela_common::{
    FieldId, FunctionId, HostMethodId, HostObjectId, HostTypeId, MethodId, SourceId, Symbol,
    TypeId, VariantId,
};
use vela_hir::{ModuleGraph, ModulePath, ModuleSource};
use vela_host::{
    HostErrorKind, HostPath, HostRef, HostValue, MockStateAdapter, PatchOp, PathProxy,
};
use vela_reflect::{
    FieldAccess, FieldDesc, FunctionAccess, FunctionDesc, MethodAccess, MethodDesc,
    MethodEffectSet, MethodParamDesc, ModuleDesc, ReflectCandidate, ReflectErrorKind, TraitDesc,
    TraitMethodDesc, TypeDesc, TypeKey, TypeKind, VariantDesc,
};

mod consts;
mod host_methods;
mod reflection_members;
mod reflection_modules;
mod reflection_values;

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
        .run_program_with_budget(&program, "main", &[], &mut budget)
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
    let stats = heap.collect_full(&roots);

    assert_eq!(roots, vec![rooted]);
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
        Vm::new().run_program(&program, "and_case", &[]),
        Ok(Value::Bool(false))
    );
    assert_eq!(
        Vm::new().run_program(&program, "or_case", &[]),
        Ok(Value::Bool(true))
    );
    assert_eq!(
        Vm::new().run_program(&program, "truthy_case", &[]),
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
            .run_program_with_managed_heap_and_budget(&program, "array_case", &[], &mut budget)
            .expect("run heap array index"),
        Value::String("xp".into())
    );
    assert_eq!(
        Vm::new()
            .run_program_with_managed_heap_and_budget(&program, "map_case", &[], &mut budget)
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
            .run_program_with_managed_heap_and_budget(&program, "array_case", &[], &mut budget)
            .expect("run heap array index write"),
        Value::String("silver".into())
    );
    assert_eq!(
        Vm::new()
            .run_program_with_managed_heap_and_budget(&program, "map_case", &[], &mut budget)
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
        Vm::new().run_program_with_managed_heap_and_budget(&program, "main", &[], &mut budget),
        Ok(Value::Int(9))
    );
    assert_eq!(budget.memory_bytes_allocated(), 0);
}

#[test]
fn runs_compiled_for_in_source() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let total = 0;
    for value in [1, 2, 3] {
        total += value;
    }
    let rewards = { "gold": 4, "xp": 6 };
    for reward in rewards {
        total += reward;
    }
    return total;
}
"#,
        "main",
    )
    .expect("compile for-in source");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(16)));
}

#[test]
fn runs_compiled_for_in_variant_patterns() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
enum Reward {
    Grant { amount },
    Skip { amount },
}

fn main() {
    let total = 0;
    let rewards = [
        Reward.Grant { amount: 2 },
        Reward.Skip { amount: 100 },
        Reward.Grant { amount: 5 },
    ];
    for Reward.Grant { amount } in rewards {
        total += amount;
    }
    return total;
}
"#,
    )
    .expect("compile for-in variant patterns");

    assert_eq!(
        Vm::new().run_program(&program, "main", &[]),
        Ok(Value::Int(7))
    );
}

#[test]
fn runs_compiled_statement_attributes_as_metadata() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    #[trace("setup")]
    let total = 1;
    #[audit]
    total += 2;
    return total;
}
"#,
        "main",
    )
    .expect("compile statement attributes");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(3)));
}

#[test]
fn runs_compiled_for_in_over_native_iterator() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let total = 0;
    for value in game.values() {
        total += value;
    }
    return total;
}
"#,
        "main",
    )
    .expect("compile native iterator for-in source");
    let mut vm = Vm::new();
    vm.register_native("game.values", |_| {
        Ok(Value::Iterator(IteratorState::from_values(vec![
            Value::Int(2),
            Value::Int(3),
            Value::Int(5),
        ])))
    });

    assert_eq!(vm.run(&code), Ok(Value::Int(10)));
}

#[test]
fn runs_compiled_range_for_in_source() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let total = 0;
    for value in 1..4 {
        total += value;
    }
    for value in 4..=5 {
        total += value;
    }
    return total;
}
"#,
        "main",
    )
    .expect("compile range for-in source");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(15)));
}

#[test]
fn runs_compiled_script_value_methods() {
    let program = compile_program_source(
            SourceId::new(1),
            r#"
fn main() {
    let values = [1, 2, 3];
    let rewards = {"gold": 4, "xp": 6};
    let empty = [];
    values.push(4);
    let popped = values.pop();
    let missing_pop = empty.pop();
    rewards.set("quest", 8);
    let missing_get = rewards.get("missing_before");
    let removed = rewards.remove("gold");
    let missing_remove = rewards.remove("missing_after");
    let keys = rewards.keys();
    let amounts = rewards.values();
    let entries = rewards.entries();
    if empty.is_empty() && values.len() == 3 && option.unwrap_or(popped, 0) == 4 && rewards.len() == 2 && ("gold").len() == 4
        && ("gold").contains("ol") && ("quest").starts_with("que") && ("quest").ends_with("st")
        && option.is_none(missing_pop)
        && option.unwrap_or(removed, 0) == 4
        && option.is_none(missing_get) && option.is_none(missing_remove)
        && rewards.has("quest") && option.unwrap_or(rewards.get("xp"), 0) == 6 && rewards.get_or("missing", 10) == 10
        && keys[0] == "quest" && keys[1] == "xp"
        && amounts[0] == 8 && amounts[1] == 6
        && entries[0].key == "quest" && entries[1].value == 6 {
        return values.len();
    }
    return 0;
}
"#,
        )
        .expect("compile script value methods");

    let mut vm = Vm::new();
    vm.register_standard_natives();

    assert_eq!(vm.run_program(&program, "main", &[]), Ok(Value::Int(3)));
}

#[test]
fn runs_compiled_script_impl_method_dispatch() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource { fn bonus(self, amount) -> int; }
struct Player { level: int }

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
        return self.level + amount;
    }
}

fn main() {
    let player = Player { level: 7 };
    return player.bonus(5);
}
"#,
    )
    .expect("compile script impl method dispatch");

    assert_eq!(
        Vm::new().run_program(&program, "main", &[]),
        Ok(Value::Int(12))
    );
}

#[test]
fn runs_compiled_script_method_named_and_default_args() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource {
    fn bonus(self, amount, multiplier = 2, offset = 1) -> int;
}
struct Player { level: int }

impl BonusSource for Player {
    fn bonus(self, amount, multiplier = 2, offset = 1) -> int {
        return self.level + amount * multiplier + offset;
    }
}

fn main() {
    let player = Player { level: 7 };
    return player.bonus(offset = 4, amount = 5)
        + Player { level: 3 }.bonus(amount = 2);
}
"#,
    )
    .expect("compile script method named/default args");

    assert_eq!(
        Vm::new().run_program(&program, "main", &[]),
        Ok(Value::Int(29))
    );
}

#[test]
fn runs_compiled_typed_parameter_method_id_dispatch() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource { fn bonus(self, amount) -> int; }
struct Player { level: int }

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
        return self.level + amount;
    }
}

fn main(player: Player) {
    return player.bonus(5);
}
"#,
    )
    .expect("compile typed parameter method id dispatch");
    let player = Value::Record {
        type_name: "Player".to_owned(),
        fields: ScriptFields::from_pairs("Player", [("level".to_owned(), Value::Int(7))]),
    };

    assert_eq!(
        Vm::new().run_program(&program, "main", &[player]),
        Ok(Value::Int(12))
    );
}

#[test]
fn runs_compiled_immediate_script_method_id_dispatch() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource { fn bonus(self, amount) -> int; }
struct Player { level: int }

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
        return self.level + amount;
    }
}

fn main() {
    return Player { level: 7 }.bonus(5);
}
"#,
    )
    .expect("compile immediate script method id dispatch");

    assert_eq!(
        Vm::new().run_program(&program, "main", &[]),
        Ok(Value::Int(12))
    );
}

#[test]
fn runs_compiled_trait_default_method_dispatch() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource {
    fn bonus(self, amount) -> int { return self.level + amount; }
    fn label(self) -> string { return self.name; }
}
struct Player { level: int, name: string }

impl BonusSource for Player {}

fn main() {
    let player = Player { level: 7, name: "hero" };
    return player.bonus(5) + player.label().len();
}
"#,
    )
    .expect("compile trait default method dispatch");

    assert_eq!(
        Vm::new().run_program(&program, "main", &[]),
        Ok(Value::Int(16))
    );
}

#[test]
fn runs_compiled_self_method_id_dispatch() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource {
    fn label(self) -> string;
    fn summary(self) -> string { return self.label(); }
}
struct Player { name: string }

impl BonusSource for Player {
    fn label(self) -> string {
        return self.name;
    }
}

fn main() {
    return Player { name: "hero" }.summary();
}
"#,
    )
    .expect("compile self method id dispatch");

    assert_eq!(
        Vm::new().run_program(&program, "main", &[]),
        Ok(Value::String("hero".to_owned()))
    );
}

#[test]
fn runs_compiled_captured_receiver_method_id_dispatch() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource { fn bonus(self, amount) -> int; }
struct Player { level: int }

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
        return self.level + amount;
    }
}

fn main() {
    let player = Player { level: 7 };
    let bonus = |ignored| player.bonus(5);
    return bonus(null);
}
"#,
    )
    .expect("compile captured receiver method id dispatch");

    assert_eq!(
        Vm::new().run_program(&program, "main", &[]),
        Ok(Value::Int(12))
    );
}

#[test]
fn runs_compiled_binding_pattern_receiver_method_id_dispatch() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource { fn bonus(self, amount) -> int; }
struct Player { level: int }

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
        return self.level + amount;
    }
}

fn main() {
    let player = Player { level: 7 };
    return match player {
        bound => bound.bonus(5),
    };
}
"#,
    )
    .expect("compile binding pattern receiver method id dispatch");

    assert_eq!(
        Vm::new().run_program(&program, "main", &[]),
        Ok(Value::Int(12))
    );
}

#[test]
fn runs_compiled_host_ref_script_impl_method_dispatch() {
    let host_ref = player_ref(3);
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource { fn bonus(self, amount) -> int; }

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
        return reflect.get(self, "level") + amount;
    }
}

fn main(player) {
    return player.bonus(5);
}
"#,
    )
    .expect("compile host ref script impl method dispatch");
    let mut adapter = host_adapter(host_ref, HostValue::Int(7));
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(reflection_registry()));
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        vm.run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host),
        Ok(Value::Int(12))
    );
    assert!(tx.patches().is_empty());
}

#[test]
fn host_ref_script_impl_dispatch_uses_registered_type_registry() {
    let host_ref = player_ref(3);
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource { fn bonus(self, amount) -> int; }

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
        return amount + 7;
    }
}

fn main(player) {
    return player.bonus(5);
}
"#,
    )
    .expect("compile host ref script impl method dispatch");
    let mut adapter = host_adapter(host_ref, HostValue::Int(7));
    let mut tx = PatchTx::new();
    let vm = Vm::new().with_type_registry(Arc::new(reflection_registry()));
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        vm.run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host),
        Ok(Value::Int(12))
    );
    assert!(tx.patches().is_empty());
}

#[test]
fn runs_compiled_record_variant_field_method_id_dispatch() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource { fn bonus(self, amount) -> int; }
struct Player { level: int }

enum Event {
    Grant { player: Player },
    None,
}

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
        return self.level + amount;
    }
}

fn main() {
    let event = Event.Grant { player: Player { level: 7 } };
    return match event {
        Event.Grant { player } => player.bonus(5),
        _ => 0,
    };
}
"#,
    )
    .expect("compile record variant field method id dispatch");

    assert_eq!(
        Vm::new().run_program(&program, "main", &[]),
        Ok(Value::Int(12))
    );
}

#[test]
fn runs_compiled_tuple_variant_field_method_id_dispatch() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource { fn bonus(self, amount) -> int; }
struct Player { level: int }

enum Event {
    Grant(player: Player),
    None,
}

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
        return self.level + amount;
    }
}

fn main() {
    let event = Event.Grant(Player { level: 7 });
    return match event {
        Event.Grant(player) => player.bonus(5),
        _ => 0,
    };
}
"#,
    )
    .expect("compile tuple variant field method id dispatch");

    assert_eq!(
        Vm::new().run_program(&program, "main", &[]),
        Ok(Value::Int(12))
    );
}

#[test]
fn explicit_impl_method_overrides_trait_default_dispatch() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource {
    fn bonus(self, amount) -> int { return self.level + amount; }
}
struct Player { level: int }

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
        return amount * 2;
    }
}

fn main() {
    let player = Player { level: 7 };
    return player.bonus(5);
}
"#,
    )
    .expect("compile explicit impl method override");

    assert_eq!(
        Vm::new().run_program(&program, "main", &[]),
        Ok(Value::Int(10))
    );
}

#[test]
fn runs_compiled_module_qualified_script_impl_method_dispatch() {
    let program = compile_module_sources(&[ModuleSource::new(
        SourceId::new(1),
        ModulePath::from_dotted("game.combat"),
        r#"
trait BonusSource { fn bonus(self, amount) -> int; }
struct Player { level: int }

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
        return self.level + amount;
    }
}

pub fn main() {
    let player = Player { level: 10 };
    return player.bonus(4);
}
"#,
    )])
    .expect("compile module-qualified script impl method dispatch");

    assert_eq!(
        Vm::new().run_program(&program, "game.combat.main", &[]),
        Ok(Value::Int(14))
    );
}

#[test]
fn runs_compiled_module_typed_parameter_method_id_dispatch() {
    let program = compile_module_sources(&[
        ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_dotted("game.model"),
            r#"
pub trait BonusSource { fn bonus(self, amount) -> int; }
pub struct Player { level: int }

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
        return self.level + amount;
    }
}
"#,
        ),
        ModuleSource::new(
            SourceId::new(2),
            ModulePath::from_dotted("game.combat"),
            r#"
use game.model.Player

pub fn main(player: Player) {
    return player.bonus(5);
}
"#,
        ),
    ])
    .expect("compile module typed parameter method id dispatch");
    let player = Value::Record {
        type_name: "game.model.Player".to_owned(),
        fields: ScriptFields::from_pairs(
            "game.model.Player",
            [("level".to_owned(), Value::Int(7))],
        ),
    };

    assert_eq!(
        Vm::new().run_program(&program, "game.combat.main", &[player]),
        Ok(Value::Int(12))
    );
}

#[test]
fn runs_compiled_break_continue_source() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let total = 0;
    for value in [1, 2, 3, 4, 5] {
        if value == 2 {
            continue;
        }
        if value == 5 {
            break;
        }
        total += value;
    }
    return total;
}
"#,
        "main",
    )
    .expect("compile break and continue source");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(8)));
}

#[test]
fn runs_compiled_block_and_if_expression_values() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let value = {
        let base = 2;
        base + 3;
    };
    let selected = if value > 4 {
        value;
    } else {
        0;
    };
    return selected;
}
"#,
        "main",
    )
    .expect("compile block and if expression values");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(5)));
}

#[test]
fn runs_compiled_if_expression_without_else_as_null() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let missing = if false {
        3;
    };
    let value = if true {
        7;
    };
    if missing == null {
        return value;
    }
    return 0;
}
"#,
        "main",
    )
    .expect("compile no-else if expression");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(7)));
}

#[test]
fn runs_compiled_if_expression_without_else_false_branch_as_null() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let value = if false {
        7;
    };
    if value == null {
        return 1;
    }
    return 0;
}
"#,
        "main",
    )
    .expect("compile no-else if expression");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(1)));
}

#[test]
fn runs_compiled_returning_block_initializer() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let ignored = {
        return 7;
    };
    return 0;
}
"#,
        "main",
    )
    .expect("compile returning block initializer");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(7)));
}

#[test]
fn runs_compiled_returning_expression_operands() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn block_arg() {
    log({
        return 7;
    });
    return 0;
}

fn if_value(flag) {
    return if flag {
        return 1;
    } else {
        return 2;
    };
}

fn match_value(value) {
    return match value {
        1 => { return 10; },
        _ => { return 11; },
    };
}
"#,
    )
    .expect("compile returning expression operands");

    assert_eq!(
        Vm::new().run_program(&program, "block_arg", &[]),
        Ok(Value::Int(7))
    );
    assert_eq!(
        Vm::new().run_program(&program, "if_value", &[Value::Bool(true)]),
        Ok(Value::Int(1))
    );
    assert_eq!(
        Vm::new().run_program(&program, "if_value", &[Value::Bool(false)]),
        Ok(Value::Int(2))
    );
    assert_eq!(
        Vm::new().run_program(&program, "match_value", &[Value::Int(1)]),
        Ok(Value::Int(10))
    );
    assert_eq!(
        Vm::new().run_program(&program, "match_value", &[Value::Int(9)]),
        Ok(Value::Int(11))
    );
}

#[test]
fn runs_compiled_returning_if_and_match_initializers() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn if_case(flag) {
    let ignored = if flag {
        return 7;
    } else {
        return 8;
    };
    return 0;
}

fn match_case(value) {
    let ignored = match value {
        1 => { return 10; },
        _ => { return 11; },
    };
    return 0;
}
"#,
    )
    .expect("compile returning if and match initializers");

    assert_eq!(
        Vm::new().run_program(&program, "if_case", &[Value::Bool(true)]),
        Ok(Value::Int(7))
    );
    assert_eq!(
        Vm::new().run_program(&program, "if_case", &[Value::Bool(false)]),
        Ok(Value::Int(8))
    );
    assert_eq!(
        Vm::new().run_program(&program, "match_case", &[Value::Int(1)]),
        Ok(Value::Int(10))
    );
    assert_eq!(
        Vm::new().run_program(&program, "match_case", &[Value::Int(2)]),
        Ok(Value::Int(11))
    );
}

#[test]
fn runs_compiled_match_expression_values() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let damage = Damage.Physical { amount: 7 };
    let value = match damage {
        Damage.Magical { amount } => amount + 100,
        Damage.Physical { amount } => {
            amount + 1;
        },
        _ => 0,
    };
    return value;
}
"#,
        "main",
    )
    .expect("compile match expression values");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(8)));
}

#[test]
fn runs_compiled_literal_match_patterns() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let value = 2;
    return match value {
        1 => 10,
        2 => 20,
        _ => 0,
    };
}
"#,
        "main",
    )
    .expect("compile literal match patterns");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(20)));
}

#[test]
fn managed_heap_execution_runs_string_literal_match_patterns() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    let label = "xp";
    return match label {
        "gold" => 1,
        "xp" => 2,
        _ => 0,
    };
}
"#,
    )
    .expect("compile heap string literal match patterns");
    let mut budget = ExecutionBudget::unbounded();

    assert_eq!(
        Vm::new()
            .run_program_with_managed_heap_and_budget(&program, "main", &[], &mut budget)
            .expect("run heap string literal match patterns"),
        Value::Int(2)
    );
    assert_eq!(budget.memory_bytes_allocated(), 0);
}

#[test]
fn runs_compiled_binding_match_patterns() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let value = 7;
    return match value {
        bound => bound + 1,
    };
}
"#,
        "main",
    )
    .expect("compile binding match patterns");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(8)));
}

#[test]
fn binding_match_assignment_does_not_mutate_scrutinee() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let value = 7;
    match value {
        bound => {
            bound = 100;
        }
    }
    return value;
}
"#,
        "main",
    )
    .expect("compile binding match assignment");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(7)));
}

#[test]
fn runs_compiled_match_guards() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let value = 7;
    return match value {
        bound if bound < 5 => 10,
        bound if bound == 7 => bound + 1,
        _ => 0,
    };
}
"#,
        "main",
    )
    .expect("compile match guards");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(8)));
}

#[test]
fn match_guards_can_read_record_pattern_bindings() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let damage = Damage.Physical { amount: 7 };
    return match damage {
        Damage.Physical { amount } if amount > 10 => 100,
        Damage.Physical { amount } if amount == 7 => amount + 1,
        _ => 0,
    };
}
"#,
        "main",
    )
    .expect("compile tuple variant literal pattern");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(8)));
}

#[test]
fn runs_compiled_record_variant_field_patterns() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
enum Reward {
    Grant { kind, amount }
}

fn main() {
    let reward = Reward.Grant { kind: "xp", amount: 7 };
    return match reward {
        Reward.Grant { kind: "gold", amount } => amount,
        Reward.Grant { kind: "xp", amount } => amount + 1,
        _ => 0,
    };
}
"#,
        "main",
    )
    .expect("compile record variant field patterns");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(8)));
}

#[test]
fn managed_heap_execution_runs_nested_record_variant_field_patterns() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
enum Reward {
    Grant { payload }
}

enum Payload {
    Xp(amount)
    Gold(amount)
}

fn main() {
    let reward = Reward.Grant { payload: Payload.Xp(7) };
    return match reward {
        Reward.Grant { payload: Payload.Gold(amount) } => amount,
        Reward.Grant { payload: Payload.Xp(amount) } => amount + 1,
        _ => 0,
    };
}
"#,
    )
    .expect("compile nested record variant field patterns");
    let mut budget = ExecutionBudget::new(10_000, 32_000, 32, 32);

    assert_eq!(
        Vm::new().run_program_with_managed_heap_and_budget(&program, "main", &[], &mut budget),
        Ok(Value::Int(8))
    );
}

#[test]
fn runs_compiled_tuple_variant_constructor_and_patterns() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
enum Damage {
    Physical(amount, bonus),
    Magical(amount),
}

fn main() {
    let damage = Damage.Physical(7, 2);
    return match damage {
        Damage.Physical(amount, bonus) => amount + bonus,
        _ => 0,
    };
}
"#,
        "main",
    )
    .expect("compile tuple variant constructor and pattern");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(9)));
}

#[test]
fn managed_heap_execution_runs_tuple_variant_literal_patterns() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
enum Damage {
    Typed(kind, amount),
}

fn main() {
    let damage = Damage.Typed("fire", 7);
    return match damage {
        Damage.Typed("frost", amount) => amount + 100,
        Damage.Typed("fire", amount) => amount + 1,
        _ => 0,
    };
}
"#,
        "main",
    )
    .expect("compile guarded record pattern");

    let mut budget = ExecutionBudget::new(10_000, 32_000, 32, 32);
    assert_eq!(
        Vm::new().run_with_managed_heap_and_budget(&code, &mut budget),
        Ok(Value::Int(8))
    );
}

#[test]
fn managed_heap_execution_runs_for_in_source() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn sum() {
    let total = 0;
    for value in [1, 2, 3] {
        total += value;
    }
    for reward in { "gold": 4, "xp": 6 } {
        total += reward;
    }
    return total;
}

fn last_name() {
    let name = "";
    for value in ["gold", "xp"] {
        name = value;
    }
    return name;
}
"#,
    )
    .expect("compile heap for-in source");
    let mut budget = ExecutionBudget::unbounded();

    assert_eq!(
        Vm::new()
            .run_program_with_managed_heap_and_budget(&program, "sum", &[], &mut budget)
            .expect("run heap for-in sum"),
        Value::Int(16)
    );
    assert_eq!(
        Vm::new()
            .run_program_with_managed_heap_and_budget(&program, "last_name", &[], &mut budget)
            .expect("run heap for-in string"),
        Value::String("xp".into())
    );
    assert_eq!(budget.memory_bytes_allocated(), 0);
}

#[test]
fn managed_heap_execution_runs_native_iterator_for_in_source() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    let names = [];
    for value in game.names() {
        names.push(value.to_upper());
    }
    return names.join(",");
}
"#,
    )
    .expect("compile heap native iterator for-in source");
    let mut vm = Vm::new();
    vm.register_standard_natives();
    vm.register_native("game.names", |_| {
        Ok(Value::Iterator(IteratorState::from_values(vec![
            Value::String("gold".to_owned()),
            Value::String("xp".to_owned()),
        ])))
    });
    let mut budget = ExecutionBudget::unbounded();

    assert_eq!(
        vm.run_program_with_managed_heap_and_budget(&program, "main", &[], &mut budget),
        Ok(Value::String("GOLD,XP".to_owned()))
    );
    assert_eq!(budget.memory_bytes_allocated(), 0);
}

#[test]
fn managed_heap_execution_runs_range_for_in_source() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    let total = 0;
    for value in 2..=4 {
        total += value;
    }
    return total;
}
"#,
    )
    .expect("compile heap range for-in source");
    let mut budget = ExecutionBudget::unbounded();

    assert_eq!(
        Vm::new().run_program_with_managed_heap_and_budget(&program, "main", &[], &mut budget),
        Ok(Value::Int(9))
    );
    assert_eq!(budget.memory_bytes_allocated(), 0);
}

#[test]
fn managed_heap_execution_runs_script_value_methods() {
    let program = compile_program_source(
            SourceId::new(1),
            r#"
fn main() {
    let names = ["gold", "xp"];
    let empty = [];
    let rewards = {"gold": 4, "xp": 6};
    names.push("quest");
    let popped = names.pop();
    let missing_pop = empty.pop();
    rewards.set("quest", "done");
    let missing_get = rewards.get("missing_before");
    let removed = rewards.remove("gold");
    let missing_remove = rewards.remove("missing_after");
    let keys = rewards.keys();
    let amounts = rewards.values();
    let entries = rewards.entries();
    let popped_name = option.unwrap_or(popped, "");
    if names.len() == 2 && popped_name == "quest" && popped_name.contains("ue") && popped_name.starts_with("que")
        && popped_name.ends_with("st") && option.is_none(missing_pop) && option.unwrap_or(removed, 0) == 4 && rewards.is_empty() == false && ("quest").len() == 5
        && option.is_none(missing_get) && option.is_none(missing_remove)
        && rewards.has("quest") && option.unwrap_or(rewards.get("xp"), 0) == 6 && rewards.get_or("missing", "fallback") == "fallback"
        && keys[0] == "quest" && keys[1] == "xp"
        && amounts[0] == "done" && amounts[1] == 6
        && entries[0].key == "quest" && entries[1].value == 6 {
        return names[0].len();
    }
    return 0;
}
"#,
        )
        .expect("compile heap script value methods");
    let mut budget = ExecutionBudget::unbounded();

    let mut vm = Vm::new();
    vm.register_standard_natives();

    assert_eq!(
        vm.run_program_with_managed_heap_and_budget(&program, "main", &[], &mut budget),
        Ok(Value::Int(4))
    );
    assert_eq!(budget.memory_bytes_allocated(), 0);
}

#[test]
fn managed_heap_execution_runs_script_impl_method_dispatch() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource { fn bonus(self, amount) -> int; }
struct Player { level: int }

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
        return self.level + amount;
    }
}

fn main() {
    let player = Player { level: 8 };
    return player.bonus(6);
}
"#,
    )
    .expect("compile heap script impl method dispatch");
    let mut budget = ExecutionBudget::unbounded();

    assert_eq!(
        Vm::new().run_program_with_managed_heap_and_budget(&program, "main", &[], &mut budget),
        Ok(Value::Int(14))
    );
    assert_eq!(budget.memory_bytes_allocated(), 0);
}

#[test]
fn managed_heap_execution_runs_trait_default_method_dispatch() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource {
    fn bonus(self, amount) -> int { return self.level + amount; }
    fn label(self) -> string { return self.name; }
}
struct Player { level: int, name: string }

impl BonusSource for Player {}

fn main() {
    let player = Player { level: 8, name: "hero" };
    return player.bonus(6) + player.label().len();
}
"#,
    )
    .expect("compile heap trait default method dispatch");
    let mut budget = ExecutionBudget::unbounded();

    assert_eq!(
        Vm::new().run_program_with_managed_heap_and_budget(&program, "main", &[], &mut budget),
        Ok(Value::Int(18))
    );
    assert_eq!(budget.memory_bytes_allocated(), 0);
}

#[test]
fn runs_compiled_const_expression_source() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
const BASE: int = 10;
const BONUS: int = BASE + 5 * 2;

fn main() {
    return BONUS;
}
"#,
        "main",
    )
    .expect("compile const expression source");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(20)));
}

#[test]
fn runs_compiled_native_call_source() {
    let mut vm = Vm::new();
    vm.register_native("log", |args| {
        assert_eq!(args, [Value::String("compiled".into())]);
        Ok(Value::Int(7))
    });

    let code = compile_function_source(
        SourceId::new(1),
        "fn main() { return log(\"compiled\"); }",
        "main",
    )
    .expect("compile native call source");

    assert_eq!(vm.run(&code), Ok(Value::Int(7)));
}

#[test]
fn heap_execution_materializes_native_args_and_stores_result() {
    let mut vm = Vm::new();
    vm.register_native("echo_label", |args| {
        assert_eq!(args, [Value::String("compiled".into())]);
        Ok(Value::String("native-result".into()))
    });
    let code = compile_function_source(
        SourceId::new(1),
        "fn main() { return echo_label(\"compiled\"); }",
        "main",
    )
    .expect("compile native call source");
    let mut heap = ScriptHeap::new();
    let mut heap_execution = HeapExecution::new(&mut heap);
    let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX, usize::MAX);

    let result = vm
        .run_with_heap_and_budget(&code, &mut heap_execution, &mut budget)
        .expect("run heap native call");

    let Value::HeapRef(result_ref) = result else {
        panic!("expected heap-backed native result");
    };
    assert_eq!(
        heap.get(result_ref),
        Some(&HeapValue::String("native-result".into()))
    );
}

#[test]
fn runs_compiled_script_function_calls() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn add_bonus(value) {
    return value + 5;
}

fn main() {
    let base = 10;
    return add_bonus(base) * 2;
}
"#,
    )
    .expect("compile program source");

    assert_eq!(
        Vm::new().run_program(&program, "main", &[]),
        Ok(Value::Int(30))
    );
}

#[test]
fn runs_compiled_named_args_and_parameter_defaults() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn grant(base, amount = 10, bonus = amount + 1) {
    return base + amount + bonus;
}

fn main() {
    return grant(bonus = 5, base = 1);
}
"#,
    )
    .expect("compile named args and parameter defaults");

    assert_eq!(
        Vm::new().run_program(&program, "main", &[]),
        Ok(Value::Int(16))
    );
}

#[test]
fn runs_entrypoint_parameter_defaults() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main(value = 7) {
    return value + 1;
}
"#,
        "main",
    )
    .expect("compile entrypoint default");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(8)));
}

#[test]
fn runs_compiled_lambdas_with_captures_after_outer_return() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn make_adder(base) {
    return |value| value + base;
}

fn main() {
    let add = make_adder(10);
    return add(5);
}
"#,
    )
    .expect("compile captured lambda");

    assert_eq!(
        Vm::new().run_program(&program, "main", &[]),
        Ok(Value::Int(15))
    );
}

#[test]
fn runs_compiled_nested_lambdas_with_transitive_captures() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn make_nested(base) {
    return |amount| {
        let scale = 2;
        return |bonus| base + amount * scale + bonus;
    };
}

fn main() {
    let make = make_nested(10);
    let add = make(4);
    return add(3);
}
"#,
    )
    .expect("compile nested captured lambda");

    assert_eq!(
        Vm::new().run_program(&program, "main", &[]),
        Ok(Value::Int(21))
    );
}

#[test]
fn runs_immediate_lambda_calls_and_block_returns() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let direct = (|value| value + 1)(4);
    let block = |value| { return value + direct; };
    return block(6);
}
"#,
        "main",
    )
    .expect("compile immediate lambda call");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(11)));
}

#[test]
fn runs_try_propagation_for_option_values() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
enum Option {
    Some(value),
    None,
}

fn maybe(value) {
    if value > 0 {
        return Option.Some(value);
    }
    return Option.None {};
}

fn present() {
    let value = maybe(4)?;
    return Option.Some(value + 1);
}

fn missing() {
    let value = maybe(0)?;
    return Option.Some(value + 1);
}
"#,
    )
    .expect("compile option propagation");

    assert_eq!(
        Vm::new().run_program(&program, "present", &[]),
        Ok(Value::Enum {
            enum_name: "Option".into(),
            variant: "Some".into(),
            fields: ScriptFields::from_pairs("Option.Some", [("0".into(), Value::Int(5))]),
        })
    );
    assert_eq!(
        Vm::new().run_program(&program, "missing", &[]),
        Ok(Value::Enum {
            enum_name: "Option".into(),
            variant: "None".into(),
            fields: ScriptFields::from_pairs("Option.None", BTreeMap::new()),
        })
    );
}

#[test]
fn managed_heap_execution_runs_try_propagation_for_result_values() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
enum Result {
    Ok(value),
    Err(message),
}

fn checked(value) {
    if value > 0 {
        return Result.Ok(value);
    }
    return Result.Err("bad");
}

fn ok_case() {
    let value = checked(3)?;
    return Result.Ok(value + 7);
}

fn err_case() {
    let value = checked(0)?;
    return Result.Ok(value + 7);
}
"#,
    )
    .expect("compile result propagation");
    let mut budget = ExecutionBudget::new(10_000, 4096, 64, 16);

    assert_eq!(
        Vm::new().run_program_with_managed_heap_and_budget(&program, "ok_case", &[], &mut budget),
        Ok(Value::Enum {
            enum_name: "Result".into(),
            variant: "Ok".into(),
            fields: ScriptFields::from_pairs("Result.Ok", [("0".into(), Value::Int(10))]),
        })
    );

    let mut budget = ExecutionBudget::new(10_000, 4096, 64, 16);
    assert_eq!(
        Vm::new().run_program_with_managed_heap_and_budget(&program, "err_case", &[], &mut budget),
        Ok(Value::Enum {
            enum_name: "Result".into(),
            variant: "Err".into(),
            fields: ScriptFields::from_pairs(
                "Result.Err",
                [("0".into(), Value::String("bad".into()))],
            ),
        })
    );
}

#[test]
fn managed_heap_execution_runs_string_parameter_defaults() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn choose(prefix = "quest", suffix = "done") {
    return prefix == "quest" && suffix == "done";
}

fn main() {
    return choose(suffix = "done");
}
"#,
    )
    .expect("compile heap parameter defaults");
    let mut budget = ExecutionBudget::new(10_000, 32_000, 32, 32);

    assert_eq!(
        Vm::new().run_program_with_managed_heap_and_budget(&program, "main", &[], &mut budget),
        Ok(Value::Bool(true))
    );
}

#[test]
fn runs_compiled_cross_module_imported_script_call() {
    let program = compile_module_sources(&[
        ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_dotted("game.main"),
            r#"
use game.reward.grant as give_reward

fn main() {
    return give_reward(4);
}
"#,
        ),
        ModuleSource::new(
            SourceId::new(2),
            ModulePath::from_dotted("game.reward"),
            r#"
pub fn grant(amount) {
    return amount + 1;
}
"#,
        ),
    ])
    .expect("compile imported cross-module script call");

    assert_eq!(
        Vm::new().run_program(&program, "game.main.main", &[]),
        Ok(Value::Int(5))
    );
}

#[test]
fn runs_compiled_same_named_cross_module_functions() {
    let program = compile_module_sources(&[
        ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_dotted("game.main"),
            r#"
use game.reward.main as reward_main

fn main() {
    return reward_main();
}
"#,
        ),
        ModuleSource::new(
            SourceId::new(2),
            ModulePath::from_dotted("game.reward"),
            r#"
pub fn main() {
    return 7;
}
"#,
        ),
    ])
    .expect("compile same-named cross-module functions");

    assert_eq!(
        Vm::new().run_program(&program, "game.main.main", &[]),
        Ok(Value::Int(7))
    );
}

#[test]
fn runs_compiled_cross_module_imported_const_expression() {
    let program = compile_module_sources(&[
        ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_dotted("game.main"),
            r#"
use game.tuning.BONUS as REWARD

fn main() {
    return REWARD + 1;
}
"#,
        ),
        ModuleSource::new(
            SourceId::new(2),
            ModulePath::from_dotted("game.tuning"),
            r#"
use game.base.BASE as START

pub const BONUS: int = START + 1;
"#,
        ),
        ModuleSource::new(
            SourceId::new(3),
            ModulePath::from_dotted("game.base"),
            r#"
pub const BASE: int = 4;
"#,
        ),
    ])
    .expect("compile imported cross-module const expression");

    assert_eq!(
        Vm::new().run_program(&program, "game.main.main", &[]),
        Ok(Value::Int(6))
    );
}

#[test]
fn runs_compiled_cross_module_imported_type_constructors() {
    let program = compile_module_sources(&[
        ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_dotted("game.main"),
            r#"
use game.reward.Reward as Prize
use game.damage.Damage as Hit

fn make_reward() {
    return Prize { count: 2 };
}

fn make_damage() {
    return Hit.Physical { amount: 7 };
}
"#,
        ),
        ModuleSource::new(
            SourceId::new(2),
            ModulePath::from_dotted("game.reward"),
            r#"
pub struct Reward { count: int }
"#,
        ),
        ModuleSource::new(
            SourceId::new(3),
            ModulePath::from_dotted("game.damage"),
            r#"
pub enum Damage { Physical { amount: int } }
"#,
        ),
    ])
    .expect("compile imported cross-module type constructors");
    let mut reward_fields = BTreeMap::new();
    reward_fields.insert("count".into(), Value::Int(2));
    let mut damage_fields = BTreeMap::new();
    damage_fields.insert("amount".into(), Value::Int(7));

    assert_eq!(
        Vm::new().run_program(&program, "game.main.make_reward", &[]),
        Ok(Value::Record {
            type_name: "game.reward.Reward".into(),
            fields: ScriptFields::from_pairs("game.reward.Reward", reward_fields),
        })
    );
    assert_eq!(
        Vm::new().run_program(&program, "game.main.make_damage", &[]),
        Ok(Value::Enum {
            enum_name: "game.damage.Damage".into(),
            variant: "Physical".into(),
            fields: ScriptFields::from_pairs("game.damage.Damage.Physical", damage_fields),
        })
    );
}

#[test]
fn runs_cross_module_imported_constructor_defaults() {
    let program = compile_module_sources(&[
        ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_dotted("game.main"),
            r#"
use game.reward.Reward as Prize

fn main() {
    let reward = Prize {};
    return reward.count + reward.item_id.len();
}
"#,
        ),
        ModuleSource::new(
            SourceId::new(2),
            ModulePath::from_dotted("game.reward"),
            r#"
pub const BASE_COUNT = 5

pub struct Reward {
    item_id: string = "gold",
    count: int = BASE_COUNT + 2,
}
"#,
        ),
    ])
    .expect("compile imported constructor defaults");

    assert_eq!(
        Vm::new().run_program(&program, "game.main.main", &[]),
        Ok(Value::Int(11))
    );
}

#[test]
fn runs_compiled_cross_module_imported_match_patterns() {
    let program = compile_module_sources(&[
        ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_dotted("game.main"),
            r#"
use game.damage.Damage as Hit

fn main() {
    let damage = Hit.Physical { amount: 7 };
    match damage {
        Hit.Magical { amount } => { return amount + 100; },
        Hit.Physical { amount } => { return amount; },
        _ => { return 0; },
    }
}
"#,
        ),
        ModuleSource::new(
            SourceId::new(2),
            ModulePath::from_dotted("game.damage"),
            r#"
pub enum Damage {
    Physical { amount: int },
    Magical { amount: int },
}
"#,
        ),
    ])
    .expect("compile imported cross-module match pattern");

    assert_eq!(
        Vm::new().run_program(&program, "game.main.main", &[]),
        Ok(Value::Int(7))
    );
}

#[test]
fn runs_compiled_cross_module_qualified_function_and_const_paths() {
    let program = compile_module_sources(&[
        ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_dotted("game.main"),
            r#"
fn main() {
    return game.reward.grant() + game.config.BONUS;
}
"#,
        ),
        ModuleSource::new(
            SourceId::new(2),
            ModulePath::from_dotted("game.reward"),
            r#"
pub fn grant() {
    return 4;
}
"#,
        ),
        ModuleSource::new(
            SourceId::new(3),
            ModulePath::from_dotted("game.config"),
            r#"
pub const BONUS: int = 5;
"#,
        ),
    ])
    .expect("compile qualified cross-module paths");

    assert_eq!(
        Vm::new().run_program(&program, "game.main.main", &[]),
        Ok(Value::Int(9))
    );
}

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
        .run_program_with_heap_and_budget(&program, "main", &[], &mut heap_execution, &mut budget)
        .expect("run nested heap source");

    let Value::HeapRef(result_ref) = result else {
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
    fields.insert("count".into(), Value::Int(2));
    fields.insert("item_id".into(), Value::String("gold".into()));

    let result = Vm::new()
        .run_program_with_managed_heap_and_budget(&program, "main", &[], &mut budget)
        .expect("run managed heap source");

    assert_eq!(
        result,
        Value::Record {
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
    vm.register_native("game.path", move |_| Ok(Value::PathProxy(proxy.clone())));
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn array_case() {
    let paths = [game.path()];
    return paths[0];
}

fn map_case() {
    let paths = {"level": game.path()};
    return paths["level"];
}
"#,
    )
    .expect("compile path proxy aggregate source");
    let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX, usize::MAX);

    assert_eq!(
        vm.run_program_with_managed_heap_and_budget(&program, "array_case", &[], &mut budget),
        Ok(Value::PathProxy(expected.clone()))
    );
    assert_eq!(
        vm.run_program_with_managed_heap_and_budget(&program, "map_case", &[], &mut budget),
        Ok(Value::PathProxy(expected))
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
                &[Value::HostRef(host_ref)],
                &mut host,
                &mut budget,
            )
            .expect("run managed host heap source")
    };

    assert_eq!(result, Value::String("gold".into()));
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
                &[Value::HostRef(host_ref)],
                &mut host,
                &mut budget,
            )
            .expect("run managed host map source")
    };

    let mut expected = BTreeMap::new();
    expected.insert("class".into(), HostValue::String("mage".into()));
    expected.insert("score".into(), HostValue::Int(3));
    assert_eq!(result, Value::Int(2));
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
                &[Value::HostRef(host_ref)],
                &mut host,
                &mut budget,
            )
            .expect("run managed host record source")
    };

    let mut expected_script_fields = BTreeMap::new();
    expected_script_fields.insert("count".into(), Value::Int(2));
    expected_script_fields.insert("item_id".into(), Value::String("gold".into()));
    let mut expected_host_fields = BTreeMap::new();
    expected_host_fields.insert("count".into(), HostValue::Int(2));
    expected_host_fields.insert("item_id".into(), HostValue::String("gold".into()));
    assert_eq!(
        result,
        Value::Record {
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
    player.level = Damage.Physical { amount: 7 };
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
                &[Value::HostRef(host_ref)],
                &mut host,
                &mut budget,
            )
            .expect("run managed host enum source")
    };

    let mut expected_script_fields = BTreeMap::new();
    expected_script_fields.insert("amount".into(), Value::Int(7));
    let mut expected_host_fields = BTreeMap::new();
    expected_host_fields.insert("amount".into(), HostValue::Int(7));
    assert_eq!(
        result,
        Value::Enum {
            enum_name: "Damage".into(),
            variant: "Physical".into(),
            fields: ScriptFields::from_pairs("Damage.Physical", expected_script_fields),
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
                &[Value::HostRef(host_ref), Value::HostRef(target_ref)],
                &mut host,
                &mut budget,
            )
            .expect("run managed host ref source")
    };

    assert_eq!(result, Value::HostRef(target_ref));
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

#[test]
fn passes_arguments_to_program_entry() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn double(value) {
    return value * 2;
}
"#,
    )
    .expect("compile program source");

    assert_eq!(
        Vm::new().run_program(&program, "double", &[Value::Int(9)]),
        Ok(Value::Int(18))
    );
}

#[test]
fn runs_compiled_array_literal_source() {
    let code = compile_function_source(
        SourceId::new(1),
        "fn main() { return [1, 2 + 3, \"gold\"]; }",
        "main",
    )
    .expect("compile array literal source");

    assert_eq!(
        Vm::new().run(&code),
        Ok(Value::Array(vec![
            Value::Int(1),
            Value::Int(5),
            Value::String("gold".into())
        ]))
    );
}

#[test]
fn heap_execution_allocates_array_and_string_literals() {
    let code = compile_function_source(
        SourceId::new(1),
        "fn main() { return [1, 2 + 3, \"gold\"]; }",
        "main",
    )
    .expect("compile array literal source");
    let mut heap = ScriptHeap::new();
    let mut heap_execution = HeapExecution::new(&mut heap);
    let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX, usize::MAX);

    let result = Vm::new()
        .run_with_heap_and_budget(&code, &mut heap_execution, &mut budget)
        .expect("run heap-backed array source");

    let Value::HeapRef(array_ref) = result else {
        panic!("expected heap array");
    };
    let Some(HeapValue::Array(values)) = heap.get(array_ref) else {
        panic!("expected heap array object");
    };
    assert_eq!(values[0], HeapSlot::Int(1));
    assert_eq!(values[1], HeapSlot::Int(5));
    let HeapSlot::Ref(string_ref) = values[2] else {
        panic!("expected heap string ref");
    };
    assert_eq!(
        heap.get(string_ref),
        Some(&HeapValue::String("gold".into()))
    );
    assert_eq!(budget.memory_bytes_allocated(), heap.allocated_bytes());
}

#[test]
fn runs_compiled_map_literal_source() {
    let code = compile_function_source(
        SourceId::new(1),
        "fn main() { return {\"level\": 2, exp: 10 + 5}; }",
        "main",
    )
    .expect("compile map literal source");
    let mut expected = BTreeMap::new();
    expected.insert("level".into(), Value::Int(2));
    expected.insert("exp".into(), Value::Int(15));

    assert_eq!(Vm::new().run(&code), Ok(Value::Map(expected)));
}

#[test]
fn runs_record_constructor_and_field_reads() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let level = 3;
    let player = Player { level, exp: 7 };
    return player.level + player.exp;
}
"#,
        "main",
    )
    .expect("compile record source");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(10)));
}

#[test]
fn heap_execution_reads_record_fields_from_heap_records() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let level = 3;
    let player = Player { level, exp: 7 };
    return player.level + player.exp;
}
"#,
        "main",
    )
    .expect("compile record source");
    let mut heap = ScriptHeap::new();
    let mut heap_execution = HeapExecution::new(&mut heap);
    let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX, usize::MAX);

    let result = Vm::new()
        .run_with_heap_and_budget(&code, &mut heap_execution, &mut budget)
        .expect("run heap-backed record source");

    assert_eq!(result, Value::Int(10));
    assert_eq!(heap.live_object_count(), 1);
}

#[test]
fn returns_first_class_record_values() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    return Reward { item_id: "gold", count: 2 };
}
"#,
        "main",
    )
    .expect("compile record source");
    let mut fields = BTreeMap::new();
    fields.insert("count".into(), Value::Int(2));
    fields.insert("item_id".into(), Value::String("gold".into()));

    assert_eq!(
        Vm::new().run(&code),
        Ok(Value::Record {
            type_name: "Reward".into(),
            fields: ScriptFields::from_pairs("Reward", fields),
        })
    );
}

#[test]
fn runs_schema_field_defaults_for_record_constructors() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
const BASE_COUNT = 2

struct Reward {
    item_id: string = "gold",
    count: int = BASE_COUNT + 3,
}

fn main() {
    let explicit = Reward { count: 7 };
    let default_count = Reward { item_id: "xp" };
    return explicit.item_id.len() + explicit.count + default_count.count;
}
"#,
    )
    .expect("compile defaulted record constructor");

    assert_eq!(
        Vm::new().run_program(&program, "main", &[]),
        Ok(Value::Int(16))
    );
}

#[test]
fn record_constructors_use_stable_slot_shapes() {
    let first = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    return Reward { count: 2, item_id: "gold" };
}
"#,
        "main",
    )
    .expect("compile first record source");
    let second = compile_function_source(
        SourceId::new(2),
        r#"
fn main() {
    return Reward { item_id: "gold", count: 2 };
}
"#,
        "main",
    )
    .expect("compile second record source");

    let Ok(Value::Record {
        fields: first_fields,
        ..
    }) = Vm::new().run(&first)
    else {
        panic!("first record");
    };
    let Ok(Value::Record {
        fields: second_fields,
        ..
    }) = Vm::new().run(&second)
    else {
        panic!("second record");
    };

    assert_eq!(first_fields.shape_id(), second_fields.shape_id());
    assert_eq!(
        first_fields
            .iter()
            .map(|(name, _)| name)
            .collect::<Vec<_>>(),
        ["count", "item_id"]
    );
}

#[test]
fn runs_compiled_immediate_slot_field_reads() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    return Reward { item_id: "gold", count: 2 }.count
        + Damage.Physical { amount: 7 }.amount;
}
"#,
        "main",
    )
    .expect("compile immediate slot field reads");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(9)));
}

#[test]
fn runs_compiled_typed_record_slot_field_reads() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
struct Reward {
    item_id: string,
    count: int,
}

fn make_reward() {
    return Reward { item_id: "gold", count: 2 };
}

fn main() {
    let reward: Reward = make_reward();
    return reward.count;
}
"#,
    )
    .expect("compile typed record slot field read");

    assert_eq!(
        Vm::new().run_program(&program, "main", &[]),
        Ok(Value::Int(2))
    );
}

#[test]
fn runs_compiled_typed_record_slot_field_writes() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
struct Reward {
    item_id: string,
    count: int,
}

fn make_reward() {
    return Reward { item_id: "gold", count: 2 };
}

fn main() {
    let reward: Reward = make_reward();
    reward.count += 3;
    reward.item_id = "xp";
    return reward.count + reward.item_id.len();
}
"#,
    )
    .expect("compile typed record slot field writes");

    assert_eq!(
        Vm::new().run_program(&program, "main", &[]),
        Ok(Value::Int(7))
    );
}

#[test]
fn runs_compiled_typed_enum_variant_slot_field_reads() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
enum Damage {
    Physical { amount: int, element: string },
    Magical { amount: int },
}

fn main() {
    let damage = Damage.Physical { amount: 7, element: "slash" };
    return damage.amount + damage.element.len();
}
"#,
    )
    .expect("compile typed enum variant slot field read");

    assert_eq!(
        Vm::new().run_program(&program, "main", &[]),
        Ok(Value::Int(12))
    );
}

#[test]
fn returns_first_class_enum_values() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    return Damage.Physical { amount: 7 };
}
"#,
        "main",
    )
    .expect("compile enum source");
    let mut fields = BTreeMap::new();
    fields.insert("amount".into(), Value::Int(7));

    assert_eq!(
        Vm::new().run(&code),
        Ok(Value::Enum {
            enum_name: "Damage".into(),
            variant: "Physical".into(),
            fields: ScriptFields::from_pairs("Damage.Physical", fields),
        })
    );
}

#[test]
fn runs_schema_field_defaults_for_enum_constructors() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
enum Damage {
    Physical { amount: int = 7, element: string = "slash" },
    Magical(amount: int = 3, element: string = "arcane"),
}

fn main() {
    let physical = Damage.Physical { amount: 5 };
    let magical = Damage.Magical();
    let physical_score = match physical {
        Damage.Physical { amount, element } => amount + element.len(),
        _ => 0,
    };
    let magical_score = match magical {
        Damage.Magical(amount, element) => amount + element.len(),
        _ => 0,
    };
    return physical_score + magical_score;
}
"#,
    )
    .expect("compile defaulted enum constructors");

    assert_eq!(
        Vm::new().run_program(&program, "main", &[]),
        Ok(Value::Int(19))
    );
}

#[test]
fn matches_enum_tag_and_binds_variant_fields() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let damage = Damage.Physical { amount: 7 };
    match damage {
        Damage.Magical { amount } => { return amount + 100; },
        Damage.Physical { amount } => { return amount + 1; },
        _ => { return 0; },
    }
}
"#,
        "main",
    )
    .expect("compile enum match source");

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(8)));
}

#[test]
fn heap_execution_matches_enum_tags_and_reads_fields() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let damage = Damage.Physical { amount: 7 };
    match damage {
        Damage.Magical { amount } => { return amount + 100; },
        Damage.Physical { amount } => { return amount + 1; },
        _ => { return 0; },
    }
}
"#,
        "main",
    )
    .expect("compile enum match source");
    let mut heap = ScriptHeap::new();
    let mut heap_execution = HeapExecution::new(&mut heap);
    let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX, usize::MAX);

    let result = Vm::new()
        .run_with_heap_and_budget(&code, &mut heap_execution, &mut budget)
        .expect("run heap-backed enum source");

    assert_eq!(result, Value::Int(8));
    assert_eq!(heap.live_object_count(), 1);
}

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
    let mut budget = ExecutionBudget::new(u64::MAX, 8, usize::MAX, usize::MAX);

    let error = Vm::new()
        .run_with_heap_and_budget(&code, &mut heap_execution, &mut budget)
        .expect_err("string allocation should exceed memory budget");

    assert_eq!(
        error.kind,
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

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(10)));
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

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(20)));
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

    assert_eq!(Vm::new().run(&code), Ok(Value::Int(1)));
}

#[test]
fn reads_host_field_through_patch_transaction() {
    let (program, host_ref) = host_read_program();
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    let result =
        Vm::new().run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host);

    assert_eq!(result, Ok(Value::Int(9)));
}

#[test]
fn set_host_field_records_patch_and_overlay_read() {
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
    let mut tx = PatchTx::new();

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };
        Vm::new().run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host)
    };

    assert_eq!(result, Ok(Value::Int(10)));
    assert_eq!(
        adapter.read_path(&level_path(host_ref)),
        Ok(HostValue::Int(9))
    );
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Int(10)));
    tx.apply(&mut adapter).expect("apply patches");
    assert_eq!(
        adapter.read_path(&level_path(host_ref)),
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
    let mut tx = PatchTx::new();
    let mut heap = ScriptHeap::new();
    let mut heap_execution = HeapExecution::new(&mut heap);
    let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX, usize::MAX);

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };
        Vm::new().run_program_with_host_heap_and_budget(
            &program,
            "main",
            &[Value::HostRef(host_ref)],
            &mut host,
            &mut heap_execution,
            &mut budget,
        )
    };

    assert!(matches!(result, Ok(Value::HeapRef(_))));
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(
        tx.patches()[0].op,
        PatchOp::Set(HostValue::String("gold".into()))
    );
}

#[test]
fn patch_budget_stops_host_writes_before_recording_overflow_patch() {
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
    let mut tx = PatchTx::new();
    let mut budget = ExecutionBudget::new(100, usize::MAX, usize::MAX, 1);

    let error = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };
        Vm::new()
            .run_program_with_host_and_budget(
                &program,
                "main",
                &[Value::HostRef(host_ref)],
                &mut host,
                &mut budget,
            )
            .expect_err("second patch exceeds budget")
    };

    assert_eq!(
        error.kind,
        VmErrorKind::BudgetExceeded {
            budget: ExecutionBudgetKind::Patches,
            limit: 1,
        }
    );
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Int(10)));
    assert_eq!(
        adapter.read_path(&level_path(host_ref)),
        Ok(HostValue::Int(9))
    );
}

#[test]
fn add_host_field_records_patch_and_overlay_read() {
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
    let mut tx = PatchTx::new();

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };
        Vm::new().run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host)
    };

    assert_eq!(result, Ok(Value::Int(10)));
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(tx.patches()[0].op, PatchOp::Add(HostValue::Int(1)));
    tx.apply(&mut adapter).expect("apply patches");
    assert_eq!(
        adapter.read_path(&level_path(host_ref)),
        Ok(HostValue::Int(10))
    );
}

#[test]
fn host_field_read_rejects_stale_generation() {
    let (program, _host_ref) = host_read_program();
    let fresh_ref = player_ref(3);
    let stale_ref = player_ref(2);
    let mut adapter = host_adapter(fresh_ref, HostValue::Int(9));
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    let error = Vm::new()
        .run_program_with_host(&program, "main", &[Value::HostRef(stale_ref)], &mut host)
        .expect_err("stale host read");

    assert_eq!(
        error.kind,
        VmErrorKind::Host(vela_host::HostErrorKind::StaleGeneration {
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
    adapter.deny_read(level_path(host_ref));
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    let error = Vm::new()
        .run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host)
        .expect_err("denied host read");

    assert_eq!(error.source_span, Some(span));
    assert_eq!(
        error.kind,
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

    assert_eq!(error.kind, VmErrorKind::DivisionByZero);
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
}

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
        Vm::new().run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host)
    };

    assert_eq!(result, Ok(Value::Int(11)));
    assert_eq!(
        adapter.read_path(&level_path(host_ref)),
        Ok(HostValue::Int(9))
    );
    assert_eq!(tx.patches().len(), 2);
    assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Int(10)));
    assert_eq!(tx.patches()[1].op, PatchOp::Add(HostValue::Int(1)));
    tx.apply(&mut adapter).expect("apply patches");
    assert_eq!(
        adapter.read_path(&level_path(host_ref)),
        Ok(HostValue::Int(11))
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
        Vm::new().run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host)
    };

    assert_eq!(result, Ok(Value::Int(11)));
    assert_eq!(adapter.read_path(&stats_level), Ok(HostValue::Int(9)));
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(tx.patches()[0].path, stats_level);
    assert_eq!(tx.patches()[0].op, PatchOp::Add(HostValue::Int(2)));
    tx.apply(&mut adapter).expect("apply nested host patch");
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
        Vm::new().run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host)
    };

    assert_eq!(result, Ok(Value::Int(7)));
    assert_eq!(adapter.read_path(&stats_level), Ok(HostValue::Int(9)));
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(tx.patches()[0].path, stats_level);
    assert_eq!(tx.patches()[0].op, PatchOp::Sub(HostValue::Int(2)));
    tx.apply(&mut adapter).expect("apply nested host sub patch");
    assert_eq!(adapter.read_path(&stats_level), Ok(HostValue::Int(7)));
}

#[test]
fn compiled_source_applies_host_numeric_compound_assignments_through_patch_tx() {
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
        Vm::new().run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host)
    };

    assert_eq!(result, Ok(Value::Int(1)));
    assert_eq!(adapter.read_path(&stats_level), Ok(HostValue::Int(4)));
    assert_eq!(tx.patches().len(), 3);
    assert_eq!(tx.patches()[0].op, PatchOp::Mul(HostValue::Int(3)));
    assert_eq!(tx.patches()[1].op, PatchOp::Div(HostValue::Int(2)));
    assert_eq!(tx.patches()[2].op, PatchOp::Rem(HostValue::Int(5)));
    tx.apply(&mut adapter)
        .expect("apply nested host numeric compound patches");
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

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };
        Vm::new().run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host)
    };

    assert_eq!(result, Ok(Value::Int(2)));
    assert_eq!(
        adapter.read_path(&reward_path),
        Ok(HostValue::Array(vec![HostValue::String("xp".into())]))
    );
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(tx.patches()[0].path, reward_path);
    assert_eq!(
        tx.patches()[0].op,
        PatchOp::Push(HostValue::String("gold".into()))
    );
    tx.apply(&mut adapter).expect("apply host push patch");
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
        Vm::new().run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host)
    };

    assert_eq!(result, Ok(Value::Int(1)));
    assert_eq!(
        adapter.read_path(&item_path),
        Ok(HostValue::String("gold".into()))
    );
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(tx.patches()[0].path, item_path);
    assert_eq!(tx.patches()[0].op, PatchOp::Remove);
    tx.apply(&mut adapter).expect("apply host remove patch");
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
        Vm::new().run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host)
    };

    assert_eq!(result, Ok(Value::Int(5)));
    assert_eq!(adapter.read_path(&item_count), Ok(HostValue::Int(4)));
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(tx.patches()[0].path, item_count);
    assert_eq!(tx.patches()[0].op, PatchOp::Add(HostValue::Int(1)));
    tx.apply(&mut adapter).expect("apply indexed host patch");
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
        Vm::new().run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host)
    };

    assert_eq!(result, Ok(Value::Int(5)));
    assert_eq!(adapter.read_path(&quest_count), Ok(HostValue::Int(4)));
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(tx.patches()[0].path, quest_count);
    assert_eq!(tx.patches()[0].op, PatchOp::Add(HostValue::Int(1)));
    tx.apply(&mut adapter)
        .expect("apply host variant field patch");
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
            &[Value::HostRef(ctx_ref)],
            &mut host,
            &mut budget,
        )
    };

    assert_eq!(result, Ok(Value::Int(1042)));
    assert!(adapter.method_calls().is_empty());
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
    tx.apply(&mut adapter).expect("apply context patches");
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
fn compiled_source_uses_reflection_natives_for_host_state() {
    let host_ref = player_ref(3);
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    let player_type = reflect.type_of(player);
    if reflect.name(player_type) == "Player" && reflect.kind(player_type) == "host" {
        if reflect.implements(player, "Damageable") {
            reflect.set(player, "level", 10);
            return reflect.get(player, "level");
        }
    }
    return 0;
}
"#,
    )
    .expect("compile reflection source");
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(reflection_registry()));

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };
        vm.run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host)
    };

    assert_eq!(result, Ok(Value::Int(10)));
    assert_eq!(
        adapter.read_path(&level_path(host_ref)),
        Ok(HostValue::Int(9))
    );
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Int(10)));
    tx.apply(&mut adapter).expect("apply reflection patch");
    assert_eq!(
        adapter.read_path(&level_path(host_ref)),
        Ok(HostValue::Int(10))
    );
}

#[test]
fn reflection_permissions_deny_writes_before_patches() {
    let host_ref = player_ref(3);
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    reflect.set(player, "level", 10);
    return 1;
}
"#,
    )
    .expect("compile denied reflection write source");
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives_with_permissions(
        Arc::new(reflection_registry()),
        reflect::ReflectPermissionSet::read_only(),
    );
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert!(matches!(
        vm.run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host),
        Err(error) if error.kind == VmErrorKind::Reflect(ReflectErrorKind::PermissionDenied {
            permission: reflect::ReflectPermission::WriteValueFields
        })
    ));
    assert!(tx.patches().is_empty());
}

#[test]
fn reflection_permissions_deny_calls_before_patches() {
    let host_ref = player_ref(3);
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    reflect.call(player, "grant_exp", 10);
    return 1;
}
"#,
    )
    .expect("compile denied reflection call source");
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    adapter.insert_method_return(HostMethodId::new(5), HostValue::Null);
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives_with_permissions(
        Arc::new(reflection_registry()),
        reflect::ReflectPermissionSet::read_only(),
    );
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert!(matches!(
        vm.run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host),
        Err(error) if error.kind == VmErrorKind::Reflect(ReflectErrorKind::PermissionDenied {
            permission: reflect::ReflectPermission::CallMethods
        })
    ));
    assert!(tx.patches().is_empty());
}

#[test]
fn reflection_permissions_deny_host_write_effect_calls_before_patches() {
    let host_ref = player_ref(3);
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    reflect.call(player, "grant_exp", 10);
    return 1;
}
"#,
    )
    .expect("compile denied reflection effect source");
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(100), "Player"))
            .host_type(HostTypeId::new(1))
            .method(
                MethodDesc::new(HostMethodId::new(5), "grant_exp")
                    .effects(MethodEffectSet::host_write())
                    .access(MethodAccess::new().reflect_callable(true)),
            ),
    );
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives_with_policy(
        Arc::new(registry),
        reflect::ReflectPolicy::new(
            reflect::ReflectPermissionSet::new()
                .with(reflect::ReflectPermission::CallMethods)
                .with(reflect::ReflectPermission::CallHostReadMethods)
                .with(reflect::ReflectPermission::InspectHostPath),
        ),
    );
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert!(matches!(
        vm.run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host),
        Err(error) if error.kind == VmErrorKind::Reflect(
            ReflectErrorKind::MethodEffectPermissionDenied {
                method: "grant_exp".to_owned(),
                permission: reflect::ReflectPermission::CallHostWriteMethods
            }
        )
    ));
    assert!(tx.patches().is_empty());
}

#[test]
fn reflection_permissions_deny_host_ref_metadata_without_inspection() {
    let host_ref = player_ref(3);
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    return reflect.type_of(player);
}
"#,
    )
    .expect("compile denied host-ref metadata source");
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives_with_permissions(
        Arc::new(reflection_registry()),
        reflect::ReflectPermissionSet::new().with(reflect::ReflectPermission::ReadTypeInfo),
    );
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert!(matches!(
        vm.run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host),
        Err(error) if error.kind == VmErrorKind::Reflect(ReflectErrorKind::PermissionDenied {
            permission: reflect::ReflectPermission::InspectHostPath
        })
    ));
    assert!(tx.patches().is_empty());
}

#[test]
fn reflection_permissions_allow_script_metadata_without_host_inspection() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
struct Player { level: int }

fn main() {
    let player = Player { level: 7 };
    return reflect.name(player);
}
"#,
    )
    .expect("compile script metadata source");
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives_with_permissions(
        Arc::new(script_reflection_registry()),
        reflect::ReflectPermissionSet::new().with(reflect::ReflectPermission::ReadTypeInfo),
    );
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        vm.run_program_with_host(&program, "main", &[], &mut host),
        Ok(Value::String("Player".into()))
    );
    assert!(tx.patches().is_empty());
}

#[test]
fn reflection_permissions_report_active_policy_metadata() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    if !reflect.has_permission("reflect.inspect_host_path") {
        return 0;
    }
    if reflect.has_permission("reflect.write_value_fields") {
        return 0;
    }
    return reflect.permissions();
}
"#,
    )
    .expect("compile reflection permission metadata source");
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives_with_permissions(
        Arc::new(TypeRegistry::new()),
        reflect::ReflectPermissionSet::read_only()
            .with(reflect::ReflectPermission::InspectHostPath),
    );
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        vm.run_program_with_host(&program, "main", &[], &mut host),
        Ok(Value::Array(vec![
            Value::String("reflect.read_type_info".to_owned()),
            Value::String("reflect.read_value_fields".to_owned()),
            Value::String("reflect.inspect_host_path".to_owned()),
        ]))
    );
    assert!(tx.patches().is_empty());
}

#[test]
fn reflection_permissions_report_unknown_permission_candidates() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    return reflect.has_permission("reflect.inspect_host");
}
"#,
    )
    .expect("compile reflection unknown permission source");
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives_with_permissions(
        Arc::new(TypeRegistry::new()),
        reflect::ReflectPermissionSet::read_only(),
    );
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    let error = vm
        .run_program_with_host(&program, "main", &[], &mut host)
        .expect_err("unknown permission should diagnose");
    assert_eq!(
        error.kind,
        VmErrorKind::Reflect(ReflectErrorKind::UnknownPermission {
            permission: "reflect.inspect_host".to_owned(),
            candidates: vec![
                "reflect.inspect_host_path".to_owned(),
                "reflect.call_methods".to_owned(),
                "reflect.access_private".to_owned()
            ]
        })
    );
    assert!(tx.patches().is_empty());
}

#[test]
fn reflection_permissions_deny_permission_metadata_without_type_read() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    return reflect.permissions();
}
"#,
    )
    .expect("compile denied reflection permission metadata source");
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives_with_permissions(
        Arc::new(TypeRegistry::new()),
        reflect::ReflectPermissionSet::new(),
    );
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert!(matches!(
        vm.run_program_with_host(&program, "main", &[], &mut host),
        Err(error) if error.kind == VmErrorKind::Reflect(ReflectErrorKind::PermissionDenied {
            permission: reflect::ReflectPermission::ReadTypeInfo
        })
    ));
    assert!(tx.patches().is_empty());
}

#[test]
fn reflection_permissions_deny_function_metadata_without_function_permission() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    reflect.function("game.admin");
    return 1;
}
"#,
    )
    .expect("compile function metadata permission source");
    let mut registry = TypeRegistry::new();
    registry.register_function(
        FunctionDesc::new(FunctionId::new(9), "game.admin")
            .access(FunctionAccess::new().require_permission("game.admin")),
    );
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives_with_policy(
        Arc::new(registry),
        reflect::ReflectPolicy::new(
            reflect::ReflectPermissionSet::new().with(reflect::ReflectPermission::ReadTypeInfo),
        ),
    );
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    let error = vm
        .run_program_with_host(&program, "main", &[], &mut host)
        .expect_err("function metadata permission should be denied");
    assert_eq!(
        error.kind,
        VmErrorKind::Reflect(ReflectErrorKind::FunctionPermissionDenied {
            function: "game.admin".to_owned(),
            permission: "game.admin".to_owned(),
        })
    );
    assert!(tx.patches().is_empty());
}

#[test]
fn reflection_field_access_denies_hidden_host_field_reads() {
    let host_ref = player_ref(3);
    let secret_field = FieldId::new(77);
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    return reflect.get(player, "secret");
}
"#,
    )
    .expect("compile hidden field reflection source");
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(100), "Player"))
            .host_type(HostTypeId::new(1))
            .field(
                FieldDesc::new(secret_field, "secret")
                    .access(FieldAccess::new().reflect_readable(false)),
            ),
    );
    let mut adapter = MockStateAdapter::new();
    adapter.insert_value(
        HostPath::new(host_ref).field(secret_field),
        HostValue::Int(99),
    );
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(registry));
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    let error = vm
        .run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host)
        .expect_err("hidden field read should be denied");
    assert_eq!(
        error.kind,
        VmErrorKind::Reflect(ReflectErrorKind::FieldNotReflectReadable {
            type_name: "Player".to_owned(),
            field: "secret".to_owned(),
        })
    );
    assert!(tx.patches().is_empty());
}

#[test]
fn reflection_field_permissions_deny_host_field_reads_before_patch() {
    let host_ref = player_ref(3);
    let title_field = FieldId::new(78);
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    return reflect.get(player, "title");
}
"#,
    )
    .expect("compile field permission reflection source");
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(100), "Player"))
            .host_type(HostTypeId::new(1))
            .field(
                FieldDesc::new(title_field, "title")
                    .access(FieldAccess::new().require_permission("player.title.inspect")),
            ),
    );
    let mut adapter = MockStateAdapter::new();
    adapter.insert_value(
        HostPath::new(host_ref).field(title_field),
        HostValue::String("Knight".to_owned()),
    );
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives_with_policy(Arc::new(registry), reflect::ReflectPolicy::all());
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    let error = vm
        .run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host)
        .expect_err("field permission should be denied");
    assert_eq!(
        error.kind,
        VmErrorKind::Reflect(ReflectErrorKind::FieldPermissionDenied {
            type_name: "Player".to_owned(),
            field: "title".to_owned(),
            permission: "player.title.inspect".to_owned(),
        })
    );
    assert!(tx.patches().is_empty());
}

#[test]
fn reflection_lookup_budget_stops_after_limit() {
    let host_ref = player_ref(3);
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    reflect.name(player);
    reflect.kind(player);
    return 1;
}
"#,
    )
    .expect("compile budgeted reflection source");
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives_with_policy(
        Arc::new(reflection_registry()),
        reflect::ReflectPolicy::all().with_lookup_limit(1),
    );
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert!(matches!(
        vm.run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host),
        Err(error) if error.kind == VmErrorKind::Reflect(ReflectErrorKind::LookupBudgetExceeded {
            limit: 1
        })
    ));
    assert!(tx.patches().is_empty());
}

#[test]
fn heap_execution_uses_reflection_natives_for_host_state() {
    let host_ref = player_ref(3);
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    let player_type = reflect.type_of(player);
    if reflect.name(player_type) == "Player" && reflect.kind(player_type) == "host" {
        if reflect.implements(player, "Damageable") {
            reflect.set(player, "level", 10);
            return reflect.get(player, "level");
        }
    }
    return 0;
}
"#,
    )
    .expect("compile reflection source");
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(reflection_registry()));
    let mut heap = ScriptHeap::new();
    let mut heap_execution = HeapExecution::new(&mut heap);
    let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX, usize::MAX);

    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };
        vm.run_program_with_host_heap_and_budget(
            &program,
            "main",
            &[Value::HostRef(host_ref)],
            &mut host,
            &mut heap_execution,
            &mut budget,
        )
    };

    assert_eq!(result, Ok(Value::Int(10)));
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Int(10)));
}

#[test]
fn compiled_source_reflection_fields_returns_metadata() {
    let host_ref = player_ref(3);
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    let fields = reflect.fields(player);
    return fields.len() == 2
        && fields[0].owner == "Player"
        && fields[0].name == "id"
        && fields[1].owner == "Player"
        && fields[1].name == "level"
        && reflect.kind(fields[1]) == "field";
}
"#,
    )
    .expect("compile reflection fields source");
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(reflection_registry()));
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    let result = vm.run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host);

    assert_eq!(result, Ok(Value::Bool(true)));
}

#[test]
fn compiled_source_reflects_name_kind_and_field_metadata() {
    let host_ref = player_ref(3);
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    let player_type = reflect.type_info("Player");
    let field = reflect.field(player, "level");
    let type_field = reflect.field(player_type, "level");
    let access = reflect.access(field);
    let all_fields = reflect.fields();
    if reflect.name(player) == "Player"
        && reflect.id(player) == 100
        && reflect.kind(player) == "host"
        && reflect.docs(player) == "A player host object."
        && option.unwrap_or(reflect.attrs(player).get("domain"), "") == "gameplay"
        && reflect.attr(player, "domain") == "gameplay"
        && reflect.has_attr(player, "domain")
        && reflect.attr(player, "missing") == null
        && !reflect.has_attr(player, "missing")
        && reflect.has_field(player, "level")
        && reflect.has_field(player_type, "level")
        && !reflect.has_field(player, "mana")
        && reflect.fields(player_type).len() == 2
        && all_fields.len() == 2
        && all_fields[1].owner == "Player"
        && all_fields[1].name == "level"
        && field.owner == "Player"
        && field.name == "level"
        && type_field.name == "level"
        && type_field.id == field.id
        && reflect.name(field) == "level"
        && reflect.owner(field) == "Player"
        && reflect.origin(field) == "host"
        && reflect.id(field) == 2
        && reflect.kind(field) == "field"
        && field.type == "int"
        && field.docs == "Current player level."
        && reflect.docs(field) == "Current player level."
        && reflect.source_span(field) == null
        && access.reflect_readable
        && access.reflect_writable
        && option.unwrap_or(field.attrs.get("unit"), "") == "level"
        && option.unwrap_or(reflect.attrs(field).get("unit"), "") == "level"
        && reflect.attr(field, "unit") == "level"
        && reflect.has_attr(field, "unit")
        && reflect.attr(field, "missing") == null
        && !reflect.has_attr(field, "missing")
        && field.writable {
        return 1;
    }
    return 0;
}
"#,
    )
    .expect("compile field reflection source");
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(reflection_registry()));
    vm.register_standard_natives();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        vm.run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host),
        Ok(Value::Int(1))
    );
}

#[test]
fn compiled_source_reflects_required_permissions_metadata() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    let function = reflect.function("game.reward.admin");
    let permissions = reflect.required_permissions(function);
    let access = reflect.access(function);
    let access_permissions = reflect.required_permissions(function.access);
    let direct_access = reflect.access(function.access);
    let public_function = reflect.function("game.reward.grant");
    return permissions.len() == 1
        && permissions[0] == "game.admin"
        && access.required_permissions[0] == "game.admin"
        && access_permissions.len() == 1
        && access_permissions[0] == "game.admin"
        && direct_access.required_permissions[0] == "game.admin"
        && reflect.required_permissions(public_function).is_empty();
}
"#,
    )
    .expect("compile required permission reflection source");
    let mut vm = Vm::new();
    vm.register_reflection_natives_with_policy(
        Arc::new(policy_module_reflection_registry()),
        reflect::ReflectPolicy::read_only().with_function_permission("game.admin"),
    );
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        vm.run_program_with_host(&program, "main", &[], &mut host),
        Ok(Value::Bool(true))
    );
    assert!(tx.patches().is_empty());
}

#[test]
fn compiled_source_reflects_effect_metadata_helper() {
    let host_ref = player_ref(3);
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    let method = reflect.method(player, "grant_exp");
    let effects = reflect.effects(method);
    let direct = reflect.effects(method.effects);
    return effects.writes_host
        && effects.reads_host
        && !effects.emits_events
        && direct.writes_host
        && direct.reads_host
        && reflect.kind(effects) == "effect_set";
}
"#,
    )
    .expect("compile reflected effect metadata source");
    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(reflection_registry()));
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        vm.run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host),
        Ok(Value::Bool(true))
    );
    assert!(tx.patches().is_empty());
}

#[test]
fn compiled_source_reflects_signature_metadata_helpers() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    let function = reflect.function("game.reward.grant");
    let params = reflect.params(function);
    let direct = reflect.params(function.params);
    return params.len() == 2
        && params[0].name == "player"
        && params[0].type == "Player"
        && params[1].name == "amount"
        && params[1].defaulted
        && direct[1].name == "amount"
        && reflect.returns(function) == "bool";
}
"#,
    )
    .expect("compile reflected signature metadata source");
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(script_module_reflection_registry()));
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        vm.run_program_with_host(&program, "main", &[], &mut host),
        Ok(Value::Bool(true))
    );
    assert!(tx.patches().is_empty());
}

#[test]
fn compiled_source_reflect_fields_respect_field_access() {
    let host_ref = player_ref(3);
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    let fields = reflect.fields(player);
    let all_fields = reflect.fields();
    if reflect.has_field(player, "level")
        && !reflect.has_field(player, "secret")
        && fields[0].owner == "Player"
        && fields[0].name == "level"
        && reflect.field(player, "level").name == "level" {
        return fields.len() * 10 + all_fields.len();
    }
    return 0;
}
"#,
    )
    .expect("compile policy fields reflection source");
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    let policy = reflect::ReflectPolicy::new(
        reflect::ReflectPermissionSet::new()
            .with(reflect::ReflectPermission::ReadTypeInfo)
            .with(reflect::ReflectPermission::InspectHostPath),
    );
    vm.register_reflection_natives_with_policy(
        Arc::new(policy_field_reflection_registry()),
        policy,
    );
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        vm.run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host),
        Ok(Value::Int(11))
    );
}

#[test]
fn compiled_source_reflects_methods_traits_and_variants() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(player) {
    let player_type = reflect.type_info("Player");
    let quest_type = reflect.type_info("QuestProgress");
    let methods = reflect.methods(player);
    let method = reflect.method(player, "grant_exp");
    let type_methods = reflect.methods(player_type);
    let type_method = reflect.method(player_type, "grant_exp");
    let all_methods = reflect.methods();
    let traits = reflect.traits(player);
    let type_traits = reflect.traits(player_type);
    let quest = QuestProgress.Active { count: 1 };
    let variants = reflect.variants(quest);
    let active = reflect.variant_info(quest, "Active");
    let active_fields = reflect.fields(quest);
    let active_count = reflect.field(quest, "count");
    let type_variants = reflect.variants(quest_type);
    let type_active = reflect.variant_info(quest_type, "Active");
    let all_variants = reflect.variants();
    if reflect.has_method(player, "grant_exp")
        && reflect.has_method(player_type, "grant_exp")
        && methods.len() == 1
        && type_methods.len() == 1
        && all_methods.len() == 1
        && all_methods[0].owner == "Player"
        && all_methods[0].name == "grant_exp"
        && methods[0].owner == "Player"
        && method.name == "grant_exp"
        && type_method.id == method.id
        && method.owner == "Player"
        && reflect.owner(method) == "Player"
        && reflect.origin(method) == "host"
        && method.attrs["effect"] == "write"
        && methods[0].returns == "bool"
        && methods[0].params[0].name == "amount"
        && methods[0].params[0].type == "int"
        && method.params[0].name == "amount"
        && traits.len() == 1
        && type_traits.len() == 1
        && variants.len() == 2
        && type_variants.len() == 2
        && variants[0].owner == "QuestProgress"
        && reflect.owner(variants[0]) == "QuestProgress"
        && active.name == "Active"
        && type_active.id == active.id
        && active.owner == "QuestProgress"
        && reflect.owner(active) == "QuestProgress"
        && reflect.origin(active) == "host"
        && active.fields[0].name == "count"
        && reflect.has_field(quest, "count")
        && !reflect.has_field(quest, "missing")
        && active_fields.len() == 1
        && active_fields[0].name == "count"
        && active_count.name == "count"
        && active_count.id == active.fields[0].id
        && reflect.has_variant(quest_type, "Active")
        && all_variants.len() == 2
        && all_variants[0].owner == "QuestProgress"
        && all_variants[0].name == "Active"
        && reflect.variant(quest) == "Active"
        && reflect.has_variant(quest, "Active")
        && !reflect.has_variant(quest, "Paused")
        && reflect.variant_is(quest, "Active") {
        return variants[0].fields.len();
    }
    return 0;
}
"#,
    )
    .expect("compile member reflection source");
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(member_reflection_registry()));
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        vm.run_program_with_host(
            &program,
            "main",
            &[Value::HostRef(player_ref(3))],
            &mut host
        ),
        Ok(Value::Int(1))
    );
}

#[test]
fn compiled_source_reflects_registered_trait_metadata() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    let traits = reflect.traits();
    let trait_info = reflect.trait_info("Damageable");
    if traits.len() == 1
        && reflect.has_trait("Damageable")
        && !reflect.has_trait("Damagable")
        && traits[0].name == "Damageable"
        && trait_info.name == "Damageable"
        && trait_info.methods[0].name == "damage"
        && trait_info.methods[0].owner == "Damageable"
        && reflect.origin(trait_info) == "host"
        && reflect.owner(trait_info.methods[0]) == "Damageable"
        && reflect.kind(trait_info.methods[0]) == "trait_method" {
        return trait_info.methods.len();
    }
    return 0;
}
"#,
    )
    .expect("compile trait metadata reflection source");
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(reflection_registry()));
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        vm.run_program_with_host(&program, "main", &[], &mut host),
        Ok(Value::Int(1))
    );
}

#[test]
fn compiled_source_reflects_registered_type_metadata() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    let types = reflect.types();
    let player = reflect.type_info("Player");
    if types.len() == 1
        && reflect.has_type("Player")
        && !reflect.has_type("Plyer")
        && types[0].name == "Player"
        && types[0].id == player.id
        && reflect.name(types[0]) == "Player"
        && reflect.kind(types[0]) == "host"
        && player.kind == "host"
        && reflect.kind(player) == "host"
        && player.origin == "host"
        && reflect.origin(player) == "host"
        && player.field_count == 2
        && player.method_count == 1
        && player.trait_count == 1 {
        return player.name;
    }
    return "missing";
}
"#,
    )
    .expect("compile type metadata reflection source");
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(reflection_registry()));
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        vm.run_program_with_host(&program, "main", &[], &mut host),
        Ok(Value::String("Player".to_owned()))
    );
}

#[test]
fn compiled_source_reflects_type_source_span_metadata() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    let player = reflect.type_info("Player");
    if player.source_span.source == 7
        && player.source_span.start == 20
        && player.source_span.end == 42 {
        return player.name;
    }
    return "missing";
}
"#,
    )
    .expect("compile type source span metadata source");
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(100), "Player"))
            .kind(TypeKind::ScriptStruct)
            .source_span(Span::new(SourceId::new(7), 20, 42)),
    );
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(registry));
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        vm.run_program_with_host(&program, "main", &[], &mut host),
        Ok(Value::String("Player".to_owned()))
    );
}

fn host_read_program() -> (Program, HostRef) {
    let host_ref = player_ref(3);
    let mut code = CodeObject::new("main", 2).with_params(vec!["player".into()]);
    code.push_instruction(Instruction::new(InstructionKind::GetHostField {
        dst: Register(1),
        root: Register(0),
        field: level_field(),
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(1),
    }));
    let mut program = Program::new();
    program.insert_function(code);
    (program, host_ref)
}

fn host_adapter(host_ref: HostRef, value: HostValue) -> MockStateAdapter {
    let mut adapter = MockStateAdapter::new();
    adapter.insert_value(level_path(host_ref), value);
    adapter
}

fn reflection_registry() -> TypeRegistry {
    let mut registry = TypeRegistry::new();
    registry.register_trait(
        TraitDesc::new("Damageable")
            .method(TraitMethodDesc::new(MethodId::new(1), "damage").defaulted(true)),
    );
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(100), "Player"))
            .host_type(HostTypeId::new(1))
            .docs("A player host object.")
            .attr("domain", "gameplay")
            .field(FieldDesc::new(FieldId::new(1), "id"))
            .field(
                FieldDesc::new(level_field(), "level")
                    .writable(true)
                    .type_hint("int")
                    .docs("Current player level.")
                    .attr("unit", "level"),
            )
            .method(
                MethodDesc::new(HostMethodId::new(5), "grant_exp")
                    .effects(MethodEffectSet::host_write())
                    .param(MethodParamDesc::new("amount").type_hint("int"))
                    .return_type("bool")
                    .docs("Grant experience.")
                    .attr("effect", "write"),
            )
            .trait_impl(TraitDesc::new("Damageable")),
    );
    registry
}

fn script_reflection_registry() -> TypeRegistry {
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(200), "Player"))
            .kind(TypeKind::ScriptStruct)
            .field(FieldDesc::new(FieldId::new(20), "level"))
            .trait_impl(TraitDesc::new("Damageable")),
    );
    registry
}

fn script_module_reflection_registry() -> TypeRegistry {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SourceId::new(1),
        ModulePath::from_dotted("game.reward"),
        r#"
#[doc("Grant reward.")]
#[event("reward")]
pub fn grant(player: Player, amount: int = 1) -> bool {
    return true;
}
"#,
    ));
    let mut registry = TypeRegistry::new();
    registry.register_script_modules(&graph);
    registry
}

fn policy_module_reflection_registry() -> TypeRegistry {
    let mut registry = TypeRegistry::new();
    registry.register_module(ModuleDesc::new("game.reward"));
    registry.register_function(
        FunctionDesc::new(FunctionId::new(1), "game.reward.grant").module("game.reward"),
    );
    registry.register_function(
        FunctionDesc::new(FunctionId::new(2), "game.reward.hidden")
            .module("game.reward")
            .access(FunctionAccess::new().reflect_visible(false)),
    );
    registry.register_function(
        FunctionDesc::new(FunctionId::new(3), "game.reward.private")
            .module("game.reward")
            .access(FunctionAccess::new().public(false).reflect_visible(true)),
    );
    registry.register_function(
        FunctionDesc::new(FunctionId::new(4), "game.reward.admin")
            .module("game.reward")
            .access(FunctionAccess::new().require_permission("game.admin")),
    );
    registry
}

fn policy_method_reflection_registry() -> TypeRegistry {
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(500), "Player"))
            .host_type(HostTypeId::new(1))
            .method(MethodDesc::new(HostMethodId::new(1), "visible"))
            .method(
                MethodDesc::new(HostMethodId::new(2), "hidden")
                    .access(MethodAccess::new().reflect_callable(false)),
            )
            .method(
                MethodDesc::new(HostMethodId::new(3), "private")
                    .access(MethodAccess::new().public(false).reflect_callable(true)),
            )
            .method(
                MethodDesc::new(HostMethodId::new(4), "admin")
                    .access(MethodAccess::new().require_permission("player.admin")),
            ),
    );
    registry
}

fn policy_field_reflection_registry() -> TypeRegistry {
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(600), "Player"))
            .host_type(HostTypeId::new(1))
            .field(FieldDesc::new(FieldId::new(1), "level"))
            .field(
                FieldDesc::new(FieldId::new(2), "secret")
                    .access(FieldAccess::new().reflect_readable(false)),
            ),
    );
    registry
}

fn member_reflection_registry() -> TypeRegistry {
    let mut registry = reflection_registry();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(300), "QuestProgress"))
            .kind(TypeKind::ScriptEnum)
            .variant(
                VariantDesc::new(VariantId::new(10), "Active")
                    .field(FieldDesc::new(FieldId::new(11), "count"))
                    .field(
                        FieldDesc::new(FieldId::new(13), "secret")
                            .access(FieldAccess::new().reflect_readable(false)),
                    ),
            )
            .variant(VariantDesc::new(VariantId::new(12), "Finished")),
    );
    registry
}

fn player_ref(generation: u32) -> HostRef {
    HostRef::new(HostTypeId::new(1), HostObjectId::new(7), generation)
}

fn level_path(host_ref: HostRef) -> HostPath {
    HostPath::new(host_ref).field(level_field())
}

fn level_field() -> FieldId {
    FieldId::new(2)
}
