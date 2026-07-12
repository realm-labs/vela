use super::*;

#[test]
fn compiler_contextualizes_negated_unsuffixed_signed_minimum() {
    let code = compile_test_function(
        SourceId::new(1),
        "fn main() { let value: i8 = -128; return value; }",
        "main",
    )
    .expect("contextual signed minimum should compile");

    assert!(
        code.constants
            .contains(&Constant::Scalar(vela_common::ScalarValue::I8(i8::MIN)))
    );
}

#[test]
fn compiler_contextualizes_negated_literal_in_typed_binary_operation() {
    let code = compile_test_function(
        SourceId::new(1),
        "fn main(value: i8) { return value + -128; }",
        "main",
    )
    .expect("typed binary literal should use the receiver primitive");

    assert!(
        code.constants
            .contains(&Constant::Scalar(vela_common::ScalarValue::I8(i8::MIN)))
    );
}

#[test]
fn compiler_preserves_unsigned_negation_as_an_operation() {
    let code = compile_test_function(SourceId::new(1), "fn main() { return -1u8; }", "main")
        .expect("unsigned literal validation must not fold unary negation");

    assert!(
        code.constants
            .contains(&Constant::Scalar(vela_common::ScalarValue::U8(1)))
    );
    assert!(
        code.instructions
            .iter()
            .any(|instruction| matches!(instruction.kind, UnlinkedInstructionKind::Negate { .. }))
    );
}

#[test]
fn compiler_rejects_contextual_negative_overflow_at_operand_span() {
    const SOURCE: &str = "fn main() { let value: i8 = -129; return value; }";
    let error = compile_test_function(SourceId::new(1), SOURCE, "main")
        .expect_err("contextual negative overflow should fail");

    let span = semantic_literal_error_span(
        error,
        "compiler::invalid_int_literal",
        "invalid integer literal `129`: integer literal out of range",
    );
    assert_eq!(&SOURCE[span.start as usize..span.end as usize], "129");
}

#[test]
fn compiler_rejects_f32_and_f64_literal_overflow() {
    for (source, literal) in [
        ("fn main() { return 3.5e38f32; }", "3.5e38f32"),
        ("fn main() { return 1.8e308f64; }", "1.8e308f64"),
    ] {
        let error = compile_test_function(SourceId::new(1), source, "main")
            .expect_err("non-finite float literal should fail");
        let span = semantic_literal_error_span(
            error,
            "compiler::invalid_float_literal",
            &format!("invalid float literal `{literal}`: float literal out of range"),
        );
        assert_eq!(&source[span.start as usize..span.end as usize], literal);
    }
}

#[test]
fn compiler_validates_deferred_dynamic_literal_range() {
    const SOURCE: &str = "fn main(value) { return value + 18446744073709551616; }";
    let error = compile_test_function(SourceId::new(1), SOURCE, "main")
        .expect_err("no numeric primitive can hold the deferred literal");

    let span = semantic_literal_error_span(
        error,
        "compiler::invalid_int_literal",
        "invalid integer literal `18446744073709551616`: integer literal out of range",
    );
    assert_eq!(
        &SOURCE[span.start as usize..span.end as usize],
        "18446744073709551616"
    );
}

fn semantic_literal_error_span(error: TestCompileError, code: &str, message: &str) -> Span {
    let diagnostics = error.into_semantic_diagnostics();
    let [diagnostic] = diagnostics.as_slice() else {
        panic!("expected one literal diagnostic, got {diagnostics:?}");
    };
    assert_eq!(diagnostic.code.as_deref(), Some(code));
    assert_eq!(diagnostic.message, message);
    diagnostic.span.expect("literal diagnostic span")
}
