use super::*;

#[test]
fn compiler_contextualizes_negated_unsuffixed_signed_minimum() {
    let code = compile_function_source(
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
    let code = compile_function_source(
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
    let code = compile_function_source(SourceId::new(1), "fn main() { return -1u8; }", "main")
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
    let error = compile_function_source(SourceId::new(1), SOURCE, "main")
        .expect_err("contextual negative overflow should fail");

    assert_eq!(
        error.kind,
        CompileErrorKind::InvalidIntLiteral {
            literal: "129".to_owned(),
            error: "integer literal out of range".to_owned(),
        }
    );
    let span = error.span.expect("literal error span");
    assert_eq!(&SOURCE[span.start as usize..span.end as usize], "129");
}

#[test]
fn compiler_rejects_f32_and_f64_literal_overflow() {
    for (source, literal) in [
        ("fn main() { return 3.5e38f32; }", "3.5e38f32"),
        ("fn main() { return 1.8e308f64; }", "1.8e308f64"),
    ] {
        let error = compile_function_source(SourceId::new(1), source, "main")
            .expect_err("non-finite float literal should fail");
        assert_eq!(
            error.kind,
            CompileErrorKind::InvalidFloatLiteral {
                literal: literal.to_owned(),
                error: "float literal out of range".to_owned(),
            }
        );
        let span = error.span.expect("literal error span");
        assert_eq!(&source[span.start as usize..span.end as usize], literal);
    }
}

#[test]
fn compiler_validates_deferred_dynamic_literal_range() {
    const SOURCE: &str = "fn main(value) { return value + 18446744073709551616; }";
    let error = compile_function_source(SourceId::new(1), SOURCE, "main")
        .expect_err("no numeric primitive can hold the deferred literal");

    assert_eq!(
        error.kind,
        CompileErrorKind::InvalidIntLiteral {
            literal: "18446744073709551616".to_owned(),
            error: "integer literal out of range".to_owned(),
        }
    );
    let span = error.span.expect("literal error span");
    assert_eq!(
        &SOURCE[span.start as usize..span.end as usize],
        "18446744073709551616"
    );
}
