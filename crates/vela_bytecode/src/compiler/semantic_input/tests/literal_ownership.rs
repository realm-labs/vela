use vela_analysis::literals::{NumericLiteralKind, ResolvedLiteralFact};
use vela_common::ScalarValue;

use super::{FixtureRoots, SemanticFixture, prepare_source};
use crate::compiler::error::CompileErrorKind;

#[test]
fn semantic_input_literal_contexts_match_frozen_direct_operator_shapes() {
    let fixture = prepare_source(
        r#"
fn main(value) {
    let positive = value + 1;
    let parenthesized = value + (2);
    let negated = value + -3;
    let equality = value == 4;
    return positive;
}
"#,
        FixtureRoots::Function("main"),
    )
    .expect("literal-qualified semantic input");
    let function = only_function(&fixture);
    let analysis = fixture
        .input
        .analysis()
        .view(function)
        .expect("main executable analysis");

    for source in ["1", "2"] {
        let expression = expression_exact(&fixture, source);
        assert!(
            matches!(
                analysis.literal(expression),
                Some(Ok(ResolvedLiteralFact::Deferred(literal)))
                    if literal.kind() == NumericLiteralKind::Integer
            ),
            "literal `{source}` resolved as {:?}",
            analysis.literal(expression)
        );
    }

    let negated = expression_exact(&fixture, "-3");
    assert_eq!(
        analysis
            .literal(negated)
            .and_then(|result| result.as_ref().ok())
            .and_then(ResolvedLiteralFact::scalar),
        Some(ScalarValue::I64(-3))
    );
    let equality = expression_exact(&fixture, "4");
    assert_eq!(
        analysis
            .literal(equality)
            .and_then(|result| result.as_ref().ok())
            .and_then(ResolvedLiteralFact::scalar),
        Some(ScalarValue::I64(4))
    );
}

#[test]
fn semantic_input_contextualizes_inline_literals_from_numeric_peers() {
    let fixture = prepare_source(
        r#"
fn main(left: i8, right: i8) {
    let negated = left + -128;
    let parenthesized = left + (127);
    let left_sided = ((-126)) + right;
    let equality = left == 125;
    return negated;
}
"#,
        FixtureRoots::Function("main"),
    )
    .expect("typed binary literals should use the proven peer primitive");
    let function = only_function(&fixture);
    let analysis = fixture
        .input
        .analysis()
        .view(function)
        .expect("main executable analysis");

    for (source, expected) in [
        ("-128", ScalarValue::I8(i8::MIN)),
        ("127", ScalarValue::I8(127)),
        ("-126", ScalarValue::I8(-126)),
    ] {
        let expression = expression_exact(&fixture, source);
        assert_eq!(
            analysis
                .literal(expression)
                .and_then(|result| result.as_ref().ok())
                .and_then(ResolvedLiteralFact::scalar),
            Some(expected),
            "literal `{source}` did not use its i8 peer"
        );
    }

    let equality = expression_exact(&fixture, "125");
    assert_eq!(
        analysis
            .literal(equality)
            .and_then(|result| result.as_ref().ok())
            .and_then(ResolvedLiteralFact::scalar),
        Some(ScalarValue::I64(125)),
        "operators outside the frozen contextual-literal family stay defaulted"
    );
}

#[test]
fn semantic_input_rejects_typed_binary_literal_overflow_before_lowering() {
    const SOURCE: &str = "fn main(value: i8) { return value + -129; }";
    let error = prepare_source(SOURCE, FixtureRoots::Function("main"))
        .expect_err("typed binary literal overflow must fail semantic input");
    let CompileErrorKind::SemanticDiagnostics(diagnostics) = error.kind else {
        panic!("expected semantic diagnostics, got {error:?}");
    };
    let [diagnostic] = diagnostics.as_slice() else {
        panic!("expected one typed literal diagnostic, got {diagnostics:?}");
    };
    assert_eq!(
        diagnostic.code.as_deref(),
        Some("compiler::invalid_int_literal")
    );
    assert_eq!(
        diagnostic.message,
        "invalid integer literal `129`: integer literal out of range"
    );
    let span = diagnostic.span.expect("typed binary literal span");
    assert_eq!(&SOURCE[span.start as usize..span.end as usize], "129");
}

#[test]
fn semantic_input_rejects_invalid_pattern_literals_before_target_placement() {
    const SOURCE: &str = r#"
fn target(value) { return value; }
fn main(value) {
    return match value {
        128i8 => target(missing = 1),
        _ => 0,
    };
}
"#;
    let error = prepare_source(SOURCE, FixtureRoots::Program)
        .expect_err("invalid pattern literal must fail semantic input");
    let CompileErrorKind::SemanticDiagnostics(diagnostics) = error.kind else {
        panic!("expected semantic diagnostics, got {error:?}");
    };
    let [diagnostic] = diagnostics.as_slice() else {
        panic!("literal validation must reject before call placement: {diagnostics:?}");
    };
    assert_eq!(
        diagnostic.code.as_deref(),
        Some("compiler::invalid_int_literal")
    );
    assert_eq!(
        diagnostic.message,
        "invalid integer literal `128i8`: integer literal out of range"
    );
    let span = diagnostic.span.expect("pattern literal span");
    assert_eq!(&SOURCE[span.start as usize..span.end as usize], "128i8");
}

fn only_function(fixture: &SemanticFixture) -> vela_def::FunctionId {
    let functions = fixture.input.analysis().functions().collect::<Vec<_>>();
    let [function] = functions.as_slice() else {
        panic!("expected one selected executable, got {functions:?}");
    };
    *function
}

fn expression_exact(fixture: &SemanticFixture, source: &str) -> vela_hir::ids::HirExprId {
    let expressions = fixture
        .expression_sources
        .iter()
        .filter_map(|(_, expression, text)| (text == source).then_some(*expression))
        .collect::<Vec<_>>();
    let [expression] = expressions.as_slice() else {
        panic!("expected one expression `{source}`, got {expressions:?}");
    };
    *expression
}
