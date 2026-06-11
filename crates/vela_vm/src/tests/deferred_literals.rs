use super::*;

#[test]
fn linked_deferred_int_literal_contextualizes_from_integer_operand() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn inc(x) {
    return x + 1;
}
"#,
    )
    .expect("program should compile");

    for (input, expected) in [
        (
            OwnedValue::Scalar(vela_common::ScalarValue::I8(1)),
            OwnedValue::Scalar(vela_common::ScalarValue::I8(2)),
        ),
        (
            OwnedValue::Scalar(vela_common::ScalarValue::U32(1)),
            OwnedValue::Scalar(vela_common::ScalarValue::U32(2)),
        ),
        (
            OwnedValue::Scalar(vela_common::ScalarValue::I64(1)),
            OwnedValue::Scalar(vela_common::ScalarValue::I64(2)),
        ),
    ] {
        let mut budget = ExecutionBudget::unbounded();
        let value =
            run_linked_test_program_with_budget(&Vm::new(), &program, "inc", &[input], &mut budget)
                .expect("integer literal should contextualize from integer operand");
        assert_eq!(value, expected);
    }
}

#[test]
fn linked_deferred_int_literal_does_not_contextualize_to_float() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn inc(x) {
    return x + 1;
}
"#,
    )
    .expect("program should compile");
    let mut budget = ExecutionBudget::unbounded();

    let error = run_linked_test_program_with_budget(
        &Vm::new(),
        &program,
        "inc",
        &[OwnedValue::Scalar(vela_common::ScalarValue::F64(1.0))],
        &mut budget,
    )
    .expect_err("integer literal should not become a float");

    assert_eq!(
        error.kind(),
        VmErrorKind::TypeMismatch {
            operation: "binary_int_literal",
        }
    );
}

#[test]
fn linked_deferred_int_literal_checks_fit_for_operand_tag() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn add_large(x) {
    return x + 300;
}
"#,
    )
    .expect("program should compile");
    let mut budget = ExecutionBudget::unbounded();

    let error = run_linked_test_program_with_budget(
        &Vm::new(),
        &program,
        "add_large",
        &[OwnedValue::Scalar(vela_common::ScalarValue::U8(1))],
        &mut budget,
    )
    .expect_err("integer literal should fail when it does not fit operand tag");

    assert_eq!(
        error.kind(),
        VmErrorKind::TypeMismatch {
            operation: "binary_int_literal",
        }
    );
}

#[test]
fn linked_deferred_float_literal_contextualizes_from_float_operand() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn scale(x) {
    return x * 1.5;
}
"#,
    )
    .expect("program should compile");
    let mut budget = ExecutionBudget::unbounded();

    let value = run_linked_test_program_with_budget(
        &Vm::new(),
        &program,
        "scale",
        &[OwnedValue::Scalar(vela_common::ScalarValue::F32(2.0))],
        &mut budget,
    )
    .expect("float literal should contextualize from float operand");

    assert_eq!(
        value,
        OwnedValue::Scalar(vela_common::ScalarValue::F32(3.0))
    );
}

#[test]
fn linked_deferred_float_literal_does_not_contextualize_to_integer() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn add_fraction(x) {
    return x + 1.5;
}
"#,
    )
    .expect("program should compile");
    let mut budget = ExecutionBudget::unbounded();

    let error = run_linked_test_program_with_budget(
        &Vm::new(),
        &program,
        "add_fraction",
        &[OwnedValue::Scalar(vela_common::ScalarValue::I64(1))],
        &mut budget,
    )
    .expect_err("float literal should not become an integer");

    assert_eq!(
        error.kind(),
        VmErrorKind::TypeMismatch {
            operation: "binary_float_literal",
        }
    );
}

#[test]
fn linked_deferred_literal_preserves_left_side_for_non_commutative_ops() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn from_one(x) {
    return 1 - x;
}
"#,
    )
    .expect("program should compile");
    let mut budget = ExecutionBudget::unbounded();

    let value = run_linked_test_program_with_budget(
        &Vm::new(),
        &program,
        "from_one",
        &[OwnedValue::Scalar(vela_common::ScalarValue::I64(4))],
        &mut budget,
    )
    .expect("left-side literal should execute before value operand");

    assert_eq!(value, OwnedValue::Scalar(vela_common::ScalarValue::I64(-3)));
}
