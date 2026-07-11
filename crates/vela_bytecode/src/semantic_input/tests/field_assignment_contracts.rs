use vela_analysis::literals::ResolvedLiteralFact;
use vela_common::{PrimitiveTag, ScalarValue};
use vela_mir::MirTypeContract;

use super::{FixtureRoots, SemanticFixture, prepare_source};
use crate::compiler::error::CompileErrorKind;

#[test]
fn script_field_assignment_contextualizes_unsuffixed_numeric_rhs() {
    let fixture = prepare_source(
        r#"
struct Reward { amount: u8 }
fn main() {
    let reward = Reward { amount: 0u8 };
    reward.amount = 255;
}
"#,
        FixtureRoots::Program,
    )
    .expect("typed script-field assignment input");
    let function = only_function(&fixture);
    let rhs = expression_exact(&fixture, "255");
    let analysis = fixture
        .input
        .analysis()
        .view(function)
        .expect("main executable analysis");

    assert_eq!(
        analysis
            .literal(rhs)
            .and_then(|result| result.as_ref().ok())
            .and_then(ResolvedLiteralFact::scalar),
        Some(ScalarValue::U8(255))
    );
    assert!(
        fixture
            .input
            .targets()
            .function_targets(function)
            .expect("main targets")
            .expression_guard(rhs)
            .is_none(),
        "a contextualized literal is statically proven"
    );
}

#[test]
fn script_field_assignment_records_dynamic_rhs_guard() {
    let fixture = prepare_source(
        r#"
struct Reward { amount: i8 }
fn main(dynamic) {
    let reward = Reward { amount: 0i8 };
    reward.amount = dynamic;
}
"#,
        FixtureRoots::Program,
    )
    .expect("dynamic script-field assignment input");
    let function = only_function(&fixture);
    let rhs = expression_exact(&fixture, "dynamic");
    let guard = fixture
        .input
        .targets()
        .function_targets(function)
        .expect("main targets")
        .expression_guard(rhs)
        .expect("dynamic field RHS guard");

    assert_eq!(guard.contract, MirTypeContract::Primitive(PrimitiveTag::I8));
    assert_eq!(guard.context.location, vela_mir::MirGuardLocation::Field);
    assert_eq!(guard.context.debug_name, "amount");
}

#[test]
fn script_field_assignment_rejects_static_rhs_mismatch_before_mir() {
    const SOURCE: &str = r#"
struct Reward { amount: i8 }
fn main() {
    let reward = Reward { amount: 0i8 };
    reward.amount = "bad";
}
"#;
    let error = prepare_source(SOURCE, FixtureRoots::Program)
        .expect_err("incompatible field assignment must fail semantic input");
    let CompileErrorKind::SemanticDiagnostics(diagnostics) = error.kind else {
        panic!("expected semantic diagnostics, got {error:?}");
    };
    let [diagnostic] = diagnostics.as_slice() else {
        panic!("expected one field mismatch, got {diagnostics:?}");
    };
    assert_eq!(
        diagnostic.code.as_deref(),
        Some("compiler::type_contract_mismatch")
    );
    assert_eq!(
        diagnostic.message,
        "type contract mismatch for field `amount`"
    );
    let span = diagnostic.span.expect("field assignment RHS span");
    assert_eq!(&SOURCE[span.start as usize..span.end as usize], "\"bad\"");
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
