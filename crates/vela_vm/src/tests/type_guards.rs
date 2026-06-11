use super::*;

#[test]
fn linked_guard_type_accepts_matching_primitive_contract() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(value) {
    let amount: i64 = value;
    return amount;
}
"#,
    )
    .expect("program should compile");
    let mut budget = ExecutionBudget::unbounded();

    let value = run_linked_test_program_with_budget(
        &Vm::new(),
        &program,
        "main",
        &[OwnedValue::i64(42)],
        &mut budget,
    )
    .expect("matching guard should pass");

    assert_eq!(value, OwnedValue::i64(42));
}

#[test]
fn linked_guard_type_rejects_dynamic_contract_mismatch() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(value) {
    let amount: i64 = value;
    return amount;
}
"#,
    )
    .expect("program should compile");
    let mut budget = ExecutionBudget::unbounded();

    let error = run_linked_test_program_with_budget(
        &Vm::new(),
        &program,
        "main",
        &[OwnedValue::String("not an integer".to_owned())],
        &mut budget,
    )
    .expect_err("mismatched guard should fail");

    assert_eq!(
        error.kind(),
        VmErrorKind::TypeContractViolation {
            expected: "i64".to_owned(),
            actual: "string".to_owned(),
            debug_name: "amount".to_owned(),
        }
    );
}
