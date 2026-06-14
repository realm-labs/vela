use super::*;
use crate::owned_value::OwnedValue;
use crate::value::Value as RuntimeValue;

fn run_records_program(
    program: &UnlinkedProgram,
    entry: &str,
    args: &[OwnedValue],
) -> VmResult<OwnedValue> {
    let mut budget = ExecutionBudget::unbounded();
    run_linked_test_program_with_budget(&Vm::new(), program, entry, args, &mut budget)
}

#[test]
fn passes_arguments_to_program_entry() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn double(OwnedValue) {
    return OwnedValue * 2;
}
"#,
    )
    .expect("compile program source");

    assert_eq!(
        run_records_program(
            &program,
            "double",
            &[OwnedValue::Scalar(vela_common::ScalarValue::I64(9))]
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(18)))
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
        run_linked_test_code(code),
        Ok(OwnedValue::Array(vec![
            OwnedValue::Scalar(vela_common::ScalarValue::I64(1)),
            OwnedValue::Scalar(vela_common::ScalarValue::I64(5)),
            OwnedValue::String("gold".into())
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
    let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX);

    let mut program = UnlinkedProgram::new();
    program.insert_function(code);
    let result = run_linked_test_program_runtime_with_heap_and_budget(
        &Vm::new(),
        &program,
        "main",
        &[],
        &mut heap_execution,
        &mut budget,
    )
    .expect("run heap-backed array source");

    let RuntimeValue::HeapRef(array_ref) = result else {
        panic!("expected heap array");
    };
    let Some(HeapValue::Array(values)) = heap.get(array_ref) else {
        panic!("expected heap array object");
    };
    assert_eq!(values[0], RuntimeValue::i64(1));
    assert_eq!(values[1], RuntimeValue::i64(5));
    let RuntimeValue::HeapRef(string_ref) = values[2] else {
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
    let expected = OwnedValue::map([
        ("exp", OwnedValue::Scalar(vela_common::ScalarValue::I64(15))),
        (
            "level",
            OwnedValue::Scalar(vela_common::ScalarValue::I64(2)),
        ),
    ]);

    assert_eq!(run_linked_test_code(code), Ok(expected));
}

#[test]
fn runs_record_constructor_and_field_reads() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
struct Player { level: i64, exp: i64 }

fn main() {
    let level = 3;
    let player = Player { level, exp: 7 };
    return player.level + player.exp;
}
"#,
    )
    .expect("compile record source");

    assert_eq!(
        run_records_program(&program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(10)))
    );
}

#[test]
fn record_semantic_equality_requires_partial_eq() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
struct Reward { code: String, amount: i64 }

fn main() {
    let left = Reward { code: "xp", amount: 10 };
    let right = Reward { code: "xp", amount: 10 };
    return left == right;
}
"#,
    )
    .expect("compile record equality source");

    let error = run_records_program(&program, "main", &[])
        .expect_err("record equality should require PartialEq");
    assert_eq!(
        error.kind(),
        VmErrorKind::TypeMismatch { operation: "equal" }
    );
    assert!(
        error.source_span.is_some(),
        "dynamic equality failure should carry the operator span"
    );
}

#[test]
fn record_identity_comparison_uses_reference_identity() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
struct Reward { code: String, amount: i64 }

fn main() {
    let left = Reward { code: "xp", amount: 10 };
    let alias = left;
    let right = Reward { code: "xp", amount: 10 };
    return left === alias && left !== right;
}
"#,
    )
    .expect("compile record identity source");

    assert_eq!(
        run_records_program(&program, "main", &[]),
        Ok(OwnedValue::Bool(true))
    );
}

#[test]
fn array_semantic_equality_is_not_implicit_structural_equality() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    return [1, 2] == [1, 2];
}
"#,
        "main",
    )
    .expect("compile array equality source");

    let error = run_linked_test_code(code).expect_err("array equality should require PartialEq");
    assert_eq!(
        error.kind(),
        VmErrorKind::TypeMismatch { operation: "equal" }
    );
    assert!(
        error.source_span.is_some(),
        "dynamic equality failure should carry the operator span"
    );
}

#[test]
fn heap_execution_reads_record_fields_from_heap_records() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
struct Player { level: i64, exp: i64 }

fn main() {
    let level = 3;
    let player = Player { level, exp: 7 };
    return player.level + player.exp;
}
"#,
    )
    .expect("compile record source");
    let mut heap = ScriptHeap::new();
    let mut heap_execution = HeapExecution::new(&mut heap);
    let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX);

    let result = run_linked_test_program_runtime_with_heap_and_budget(
        &Vm::new(),
        &program,
        "main",
        &[],
        &mut heap_execution,
        &mut budget,
    )
    .expect("run heap-backed record source");

    assert_eq!(
        result,
        OwnedValue::Scalar(vela_common::ScalarValue::I64(10))
    );
    assert_eq!(heap.live_object_count(), 1);
}

#[test]
fn linked_execution_reads_dynamic_record_fields_for_untyped_parameters() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
struct Reward { base: int, multiplier: int }

fn score_reward(reward) {
    return reward.base * reward.multiplier;
}

fn main() {
    let reward = Reward { base: 12, multiplier: 3 };
    return score_reward(reward) + 4;
}
"#,
    )
    .expect("compile dynamic record field source");

    assert_eq!(
        run_records_program(&program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(40)))
    );
}

#[test]
fn linked_execution_writes_dynamic_record_fields_for_untyped_parameters() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
struct Reward { count: int }

fn add_count(reward) {
    reward.count += 3;
    return reward.count;
}

fn main() {
    let reward = Reward { count: 2 };
    return add_count(reward);
}
"#,
    )
    .expect("compile dynamic record field write source");

    assert_eq!(
        run_records_program(&program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(5)))
    );
}

#[test]
fn linked_execution_reads_dynamic_enum_fields_for_untyped_parameters() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
enum RewardResult {
    Granted { amount: int },
}

fn read_amount(result) {
    return result.amount;
}

fn main() {
    let result = RewardResult::Granted { amount: 7 };
    return read_amount(result);
}
"#,
    )
    .expect("compile dynamic enum field source");

    assert_eq!(
        run_records_program(&program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(7)))
    );
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
    fields.insert(
        "count".into(),
        OwnedValue::Scalar(vela_common::ScalarValue::I64(2)),
    );
    fields.insert("item_id".into(), OwnedValue::String("gold".into()));

    assert_eq!(
        run_linked_test_code(code),
        Ok(OwnedValue::Record {
            type_name: "Reward".into(),
            fields: ScriptFields::from_pairs("Reward", fields),
        })
    );
}

#[test]
fn runs_schema_field_defaults_for_record_constructors() {
    let program = compile_standard_program_source(
        SourceId::new(1),
        r#"
const BASE_COUNT: i64 = 2

struct Reward {
    item_id: String = "gold",
    count: i64 = BASE_COUNT + 3,
}

fn main() {
    let explicit = Reward { count: 7 };
    let default_count = Reward { item_id: "xp" };
    if explicit.item_id == "gold" {
        return 4 + explicit.count + default_count.count;
    }
    return 0;
}
"#,
    )
    .expect("compile defaulted record constructor");

    assert_eq!(
        run_records_program(&program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(16)))
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

    let Ok(OwnedValue::Record {
        fields: first_fields,
        ..
    }) = run_linked_test_code(first)
    else {
        panic!("first record");
    };
    let Ok(OwnedValue::Record {
        fields: second_fields,
        ..
    }) = run_linked_test_code(second)
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
    let program = compile_program_source(
        SourceId::new(1),
        r#"
struct Reward { item_id: String, count: i64 }
enum Damage { Physical { amount: i64 } }

fn main() {
    return Reward { item_id: "gold", count: 2 }.count
        + Damage::Physical { amount: 7 }.amount;
}
"#,
    )
    .expect("compile immediate slot field reads");

    assert_eq!(
        run_records_program(&program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(9)))
    );
}

#[test]
fn runs_compiled_typed_record_slot_field_reads() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
struct Reward {
    item_id: String,
    count: i64,
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
        run_records_program(&program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );
}

#[test]
fn runs_compiled_typed_record_slot_field_writes() {
    let program = compile_standard_program_source(
        SourceId::new(1),
        r#"
struct Reward {
    item_id: String,
    count: i64,
}

fn make_reward() {
    return Reward { item_id: "gold", count: 2 };
}

fn main() {
    let reward: Reward = make_reward();
    reward.count += 3;
    reward.item_id = "xp";
    if reward.item_id == "xp" {
        return reward.count + 2;
    }
    return 0;
}
"#,
    )
    .expect("compile typed record slot field writes");

    assert_eq!(
        run_records_program(&program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(7)))
    );
}

#[test]
fn runs_compiled_typed_enum_variant_slot_field_reads() {
    let program = compile_standard_program_source(
        SourceId::new(1),
        r#"
enum Damage {
    Physical { amount: i64, element: String },
    Magical { amount: i64 },
}

fn main() {
    let damage = Damage::Physical { amount: 7, element: "slash" };
    if damage.element == "slash" {
        return damage.amount + 5;
    }
    return 0;
}
"#,
    )
    .expect("compile typed enum variant slot field read");

    assert_eq!(
        run_records_program(&program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(12)))
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
    fields.insert(
        "amount".into(),
        OwnedValue::Scalar(vela_common::ScalarValue::I64(7)),
    );

    assert_eq!(
        run_linked_test_code(code),
        Ok(OwnedValue::Enum {
            enum_name: "Damage".into(),
            variant: "Physical".into(),
            fields: ScriptFields::from_pairs("Damage::Physical", fields),
        })
    );
}

#[test]
fn runs_schema_field_defaults_for_enum_constructors() {
    let program = compile_standard_program_source(
        SourceId::new(1),
        r#"
enum Damage {
    Physical { amount: i64 = 7, element: String = "slash" },
    Magical(amount: i64 = 3, element: String = "arcane"),
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
        run_records_program(&program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(19)))
    );
}

#[test]
fn matches_enum_tag_and_binds_variant_fields() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
enum Damage {
    Physical { amount: i64 },
    Magical { amount: i64 },
}

fn main() {
    let damage = Damage::Physical { amount: 7 };
    match damage {
        Damage::Magical { amount } => { return amount + 100; },
        Damage::Physical { amount } => { return amount + 1; },
        _ => { return 0; },
    }
}
"#,
    )
    .expect("compile enum match source");

    assert_eq!(
        run_records_program(&program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(8)))
    );
}

#[test]
fn heap_execution_matches_enum_tags_and_reads_fields() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
enum Damage {
    Physical { amount: i64 },
    Magical { amount: i64 },
}

fn main() {
    let damage = Damage::Physical { amount: 7 };
    match damage {
        Damage::Magical { amount } => { return amount + 100; },
        Damage::Physical { amount } => { return amount + 1; },
        _ => { return 0; },
    }
}
"#,
    )
    .expect("compile enum match source");
    let mut heap = ScriptHeap::new();
    let mut heap_execution = HeapExecution::new(&mut heap);
    let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX);

    let result = run_linked_test_program_runtime_with_heap_and_budget(
        &Vm::new(),
        &program,
        "main",
        &[],
        &mut heap_execution,
        &mut budget,
    )
    .expect("run heap-backed enum source");

    assert_eq!(result, OwnedValue::Scalar(vela_common::ScalarValue::I64(8)));
    assert_eq!(heap.live_object_count(), 1);
}
