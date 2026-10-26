use super::*;
use crate::owned_value::OwnedValue;
use crate::value::Value as RuntimeValue;

#[test]
fn managed_heap_execution_runs_for_in_source() {
    let program = compile_test_program(
        SourceId::new(1),
        r#"
fn sum() {
    let total = 0;
    for value in [1, 2, 3] {
        total += value;
    }
    for reward in { "gold": 4, "xp": 6 } {
        total += reward.value;
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
        run_linked_test_program_with_budget(&Vm::new(), &program, "sum", &[], &mut budget)
            .expect("run heap for-in sum"),
        OwnedValue::Scalar(vela_common::ScalarValue::I64(16))
    );
    assert_eq!(
        run_linked_test_program_with_budget(&Vm::new(), &program, "last_name", &[], &mut budget)
            .expect("run heap for-in string"),
        OwnedValue::String("xp".into())
    );
    assert_eq!(budget.memory_bytes_allocated(), 0);
}

#[test]
fn managed_heap_execution_runs_native_iterator_for_in_source() {
    let program = compile_standard_program_source_with_native_functions(
        SourceId::new(1),
        r#"
fn main() {
    let names = [];
    for value in game::names() {
        names.push(value);
    }
    return names.join(",");
}
"#,
        &["game::names"],
    )
    .expect("compile heap native iterator for-in source");
    let mut vm = Vm::new();
    vm.register_standard_natives();
    vm.register_native("game::names", |_| {
        Ok(OwnedValue::Array(vec![
            OwnedValue::String("gold".to_owned()),
            OwnedValue::String("xp".to_owned()),
        ]))
    });
    let mut budget = ExecutionBudget::unbounded();

    assert_eq!(
        run_linked_test_program_with_budget(&vm, &program, "main", &[], &mut budget),
        Ok(OwnedValue::String("gold,xp".to_owned()))
    );
    assert_eq!(budget.memory_bytes_allocated(), 0);
}
#[test]
fn managed_heap_execution_runs_for_in_string_value_methods() {
    let program = compile_standard_program_source(
        SourceId::new(1),
        r#"
fn main() {
    let total = 0;
    for label in ["quest", "raid", "daily", "bonus"] {
        if label.starts_with("q") || label.contains("i") {
            total += label.len();
        }
    }
    return total;
}
"#,
    )
    .expect("compile for-in string value methods");
    let mut vm = Vm::new();
    vm.register_standard_natives();
    let mut budget = ExecutionBudget::unbounded();

    assert_eq!(
        run_linked_test_program_with_budget(&vm, &program, "main", &[], &mut budget),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(14)))
    );
    assert_eq!(budget.memory_bytes_allocated(), 0);
}

#[test]
fn managed_heap_execution_runs_range_for_in_source() {
    let program = compile_test_program(
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
        run_linked_test_program_with_budget(&Vm::new(), &program, "main", &[], &mut budget),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(9)))
    );
    assert_eq!(budget.memory_bytes_allocated(), 0);
}

#[test]
fn managed_heap_execution_runs_script_value_methods() {
    let program = compile_standard_program_source(
        SourceId::new(1),
        include_str!("../../tests/fixtures/execution/managed_value_methods.vela"),
    )
    .expect("compile heap script value methods");
    let mut budget = ExecutionBudget::unbounded();

    let mut vm = Vm::new();
    vm.register_standard_natives();

    assert_eq!(
        run_linked_test_program_with_budget(&vm, &program, "main", &[], &mut budget),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(4)))
    );
    assert_eq!(budget.memory_bytes_allocated(), 0);
}

#[test]
fn managed_heap_execution_uses_record_identity_map_set_keys() {
    let program = compile_standard_program_source(
        SourceId::new(1),
        include_str!("../../tests/fixtures/execution/record_identity_keys.vela"),
    )
    .expect("compile value-keyed record map/set source");
    let mut budget = ExecutionBudget::unbounded();
    let mut vm = Vm::new();
    vm.register_standard_natives();

    assert_eq!(
        run_linked_test_program_with_budget(&vm, &program, "main", &[], &mut budget),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(11)))
    );
}

#[test]
fn managed_heap_execution_uses_closure_and_iterator_identity_map_set_keys() {
    let program = compile_standard_program_source(
        SourceId::new(1),
        include_str!("../../tests/fixtures/execution/closure_iterator_identity_keys.vela"),
    )
    .expect("compile value-keyed closure/iterator map/set source");
    let mut budget = ExecutionBudget::unbounded();
    let mut vm = Vm::new();
    vm.register_standard_natives();

    assert_eq!(
        run_linked_test_program_with_budget(&vm, &program, "main", &[], &mut budget),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(29)))
    );
}

#[test]
fn managed_heap_map_indexing_uses_record_identity_keys() {
    let program = compile_standard_program_source(
        SourceId::new(1),
        include_str!("../../tests/fixtures/execution/record_index_identity.vela"),
    )
    .expect("compile value-keyed record map indexing source");
    let mut budget = ExecutionBudget::unbounded();
    let mut vm = Vm::new();
    vm.register_standard_natives();

    assert_eq!(
        run_linked_test_program_with_budget(&vm, &program, "main", &[], &mut budget),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(26)))
    );
}

#[test]
fn managed_heap_execution_uses_finite_float_map_set_keys() {
    let program = compile_standard_program_source(
        SourceId::new(1),
        include_str!("../../tests/fixtures/execution/finite_float_keys.vela"),
    )
    .expect("compile finite float map/set source");
    let mut budget = ExecutionBudget::unbounded();
    let mut vm = Vm::new();
    vm.register_standard_natives();

    assert_eq!(
        run_linked_test_program_with_budget(&vm, &program, "main", &[], &mut budget),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(19)))
    );
}

#[test]
fn managed_heap_map_replacement_preserves_first_inserted_key() {
    let program = compile_standard_program_source(
        SourceId::new(1),
        r#"
fn main() {
    let scores = {"seed": 0};
    scores.clear();
    scores.set(-0.0f64, 8);
    scores.set(0.0f64, 9);
    return scores;
}
"#,
    )
    .expect("compile map key replacement source");
    let mut budget = ExecutionBudget::unbounded();
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let result = run_linked_test_program_with_budget(&vm, &program, "main", &[], &mut budget)
        .expect("run map key replacement source");

    let OwnedValue::Map(entries) = result else {
        panic!("main should return a map");
    };
    assert_eq!(entries.len(), 1, "0.0 and -0.0 must share a ValueKey");
    assert_eq!(
        entries[0].value,
        OwnedValue::Scalar(vela_common::ScalarValue::I64(9))
    );
    let OwnedValue::Scalar(vela_common::ScalarValue::F64(key)) = entries[0].key else {
        panic!("stored key should remain the first f64 key");
    };
    assert!(
        key.is_sign_negative(),
        "replacing an existing map entry must not replace the original key value"
    );
}

#[test]
fn managed_heap_execution_runs_script_impl_method_dispatch() {
    let program = compile_test_program(
        SourceId::new(1),
        r#"
trait BonusSource { fn bonus(self, amount) -> i64; }
struct Player { level: i64 }

impl BonusSource for Player {
    fn bonus(self, amount) -> i64 {
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
        run_linked_test_program_with_budget(&Vm::new(), &program, "main", &[], &mut budget),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(14)))
    );
    assert_eq!(budget.memory_bytes_allocated(), 0);
}

#[test]
fn managed_heap_execution_runs_trait_default_method_dispatch() {
    let program = compile_standard_program_source(
        SourceId::new(1),
        include_str!("../../tests/fixtures/execution/managed_trait_defaults.vela"),
    )
    .expect("compile heap trait default method dispatch");
    let mut budget = ExecutionBudget::unbounded();

    assert_eq!(
        run_linked_test_program_with_budget(&Vm::new(), &program, "main", &[], &mut budget),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(18)))
    );
    assert_eq!(budget.memory_bytes_allocated(), 0);
}

#[test]
fn runs_compiled_const_expression_source() {
    let code = compile_test_function(
        SourceId::new(1),
        r#"
const BASE: i64 = 10;
const BONUS: i64 = BASE + 5 * 2;

fn main() {
    return BONUS;
}
"#,
        "main",
    )
    .expect("compile const expression source");

    assert_eq!(
        run_linked_test_code(code),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(20)))
    );
}

#[test]
fn runs_compiled_native_call_source() {
    let mut vm = Vm::new();
    vm.register_native("log", |args| {
        assert_eq!(args, [OwnedValue::String("compiled".into())]);
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(7)))
    });

    let program = compile_standard_program_source_with_native_functions(
        SourceId::new(1),
        "fn main() { return log(\"compiled\"); }",
        &["log"],
    )
    .expect("compile native call source");
    let mut budget = ExecutionBudget::unbounded();

    assert_eq!(
        run_linked_test_program_with_budget(&vm, &program, "main", &[], &mut budget),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(7)))
    );
}

#[test]
fn heap_execution_materializes_native_args_and_stores_result() {
    let mut vm = Vm::new();
    vm.register_native("echo_label", |args| {
        assert_eq!(args, [OwnedValue::String("compiled".into())]);
        Ok(OwnedValue::String("native-result".into()))
    });
    let program = compile_standard_program_source_with_native_functions(
        SourceId::new(1),
        "fn main() { return echo_label(\"compiled\"); }",
        &["echo_label"],
    )
    .expect("compile native call source");
    let mut heap = ScriptHeap::new();
    let mut heap_execution = HeapExecution::new(&mut heap);
    let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX);

    let result = run_linked_test_program_runtime_with_heap_and_budget(
        &vm,
        &program,
        "main",
        &[],
        &mut heap_execution,
        &mut budget,
    )
    .expect("run heap native call");

    let RuntimeValue::HeapRef(result_ref) = result else {
        panic!("expected heap-backed native result");
    };
    assert_eq!(
        heap.get(result_ref),
        Some(&HeapValue::String("native-result".into()))
    );
}

#[test]
fn runs_compiled_script_function_calls() {
    let program = compile_test_program(
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
    let mut budget = ExecutionBudget::unbounded();

    assert_eq!(
        run_linked_test_program_with_budget(&Vm::new(), &program, "main", &[], &mut budget),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(30)))
    );
}

#[test]
fn runs_linked_compiled_script_function_calls() {
    let program = compile_test_program(
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
    let linked = Linker::new()
        .link_test_program(&program)
        .expect("link compiled program");

    assert_eq!(
        Vm::new().run_linked_program(&linked, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(30)))
    );
}

#[test]
fn runs_compiled_named_args_and_parameter_defaults() {
    let program = compile_test_program(
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
    let mut budget = ExecutionBudget::unbounded();

    assert_eq!(
        run_linked_test_program_with_budget(&Vm::new(), &program, "main", &[], &mut budget),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(16)))
    );
}

#[test]
fn runs_complex_parameter_default_expressions() {
    let program = compile_test_program(
        SourceId::new(1),
        r#"
fn double(value) {
    return value * 2;
}

fn grant(base, amount = double(base), bonus = amount + double(1)) {
    return base + amount + bonus;
}

fn main() {
    return grant(base = 4);
}
"#,
    )
    .expect("complex parameter default expressions should compile");
    let mut budget = ExecutionBudget::unbounded();

    assert_eq!(
        run_linked_test_program_with_budget(&Vm::new(), &program, "main", &[], &mut budget),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(22)))
    );
}

#[test]
fn runs_logical_parameter_default_expressions() {
    let program = compile_test_program(
        SourceId::new(1),
        r#"
fn defaults(any_true = true || (1 / 0 == 0), all_false = false && (1 / 0 == 0)) {
    if any_true && !all_false {
        return 7;
    }
    return 0;
}

fn main() {
    return defaults();
}
"#,
    )
    .expect("logical parameter defaults should compile");
    let mut budget = ExecutionBudget::unbounded();

    assert_eq!(
        run_linked_test_program_with_budget(&Vm::new(), &program, "main", &[], &mut budget),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(7)))
    );
}

#[test]
fn runs_simple_block_parameter_default_expressions() {
    let program = compile_test_program(
        SourceId::new(1),
        r#"
fn defaults(value = { 1 + 2 }, empty = {}, semicolon = { 9; }) {
    empty;
    semicolon;
    return value;
}

fn main() {
    return defaults();
}
"#,
    )
    .expect("simple block parameter defaults should compile");
    let mut budget = ExecutionBudget::unbounded();

    assert_eq!(
        run_linked_test_program_with_budget(&Vm::new(), &program, "main", &[], &mut budget),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(3)))
    );
}

#[test]
fn runs_if_parameter_default_expressions() {
    let program = compile_test_program(
        SourceId::new(1),
        r#"
fn defaults(value = if false { 1 } else if true { 2 } else { 3 }, missing = if false { 9 }) {
    missing;
    return value;
}

fn main() {
    return defaults();
}
"#,
    )
    .expect("if parameter defaults should compile");
    let mut budget = ExecutionBudget::unbounded();

    assert_eq!(
        run_linked_test_program_with_budget(&Vm::new(), &program, "main", &[], &mut budget),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );
}

#[test]
fn runs_index_parameter_default_expressions() {
    let program = compile_test_program(
        SourceId::new(1),
        r#"
fn defaults(first = [10, 20][1], second = { "key": 7 }["key"], third = [[1], [2]][1][0]) {
    return first + second + third;
}

fn main() {
    return defaults();
}
"#,
    )
    .expect("index parameter defaults should compile");
    let mut budget = ExecutionBudget::unbounded();

    assert_eq!(
        run_linked_test_program_with_budget(&Vm::new(), &program, "main", &[], &mut budget),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(29)))
    );
}

#[test]
fn runs_entrypoint_parameter_defaults() {
    let code = compile_test_function(
        SourceId::new(1),
        r#"
fn main(value = 7) {
    return value + 1;
}
"#,
        "main",
    )
    .expect("compile entrypoint default");

    assert_eq!(
        run_linked_test_code(code),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(8)))
    );
}

#[test]
fn runs_compiled_lambdas_with_captures_after_outer_return() {
    let program = compile_test_program(
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
    let mut budget = ExecutionBudget::unbounded();

    assert_eq!(
        run_linked_test_program_with_budget(&Vm::new(), &program, "main", &[], &mut budget),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(15)))
    );
}

#[test]
fn program_image_flattens_lambdas_and_linked_program_runs_captures_after_outer_return() {
    let program = compile_test_program(
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
    let linked = Linker::new()
        .link_test_program(&program)
        .expect("link captured lambda program");
    let image = linked.image();
    let make_adder = image
        .function_by_name("make_adder")
        .expect("make_adder image function");
    let closure_index = make_adder
        .instructions
        .iter()
        .find_map(|instruction| match &instruction.kind {
            UnlinkedInstructionKind::MakeClosure { function, .. } => Some(*function),
            _ => None,
        })
        .expect("make_adder should build a closure");

    assert!(make_adder.nested_functions.is_empty());
    assert_eq!(
        image
            .function(closure_index)
            .expect("image closure function")
            .params
            .as_slice(),
        ["value"]
    );

    assert_eq!(
        Vm::new().run_linked_program(&linked, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(15)))
    );
}

#[test]
fn runs_compiled_nested_lambdas_with_transitive_captures() {
    let program = compile_test_program(
        SourceId::new(1),
        include_str!("../../tests/fixtures/execution/nested_captures.vela"),
    )
    .expect("compile nested captured lambda");
    let mut budget = ExecutionBudget::unbounded();

    assert_eq!(
        run_linked_test_program_with_budget(&Vm::new(), &program, "main", &[], &mut budget),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(21)))
    );
}

#[test]
fn runs_immediate_lambda_calls_and_block_returns() {
    let code = compile_test_function(
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

    assert_eq!(
        run_linked_test_code(code),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(11)))
    );
}

#[test]
fn runs_try_propagation_for_option_values() {
    let program = compile_test_program(
        SourceId::new(1),
        include_str!("../../tests/fixtures/execution/option_propagation.vela"),
    )
    .expect("compile option propagation");
    let mut budget = ExecutionBudget::unbounded();

    assert_eq!(
        run_linked_test_program_with_budget(&Vm::new(), &program, "present", &[], &mut budget),
        Ok(OwnedValue::Enum {
            enum_name: "Option".into(),
            variant: "Some".into(),
            fields: ScriptFields::from_pairs(
                "Option::Some",
                [(
                    "0".into(),
                    OwnedValue::Scalar(vela_common::ScalarValue::I64(5))
                )]
            ),
        })
    );
    assert_eq!(
        run_linked_test_program_with_budget(&Vm::new(), &program, "missing", &[], &mut budget),
        Ok(OwnedValue::Enum {
            enum_name: "Option".into(),
            variant: "None".into(),
            fields: ScriptFields::from_pairs("Option::None", BTreeMap::new()),
        })
    );
}

#[test]
fn managed_heap_execution_runs_try_propagation_for_result_values() {
    let program = compile_test_program(
        SourceId::new(1),
        include_str!("../../tests/fixtures/execution/result_propagation.vela"),
    )
    .expect("compile result propagation");
    let mut budget = ExecutionBudget::new(10_000, 4096, 64);

    assert_eq!(
        run_linked_test_program_with_budget(&Vm::new(), &program, "ok_case", &[], &mut budget),
        Ok(OwnedValue::Enum {
            enum_name: "Result".into(),
            variant: "Ok".into(),
            fields: ScriptFields::from_pairs(
                "Result::Ok",
                [(
                    "0".into(),
                    OwnedValue::Scalar(vela_common::ScalarValue::I64(10))
                )]
            ),
        })
    );

    let mut budget = ExecutionBudget::new(10_000, 4096, 64);
    assert_eq!(
        run_linked_test_program_with_budget(&Vm::new(), &program, "err_case", &[], &mut budget),
        Ok(OwnedValue::Enum {
            enum_name: "Result".into(),
            variant: "Err".into(),
            fields: ScriptFields::from_pairs(
                "Result::Err",
                [("0".into(), OwnedValue::String("bad".into()))],
            ),
        })
    );
}

#[test]
fn typed_try_propagation_rejects_cross_family_option_result_values() {
    let program = compile_test_program(
        SourceId::new(1),
        include_str!("../../tests/fixtures/contracts/cross_family_try.vela"),
    )
    .expect("compile cross-family propagation fixture");

    for entry in [
        "result_ok_in_option",
        "result_err_in_option",
        "option_some_in_result",
        "option_none_in_result",
    ] {
        let mut budget = ExecutionBudget::unbounded();
        let error =
            run_linked_test_program_with_budget(&Vm::new(), &program, entry, &[], &mut budget)
                .expect_err("typed cross-family try propagation should fail");
        assert_eq!(
            error.kind(),
            VmErrorKind::TypeMismatch {
                operation: "try propagation",
            },
            "{entry} should reject the wrong try family",
        );
    }
}

#[test]
fn managed_heap_execution_runs_string_parameter_defaults() {
    let program = compile_test_program(
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
    let mut budget = ExecutionBudget::new(10_000, 32_000, 32);

    assert_eq!(
        run_linked_test_program_with_budget(&Vm::new(), &program, "main", &[], &mut budget),
        Ok(OwnedValue::Bool(true))
    );
}

#[test]
fn managed_heap_execution_runs_interpolated_parameter_defaults() {
    let program = compile_test_program(
        SourceId::new(1),
        r#"
fn greet(name = "Ada", message = f"hello {name}") {
    return message;
}

fn main() {
    return greet();
}
"#,
    )
    .expect("compile interpolated parameter defaults");
    let mut budget = ExecutionBudget::new(10_000, 32_000, 32);

    assert_eq!(
        run_linked_test_program_with_budget(&Vm::new(), &program, "main", &[], &mut budget),
        Ok(OwnedValue::String("hello Ada".into()))
    );
}

#[test]
fn managed_heap_execution_runs_range_parameter_defaults() {
    let program = compile_test_program(
        SourceId::new(1),
        r#"
fn sum(values = 2..=4) {
    let total = 0;
    for value in values {
        total += value;
    }
    return total;
}

fn main() {
    return sum();
}
"#,
    )
    .expect("compile range parameter defaults");
    let mut budget = ExecutionBudget::new(10_000, 32_000, 32);

    assert_eq!(
        run_linked_test_program_with_budget(&Vm::new(), &program, "main", &[], &mut budget),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(9)))
    );
}
