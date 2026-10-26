use super::*;

const BUDGET_CEILING: u64 = 100_000;

fn compile_fixture(source: &str) -> vela_bytecode::compiler::CompiledProgram {
    compile_standard_program_source(SourceId::new(1), source)
        .expect("runtime semantics fixture should compile")
}

fn standard_vm() -> Vm {
    let mut vm = Vm::new();
    vm.register_standard_natives();
    vm
}

fn run_fixture(program: &UnlinkedProgram, entry: &str) -> VmResult<OwnedValue> {
    let mut budget = ExecutionBudget::new(BUDGET_CEILING, usize::MAX, usize::MAX);
    run_linked_test_program_with_budget(&standard_vm(), program, entry, &[], &mut budget)
}

fn assert_execution_unit_edge(
    program: &UnlinkedProgram,
    entry: &str,
    args: &[OwnedValue],
    exact_limit: u64,
    expected: OwnedValue,
) {
    let vm = standard_vm();
    let mut exact = ExecutionBudget::new(exact_limit, usize::MAX, usize::MAX);
    assert_eq!(
        run_linked_test_program_with_budget(&vm, program, entry, args, &mut exact),
        Ok(expected)
    );
    assert_eq!(exact.execution_units_consumed(), exact_limit);

    let failing_limit = exact_limit
        .checked_sub(1)
        .expect("execution-unit edge must include at least one charge");
    let mut short = ExecutionBudget::new(failing_limit, usize::MAX, usize::MAX);
    let error = run_linked_test_program_with_budget(&vm, program, entry, args, &mut short)
        .expect_err("one fewer execution unit must exhaust the budget");
    assert_eq!(
        error.kind(),
        VmErrorKind::BudgetExceeded {
            budget: ExecutionBudgetKind::ExecutionUnits,
            limit: failing_limit,
        }
    );
    assert_eq!(short.execution_units_consumed(), failing_limit);
}

#[test]
fn indexed_assignment_evaluates_target_then_value_once() {
    let program = compile_fixture(include_str!(
        "../../tests/fixtures/execution/assignment_order.vela"
    ));

    assert_eq!(
        run_fixture(&program, "main"),
        Ok(OwnedValue::Array(vec![
            OwnedValue::i64(7),
            OwnedValue::i64(3),
            OwnedValue::i64(1),
            OwnedValue::i64(2),
            OwnedValue::i64(3),
        ]))
    );
}

#[test]
fn indexed_compound_assignment_observes_aliasing_value_write() {
    let program = compile_fixture(include_str!(
        "../../tests/fixtures/execution/compound_assignment_order.vela"
    ));

    assert_eq!(
        run_fixture(&program, "main"),
        Ok(OwnedValue::Array(vec![
            OwnedValue::i64(105),
            OwnedValue::i64(3),
            OwnedValue::i64(1),
            OwnedValue::i64(2),
            OwnedValue::i64(3),
        ]))
    );
}

#[test]
fn assignment_target_components_survive_rhs_local_reassignment() {
    let program = compile_fixture(include_str!(
        "../../tests/fixtures/execution/assignment_target_reassignment.vela"
    ));

    assert_eq!(
        run_fixture(&program, "main"),
        Ok(OwnedValue::Array(vec![
            OwnedValue::i64(15),
            OwnedValue::i64(20),
            OwnedValue::i64(100),
            OwnedValue::i64(200),
            OwnedValue::i64(37),
            OwnedValue::i64(300),
        ]))
    );
}

#[test]
fn non_fusible_i64_immediate_binary_evaluates_effectful_lhs_once() {
    let program = compile_fixture(
        r#"
fn effectful(trace: Array<i64>) -> i64 {
    trace.push(1);
    return 8;
}

fn main() {
    let trace = [];
    let result = effectful(trace) / 2;
    return [result, trace.len()];
}
"#,
    );

    assert_eq!(
        run_fixture(&program, "main"),
        Ok(OwnedValue::Array(vec![
            OwnedValue::i64(4),
            OwnedValue::i64(1),
        ]))
    );
}

#[test]
fn loop_execution_unit_limit_has_a_stable_edge() {
    let program = compile_fixture(
        r#"
fn main() {
    let total = 0;
    for value in [1, 2, 3, 4] {
        total += value;
    }
    return total;
}
"#,
    );

    assert_execution_unit_edge(&program, "main", &[], 11, OwnedValue::i64(10));
}

#[test]
fn script_call_execution_unit_limit_has_a_stable_edge() {
    let program = compile_fixture(
        r#"
fn add_one(value) {
    return value + 1;
}

fn main() {
    return add_one(4);
}
"#,
    );

    assert_execution_unit_edge(&program, "main", &[], 1, OwnedValue::i64(5));
}

#[test]
fn allocation_execution_unit_limit_has_a_stable_edge() {
    let program = compile_fixture("fn main() { return [1, 2, 3]; }");
    assert_execution_unit_edge(
        &program,
        "main",
        &[],
        1,
        OwnedValue::Array(vec![
            OwnedValue::i64(1),
            OwnedValue::i64(2),
            OwnedValue::i64(3),
        ]),
    );
}

#[test]
fn callback_execution_unit_limit_has_a_stable_edge() {
    let program = compile_fixture("fn main() { return [1, 2].map(|value| value + 1); }");
    assert_execution_unit_edge(
        &program,
        "main",
        &[],
        11,
        OwnedValue::Array(vec![OwnedValue::i64(2), OwnedValue::i64(3)]),
    );
}

#[test]
fn container_guard_scan_execution_unit_limit_has_a_stable_edge() {
    let program = compile_test_program(
        SourceId::new(1),
        r#"
fn main(values: Array<i64>) -> i64 {
    return 1;
}
"#,
    )
    .expect("container guard fixture should compile");
    let values = OwnedValue::Array(vec![
        OwnedValue::i64(2),
        OwnedValue::String("bad".to_owned()),
    ]);

    let vm = Vm::new();
    let mut short = ExecutionBudget::new(1, usize::MAX, usize::MAX);
    let error = run_linked_test_program_with_budget(
        &vm,
        &program,
        "main",
        std::slice::from_ref(&values),
        &mut short,
    )
    .expect_err("budget must stop the guard scan before the mismatched element");
    assert_eq!(
        error.kind(),
        VmErrorKind::BudgetExceeded {
            budget: ExecutionBudgetKind::ExecutionUnits,
            limit: 1,
        }
    );
    assert_eq!(short.execution_units_consumed(), 1);

    let mut exact = ExecutionBudget::new(2, usize::MAX, usize::MAX);
    let error = run_linked_test_program_with_budget(&vm, &program, "main", &[values], &mut exact)
        .expect_err("the complete guard scan must report the element mismatch");
    assert_eq!(
        error.kind(),
        VmErrorKind::TypeContractViolation {
            expected: "i64".to_owned(),
            actual: "String".to_owned(),
            debug_name: "values".to_owned(),
        }
    );
    assert_eq!(exact.execution_units_consumed(), 2);
}

#[test]
fn try_propagation_execution_unit_limit_has_stable_edges() {
    let program = compile_fixture(
        r#"
enum Option {
    Some(value),
    None,
}

fn pass(value: Option<i64>) -> Option<i64> {
    let inner = value?;
    return Option::Some(inner + 1);
}

fn present() {
    return pass(Option::Some(4));
}

fn absent() {
    return pass(Option::None);
}
"#,
    );

    assert_execution_unit_edge(
        &program,
        "present",
        &[],
        3,
        OwnedValue::enum_variant("Option", "Some", [("0", OwnedValue::i64(5))]),
    );
    assert_execution_unit_edge(
        &program,
        "absent",
        &[],
        2,
        OwnedValue::enum_variant("Option", "None", Vec::<(&str, OwnedValue)>::new()),
    );
}
