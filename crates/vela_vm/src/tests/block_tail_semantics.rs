use super::*;
use vela_common::ScalarValue;

fn call(program: &UnlinkedProgram, entry: &str) -> VmResult<OwnedValue> {
    let mut budget = ExecutionBudget::unbounded();
    run_linked_test_program_with_budget(&Vm::new(), program, entry, &[], &mut budget)
}

#[test]
fn production_vm_respects_terminated_block_tail_semantics() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn terminated() {
    return { 9; };
}

fn nonterminated() {
    return { 9 };
}

fn defaulted(value = { 9; }) {
    return value;
}
"#,
    )
    .expect("block-tail fixture should compile");

    assert_eq!(call(&program, "terminated"), Ok(OwnedValue::Unit));
    assert_eq!(
        call(&program, "nonterminated"),
        Ok(OwnedValue::Scalar(ScalarValue::I64(9)))
    );
    assert_eq!(call(&program, "defaulted"), Ok(OwnedValue::Unit));
}
