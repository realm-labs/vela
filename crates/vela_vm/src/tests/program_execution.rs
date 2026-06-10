use super::*;
use crate::owned_value::OwnedValue;
use crate::value::Value as RuntimeValue;

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
        run_linked_test_program_with_budget(&Vm::new(), &program, "sum", &[], &mut budget)
            .expect("run heap for-in sum"),
        OwnedValue::Int(16)
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
        run_linked_test_program_with_budget(&Vm::new(), &program, "main", &[], &mut budget),
        Ok(OwnedValue::Int(9))
    );
    assert_eq!(budget.memory_bytes_allocated(), 0);
}

#[test]
fn managed_heap_execution_runs_script_value_methods() {
    let program = compile_standard_program_source(
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
    let popped_name = option::unwrap_or(popped, "");
    if names.len() == 2 && popped_name == "quest" && popped_name.contains("ue") && popped_name.starts_with("que")
        && popped_name.ends_with("st") && option::is_none(missing_pop) && option::unwrap_or(removed, 0) == 4 && rewards.is_empty() == false && ("quest").len() == 5
        && option::is_none(missing_get) && option::is_none(missing_remove)
        && rewards.has("quest") && option::unwrap_or(rewards.get("xp"), 0) == 6 && rewards.get_or("missing", "fallback") == "fallback"
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
        run_linked_test_program_with_budget(&vm, &program, "main", &[], &mut budget),
        Ok(OwnedValue::Int(4))
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
        run_linked_test_program_with_budget(&Vm::new(), &program, "main", &[], &mut budget),
        Ok(OwnedValue::Int(14))
    );
    assert_eq!(budget.memory_bytes_allocated(), 0);
}

#[test]
fn managed_heap_execution_runs_trait_default_method_dispatch() {
    let program = compile_standard_program_source(
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
    if player.label() == "hero" {
        return player.bonus(6) + 4;
    }
    return 0;
}
"#,
    )
    .expect("compile heap trait default method dispatch");
    let mut budget = ExecutionBudget::unbounded();

    assert_eq!(
        run_linked_test_program_with_budget(&Vm::new(), &program, "main", &[], &mut budget),
        Ok(OwnedValue::Int(18))
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

    assert_eq!(Vm::new().run(&code), Ok(OwnedValue::Int(20)));
}

#[test]
fn runs_compiled_native_call_source() {
    let mut vm = Vm::new();
    vm.register_native("log", |args| {
        assert_eq!(args, [OwnedValue::String("compiled".into())]);
        Ok(OwnedValue::Int(7))
    });

    let code = compile_function_source(
        SourceId::new(1),
        "fn main() { return log(\"compiled\"); }",
        "main",
    )
    .expect("compile native call source");

    assert_eq!(vm.run(&code), Ok(OwnedValue::Int(7)));
}

#[test]
fn heap_execution_materializes_native_args_and_stores_result() {
    let mut vm = Vm::new();
    vm.register_native("echo_label", |args| {
        assert_eq!(args, [OwnedValue::String("compiled".into())]);
        Ok(OwnedValue::String("native-result".into()))
    });
    let code = compile_function_source(
        SourceId::new(1),
        "fn main() { return echo_label(\"compiled\"); }",
        "main",
    )
    .expect("compile native call source");
    let mut heap = ScriptHeap::new();
    let mut heap_execution = HeapExecution::new(&mut heap);
    let mut budget = ExecutionBudget::new(u64::MAX, 4096, usize::MAX);

    let result = vm
        .run_with_heap_and_budget(&code, &mut heap_execution, &mut budget)
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
        Ok(OwnedValue::Int(30))
    );
}

#[test]
fn runs_linked_compiled_script_function_calls() {
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
    let linked = Linker::new()
        .link_program(&program)
        .expect("link compiled program");

    assert_eq!(
        Vm::new().run_linked_program(&linked, "main", &[]),
        Ok(OwnedValue::Int(30))
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
        Ok(OwnedValue::Int(16))
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

    assert_eq!(Vm::new().run(&code), Ok(OwnedValue::Int(8)));
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
        Ok(OwnedValue::Int(15))
    );
}

#[test]
fn program_image_flattens_lambdas_and_linked_program_runs_captures_after_outer_return() {
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
    let image = ProgramImage::from_program(&program);
    let linked = Linker::new()
        .link_program(&program)
        .expect("link captured lambda program");
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
        Ok(OwnedValue::Int(15))
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
        Ok(OwnedValue::Int(21))
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

    assert_eq!(Vm::new().run(&code), Ok(OwnedValue::Int(11)));
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
        return Option::Some(value);
    }
    return Option::None {};
}

fn present() {
    let value = maybe(4)?;
    return Option::Some(value + 1);
}

fn missing() {
    let value = maybe(0)?;
    return Option::Some(value + 1);
}
"#,
    )
    .expect("compile option propagation");

    assert_eq!(
        Vm::new().run_program(&program, "present", &[]),
        Ok(OwnedValue::Enum {
            enum_name: "Option".into(),
            variant: "Some".into(),
            fields: ScriptFields::from_pairs("Option::Some", [("0".into(), OwnedValue::Int(5))]),
        })
    );
    assert_eq!(
        Vm::new().run_program(&program, "missing", &[]),
        Ok(OwnedValue::Enum {
            enum_name: "Option".into(),
            variant: "None".into(),
            fields: ScriptFields::from_pairs("Option::None", BTreeMap::new()),
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
        return Result::Ok(value);
    }
    return Result::Err("bad");
}

fn ok_case() {
    let value = checked(3)?;
    return Result::Ok(value + 7);
}

fn err_case() {
    let value = checked(0)?;
    return Result::Ok(value + 7);
}
"#,
    )
    .expect("compile result propagation");
    let mut budget = ExecutionBudget::new(10_000, 4096, 64);

    assert_eq!(
        run_linked_test_program_with_budget(&Vm::new(), &program, "ok_case", &[], &mut budget),
        Ok(OwnedValue::Enum {
            enum_name: "Result".into(),
            variant: "Ok".into(),
            fields: ScriptFields::from_pairs("Result::Ok", [("0".into(), OwnedValue::Int(10))]),
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
    let mut budget = ExecutionBudget::new(10_000, 32_000, 32);

    assert_eq!(
        run_linked_test_program_with_budget(&Vm::new(), &program, "main", &[], &mut budget),
        Ok(OwnedValue::Bool(true))
    );
}
