use super::*;

#[test]
fn runs_compiled_aggregate_const_reads() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
const BASE = 2;
const REWARDS = [BASE, BASE + 3, 7];
const TABLE = {"gold": BASE, "xp": BASE + 4};

fn main() {
    let rewards = REWARDS;
    rewards.push(11);
    let fresh = REWARDS;
    return fresh.len() * 100 + rewards.sum() + TABLE["xp"];
}
"#,
    )
    .expect("compile aggregate const source");
    let mut vm = Vm::new();
    vm.register_standard_natives();

    assert_eq!(vm.run_program(&program, "main", &[]), Ok(Value::Int(331)));
}

#[test]
fn managed_heap_execution_runs_aggregate_const_reads() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
const WORDS = ["boar", "wolf"];
const TAGS = {"event": "kill", "kind": "wolf"};

fn main() {
    let words = WORDS;
    let tags = TAGS;
    if words.join(",") == "boar,wolf" && tags["kind"] == "wolf" {
        return 1;
    }
    return 0;
}
"#,
    )
    .expect("compile managed aggregate const source");
    let mut vm = Vm::new();
    vm.register_standard_natives();
    let mut budget = ExecutionBudget::unbounded();

    assert_eq!(
        vm.run_program_with_managed_heap_and_budget(&program, "main", &[], &mut budget),
        Ok(Value::Int(1))
    );
}

#[test]
fn compiled_aggregate_const_reads_emit_aggregate_constants() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
const VALUES = [1, 2, 3];
const LABELS = {"kind": "quest"};

fn main() {
    let values = VALUES;
    let labels = LABELS;
    return values.len() + labels["kind"].len();
}
"#,
    )
    .expect("compile aggregate const bytecode source");
    let code = program.function("main").expect("main function");

    assert!(code.constants.contains(&Constant::Array(vec![
        Constant::Int(1),
        Constant::Int(2),
        Constant::Int(3),
    ])));
    assert!(code.constants.contains(&Constant::Map(vec![(
        "kind".to_owned(),
        Constant::String("quest".to_owned()),
    )])));
}
