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

#[test]
fn linked_parameter_guard_rejects_public_entry_mismatch() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(value: i64) {
    return value;
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
    .expect_err("checked entry should reject mismatched argument");

    assert_eq!(
        error.kind(),
        VmErrorKind::TypeContractViolation {
            expected: "i64".to_owned(),
            actual: "string".to_owned(),
            debug_name: "value".to_owned(),
        }
    );
}

#[test]
fn linked_parameter_guard_rejects_nested_script_call_mismatch() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn require_i64(value: i64) {
    return value;
}

fn main(value) {
    return require_i64(value);
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
    .expect_err("script call checked entry should reject mismatched argument");

    assert_eq!(
        error.kind(),
        VmErrorKind::TypeContractViolation {
            expected: "i64".to_owned(),
            actual: "string".to_owned(),
            debug_name: "value".to_owned(),
        }
    );
}

#[test]
fn linked_parameter_guard_accepts_string_and_bytes_primitive_tags() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn echo_text(value: string) {
    return value;
}

fn echo_bytes(value: bytes) {
    return value;
}
"#,
    )
    .expect("program should compile");

    let mut budget = ExecutionBudget::unbounded();
    let text = run_linked_test_program_with_budget(
        &Vm::new(),
        &program,
        "echo_text",
        &[OwnedValue::String("ok".to_owned())],
        &mut budget,
    )
    .expect("string primitive guard should pass");
    assert_eq!(text, OwnedValue::String("ok".to_owned()));

    let bytes = run_linked_test_program_with_budget(
        &Vm::new(),
        &program,
        "echo_bytes",
        &[OwnedValue::Bytes(vec![0, 1, 255])],
        &mut budget,
    )
    .expect("bytes primitive guard should pass");
    assert_eq!(bytes, OwnedValue::Bytes(vec![0, 1, 255]));
}

#[test]
fn linked_return_guard_rejects_dynamic_contract_mismatch() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(value) -> i64 {
    return value;
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
    .expect_err("return guard should reject mismatched value");

    assert_eq!(
        error.kind(),
        VmErrorKind::TypeContractViolation {
            expected: "i64".to_owned(),
            actual: "string".to_owned(),
            debug_name: "return".to_owned(),
        }
    );
}

#[test]
fn static_contract_mismatch_remains_compile_error() {
    compile_program_source(
        SourceId::new(1),
        r#"
fn require_i64(value: i64) {
    return value;
}

fn main() {
    return require_i64("not an integer");
}
"#,
    )
    .expect_err("statically known mismatch should not reach runtime");
}
