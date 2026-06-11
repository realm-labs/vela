use super::*;
use crate::owned_value::OwnedValue;

#[test]
fn runs_compiled_aggregate_const_reads() {
    let program = compile_standard_program_source(
        SourceId::new(1),
        r#"
const BASE = 2;
const REWARDS = [BASE, BASE + 3, 7];
const TABLE = {"gold": BASE, "xp": BASE + 4};

fn main() {
    let rewards = REWARDS;
    rewards[0] = 11;
    let fresh = REWARDS;
    return fresh[0] * 100 + fresh[1] * 10 + fresh[2] + rewards[0] + TABLE["xp"];
}
"#,
    )
    .expect("compile aggregate const source");
    let mut vm = Vm::new();
    vm.register_standard_natives();
    let mut budget = ExecutionBudget::unbounded();

    assert_eq!(
        run_linked_test_program_with_budget(&vm, &program, "main", &[], &mut budget),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(274)))
    );
}

#[test]
fn managed_heap_execution_runs_aggregate_const_reads() {
    let program = compile_standard_program_source(
        SourceId::new(1),
        r#"
const WORDS = ["boar", "wolf"];
const TAGS = {"event": "kill", "kind": "wolf"};

fn main() {
    let words = WORDS;
    let tags = TAGS;
    if words[0] == "boar" && words[1] == "wolf" && tags["kind"] == "wolf" {
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
        run_linked_test_program_with_budget(&vm, &program, "main", &[], &mut budget),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
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
        Constant::Scalar(vela_common::ScalarValue::I64(1)),
        Constant::Scalar(vela_common::ScalarValue::I64(2)),
        Constant::Scalar(vela_common::ScalarValue::I64(3)),
    ])));
    assert!(code.constants.contains(&Constant::Map(vec![(
        "kind".to_owned(),
        Constant::String("quest".to_owned()),
    )])));
}

#[test]
fn runs_cross_module_imported_aggregate_const_reads() {
    let registry = vela_stdlib::standard_registry().expect("standard registry should build");
    let program = vela_bytecode::compiler::compile_module_sources_with_registry(
        &[
            ModuleSource::new(
                SourceId::new(1),
                ModulePath::from_qualified("game::main"),
                r#"
use game::tuning::REWARDS
use game::tuning::LABELS as META

fn main() {
    let rewards = REWARDS;
    rewards[0] = 9;
    let fresh = REWARDS;
    return fresh[0] * 100 + fresh[1] * 10 + fresh[2] + rewards[0] + META["xp"];
}
"#,
            ),
            ModuleSource::new(
                SourceId::new(2),
                ModulePath::from_qualified("game::tuning"),
                r#"
use game::base::BASE

pub const REWARDS = [BASE, BASE + 2, 7];
pub const LABELS = {"xp": BASE + 5};
"#,
            ),
            ModuleSource::new(
                SourceId::new(3),
                ModulePath::from_qualified("game::base"),
                r#"
pub const BASE = 3;
"#,
            ),
        ],
        registry.compile_view(),
    )
    .expect("compile imported aggregate const source");
    let mut vm = Vm::new();
    vm.register_standard_natives();
    let mut budget = ExecutionBudget::unbounded();

    assert_eq!(
        run_linked_test_program_with_budget(&vm, &program, "game::main::main", &[], &mut budget),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(374)))
    );
}
