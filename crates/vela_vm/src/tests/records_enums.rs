use super::*;

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
    assert_eq!(values[0], Value::Int(1));
    assert_eq!(values[1], Value::Int(5));
    let Value::HeapRef(string_ref) = values[2] else {
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
        + Damage::Physical { amount: 7 }.amount;
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
    let damage = Damage::Physical { amount: 7, element: "slash" };
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
    return Damage::Physical { amount: 7 };
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
            fields: ScriptFields::from_pairs("Damage::Physical", fields),
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
    let physical = Damage::Physical { amount: 5 };
    let magical = Damage::Magical();
    let physical_score = match physical {
        Damage::Physical { amount, element } => amount + element.len(),
        _ => 0,
    };
    let magical_score = match magical {
        Damage::Magical(amount, element) => amount + element.len(),
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
    let damage = Damage::Physical { amount: 7 };
    match damage {
        Damage::Magical { amount } => { return amount + 100; },
        Damage::Physical { amount } => { return amount + 1; },
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
    let damage = Damage::Physical { amount: 7 };
    match damage {
        Damage::Magical { amount } => { return amount + 100; },
        Damage::Physical { amount } => { return amount + 1; },
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
