use super::*;

const BUDGET_CEILING: u64 = 100_000;

fn compile_fixture(source: &str) -> UnlinkedProgram {
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

fn assert_instruction_edge(
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
    assert_eq!(exact.instructions_executed(), exact_limit);

    let failing_limit = exact_limit
        .checked_sub(1)
        .expect("instruction edge must include at least one charge");
    let mut short = ExecutionBudget::new(failing_limit, usize::MAX, usize::MAX);
    let error = run_linked_test_program_with_budget(&vm, program, entry, args, &mut short)
        .expect_err("one fewer instruction must exhaust the budget");
    assert_eq!(
        error.kind(),
        VmErrorKind::BudgetExceeded {
            budget: ExecutionBudgetKind::Instructions,
            limit: failing_limit,
        }
    );
    assert_eq!(short.instructions_executed(), failing_limit);
}

#[test]
fn indexed_assignment_evaluates_target_then_value_once() {
    let program = compile_fixture(
        r#"
fn receiver(values, trace) {
    trace.push(1);
    return values;
}

fn index(trace) {
    trace.push(2);
    return 0;
}

fn value(trace) {
    trace.push(3);
    return 7;
}

fn main() {
    let values = [10];
    let trace = [];
    receiver(values, trace)[index(trace)] = value(trace);
    return [values[0], trace.len(), trace[0], trace[1], trace[2]];
}
"#,
    );

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
    let program = compile_fixture(
        r#"
fn receiver(values, trace) {
    trace.push(1);
    return values;
}

fn index(trace) {
    trace.push(2);
    return 0;
}

fn aliasing_value(values, trace) {
    trace.push(3);
    values[0] = 100;
    return 5;
}

fn main() {
    let values = [10];
    let trace = [];
    receiver(values, trace)[index(trace)] += aliasing_value(values, trace);
    return [values[0], trace.len(), trace[0], trace[1], trace[2]];
}
"#,
    );

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
    let program = compile_fixture(
        r#"
struct Box { value: i64 }

fn main() {
    let original_values = [10, 20];
    let replacement_values = [100, 200];
    let values = original_values;
    let index = 0;
    values[index] += if true {
        values = replacement_values;
        index = 1;
        5
    } else {
        0
    };

    let original_box = Box { value: 30 };
    let replacement_box = Box { value: 300 };
    let target = original_box;
    target.value += if true {
        target = replacement_box;
        7
    } else {
        0
    };

    return [
        original_values[0],
        original_values[1],
        replacement_values[0],
        replacement_values[1],
        original_box.value,
        replacement_box.value,
    ];
}
"#,
    );

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
fn loop_instruction_limit_has_a_stable_edge() {
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

    assert_instruction_edge(&program, "main", &[], 25, OwnedValue::i64(10));
}

#[test]
fn script_call_instruction_limit_has_a_stable_edge() {
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

    assert_instruction_edge(&program, "main", &[], 5, OwnedValue::i64(5));
}

#[test]
fn container_guard_scan_instruction_limit_has_a_stable_edge() {
    let program = compile_program_source(
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
            budget: ExecutionBudgetKind::Instructions,
            limit: 1,
        }
    );
    assert_eq!(short.instructions_executed(), 1);

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
    assert_eq!(exact.instructions_executed(), 2);
}

#[test]
fn try_propagation_instruction_limit_has_stable_edges() {
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
    return pass(Option::None {});
}
"#,
    );

    assert_instruction_edge(
        &program,
        "present",
        &[],
        8,
        OwnedValue::enum_variant("Option", "Some", [("0", OwnedValue::i64(5))]),
    );
    assert_instruction_edge(
        &program,
        "absent",
        &[],
        4,
        OwnedValue::enum_variant("Option", "None", Vec::<(&str, OwnedValue)>::new()),
    );
}

fn run_host_compound_fixture(
    program: &UnlinkedProgram,
    host_ref: HostRef,
    initial: i64,
    instruction_limit: u64,
) -> (VmResult<OwnedValue>, MockStateAdapter, ExecutionBudget) {
    let mut adapter = host_adapter(
        host_ref,
        HostValue::Scalar(vela_common::ScalarValue::I64(initial)),
    );
    let mut access = HostAccess::new();
    let mut budget = ExecutionBudget::new(instruction_limit, usize::MAX, usize::MAX);
    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            access: &mut access,
            script_globals: None,
        };
        run_linked_test_program_with_host_budget(
            &Vm::new(),
            program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host,
            &mut budget,
        )
    };
    (result, adapter, budget)
}

#[test]
fn host_compound_write_instruction_limit_has_a_stable_edge() {
    let host_ref = player_ref(1);
    let program = compile_host_program_source(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    player.level += 1;
    return player.level;
}
"#,
        host_definition_registry(
            &[("Player", host_ref.type_id)],
            &[TestHostField::new("Player", "level", level_field()).type_hint("i64")],
            &[],
        ),
    )
    .expect("host boundary fixture should compile");

    let exact_limit = 6;
    let (result, adapter, budget) = run_host_compound_fixture(&program, host_ref, 10, exact_limit);
    assert_eq!(result, Ok(OwnedValue::i64(11)));
    assert_eq!(budget.instructions_executed(), exact_limit);
    assert_eq!(
        adapter.read_diagnostic_path(&level_path(host_ref)),
        Ok(HostValue::Scalar(vela_common::ScalarValue::I64(11)))
    );

    let failing_limit = 2;
    let (result, adapter, budget) =
        run_host_compound_fixture(&program, host_ref, 10, failing_limit);
    assert_eq!(
        result
            .expect_err("budget must stop execution before the host mutation")
            .kind(),
        VmErrorKind::BudgetExceeded {
            budget: ExecutionBudgetKind::Instructions,
            limit: failing_limit,
        }
    );
    assert_eq!(budget.instructions_executed(), failing_limit);
    assert_eq!(
        adapter.read_diagnostic_path(&level_path(host_ref)),
        Ok(HostValue::Scalar(vela_common::ScalarValue::I64(10)))
    );

    let (result, adapter, budget) = run_host_compound_fixture(&program, host_ref, 10, 3);
    assert_eq!(
        result
            .expect_err("budget must stop execution after the completed host mutation")
            .kind(),
        VmErrorKind::BudgetExceeded {
            budget: ExecutionBudgetKind::Instructions,
            limit: 3,
        }
    );
    assert_eq!(budget.instructions_executed(), 3);
    assert_eq!(
        adapter.read_diagnostic_path(&level_path(host_ref)),
        Ok(HostValue::Scalar(vela_common::ScalarValue::I64(11)))
    );
}

#[test]
fn host_assignment_root_survives_rhs_local_reassignment() {
    let original = HostRef::new(HostTypeId::new(1), HostObjectId::new(7), 1);
    let replacement = HostRef::new(HostTypeId::new(1), HostObjectId::new(8), 1);
    let program = compile_host_program_source(
        SourceId::new(1),
        r#"
fn main(player: Player, replacement: Player) {
    player.level += if true {
        player = replacement;
        1
    } else {
        0
    };
    return 0;
}
"#,
        host_definition_registry(
            &[("Player", original.type_id)],
            &[TestHostField::new("Player", "level", level_field()).type_hint("i64")],
            &[],
        ),
    )
    .expect("host root capture fixture should compile");
    let mut adapter = MockStateAdapter::new();
    adapter.insert_diagnostic_path_value(
        level_path(original),
        HostValue::Scalar(vela_common::ScalarValue::I64(10)),
    );
    adapter.insert_diagnostic_path_value(
        level_path(replacement),
        HostValue::Scalar(vela_common::ScalarValue::I64(100)),
    );
    let mut access = HostAccess::new();
    let mut budget = ExecutionBudget::new(BUDGET_CEILING, usize::MAX, usize::MAX);
    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            access: &mut access,
            script_globals: None,
        };
        run_linked_test_program_with_host_budget(
            &Vm::new(),
            &program,
            "main",
            &[
                OwnedValue::HostRef(original),
                OwnedValue::HostRef(replacement),
            ],
            &mut host,
            &mut budget,
        )
    };

    assert_eq!(result, Ok(OwnedValue::i64(0)));
    assert_eq!(
        adapter.read_diagnostic_path(&level_path(original)),
        Ok(HostValue::Scalar(vela_common::ScalarValue::I64(11)))
    );
    assert_eq!(
        adapter.read_diagnostic_path(&level_path(replacement)),
        Ok(HostValue::Scalar(vela_common::ScalarValue::I64(100)))
    );
}
