use std::sync::{Arc, Mutex};

use super::*;

fn run_marked(source: &str) -> (VmResult<OwnedValue>, Vec<i64>) {
    let program =
        compile_standard_program_source_with_native_functions(SourceId::new(1), source, &["mark"])
            .expect("evaluation-order fixture should compile");
    let observed = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&observed);
    let mut vm = Vm::new();
    vm.register_native("mark", move |args| {
        let [OwnedValue::Scalar(vela_common::ScalarValue::I64(value))] = args else {
            panic!("mark expects one i64 argument, got {args:?}");
        };
        recorded.lock().expect("mark trace lock").push(*value);
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(*value)))
    });
    let mut budget = ExecutionBudget::unbounded();
    let result = run_linked_test_program_with_budget(&vm, &program, "main", &[], &mut budget);
    let trace = observed.lock().expect("evaluation trace lock").clone();
    (result, trace)
}

#[test]
fn named_script_arguments_evaluate_in_source_order_before_slot_projection() {
    let (result, trace) = run_marked(
        r#"
fn combine(first, second) { return first * 10 + second; }
fn main() { return combine(second = mark(2), first = mark(1)); }
"#,
    );

    assert_eq!(
        result,
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(12)))
    );
    assert_eq!(trace, [2, 1]);
}

#[test]
fn positional_nested_script_calls_evaluate_left_to_right() {
    let (result, trace) = run_marked(
        r#"
fn combine(first, second) { return first * 10 + second; }
fn main() { return combine(mark(1), combine(mark(2), mark(3))); }
"#,
    );

    assert_eq!(
        result,
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(33)))
    );
    assert_eq!(trace, [1, 2, 3]);
}

#[test]
fn logical_short_circuit_skips_side_effecting_rhs_operands() {
    let (result, trace) = run_marked(
        r#"
fn main() {
    return (false && mark(1)) || (true || mark(2));
}
"#,
    );

    assert_eq!(result, Ok(OwnedValue::Bool(true)));
    assert!(trace.is_empty(), "short-circuited markers ran: {trace:?}");
}

#[test]
fn named_tuple_constructor_fields_evaluate_in_source_order_before_projection() {
    let (result, trace) = run_marked(
        r#"
enum Pair { Values(first: i64, second: i64) }
fn main() {
    let pair = Pair::Values(second = mark(2), first = mark(1));
    return match pair { Pair::Values(first, second) => first * 10 + second };
}
"#,
    );

    assert_eq!(
        result,
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(12)))
    );
    assert_eq!(trace, [2, 1]);
}

#[test]
fn runtime_map_literals_use_hir_owned_char_numeric_and_path_keys() {
    let program = compile_standard_program_source(
        SourceId::new(1),
        r#"
enum RewardKey { Small }
fn main() {
    let values = { 'x': 1, 0x10u8: 2, 3.5f32: 4, RewardKey::Small: 8 };
    return values["x"]
        + values["0x10u8"]
        + values["3.5f32"]
        + values["RewardKey::Small"];
}
"#,
    )
    .expect("logical map keys should compile from HIR");
    let mut budget = ExecutionBudget::unbounded();

    assert_eq!(
        run_linked_test_program_with_budget(&Vm::new(), &program, "main", &[], &mut budget),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(15)))
    );
}
