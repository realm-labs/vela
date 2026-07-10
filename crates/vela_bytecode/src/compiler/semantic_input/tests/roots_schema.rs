use vela_common::{PrimitiveTag, ScalarValue};
use vela_mir::{CompileGuardKey, MirEvaluatedConstant, MirTypeContract};

use super::{FixtureRoots, prepare_source};
use crate::compiler::error::CompileErrorKind;

#[test]
fn function_roots_exclude_unselected_body_diagnostics_and_targets() {
    let fixture = prepare_source(
        r#"
fn selected() -> i64 { return 1; }
fn ignored() -> i64 { return math::max("wrong", 2); }
"#,
        FixtureRoots::Function("selected"),
    )
    .expect("an unselected function must not leak body diagnostics");
    let targets = fixture.input.targets();
    let selected = fixture.declarations["selected"];
    let ignored = fixture.declarations["ignored"];
    let selected_function = targets
        .function_for_declaration(selected)
        .expect("selected function descriptor");
    let ignored_function = targets
        .function_for_declaration(ignored)
        .expect("unselected function descriptor");
    let roots = targets
        .compilation_roots()
        .map(|(function, _)| function)
        .collect::<Vec<_>>();

    assert_eq!(roots, [selected_function]);
    assert!(!roots.contains(&ignored_function));
    assert!(fixture.input.analysis().view(selected_function).is_some());
    assert!(fixture.input.analysis().view(ignored_function).is_none());
}

#[test]
fn schema_only_program_has_closed_descriptors_without_runtime_roots() {
    let fixture = prepare_source(
        r#"
struct Reward { amount: i64 }
enum State { Ready(value: i64), Idle }
global current: Any;
"#,
        FixtureRoots::Program,
    )
    .expect("schema-only semantic input should build");
    let targets = fixture.input.targets();

    assert_eq!(targets.compilation_roots().count(), 0);
    let reward = targets
        .type_by_name("script::Reward")
        .expect("record descriptor");
    assert_eq!(reward.fields.len(), 1);
    assert!(
        reward
            .fields
            .iter()
            .all(|field| targets.field_descriptor(*field).is_some())
    );
    let state = targets
        .type_by_name("script::State")
        .expect("enum descriptor");
    assert_eq!(state.variants.len(), 2);
    for variant in &state.variants {
        let variant = targets
            .variant_descriptor(*variant)
            .expect("variant edge must resolve");
        assert!(
            variant
                .fields
                .iter()
                .all(|field| targets.field_descriptor(*field).is_some())
        );
    }

    let global = fixture.declarations["current"];
    assert_eq!(
        targets.global(global).map(|global| &global.contract),
        Some(&MirTypeContract::Any)
    );
    assert!(targets.guard(CompileGuardKey::Global(global)).is_none());
}

#[test]
fn schema_defaults_reuse_the_authoritative_compile_time_value() {
    let fixture = prepare_source(
        r#"
const BASE: i64 = 2;
struct Reward { amount: i64 = BASE + 1 }
"#,
        FixtureRoots::Program,
    )
    .expect("constant schema default should build");
    let body = fixture.schema_default_bodies[0];

    assert_eq!(
        fixture.input.targets().evaluated_schema_default(body),
        Some(&MirEvaluatedConstant::Scalar(ScalarValue::I64(3)))
    );
    let field = fixture
        .input
        .targets()
        .type_by_name("script::Reward")
        .expect("record descriptor")
        .fields[0];
    assert_eq!(
        fixture
            .input
            .targets()
            .field_descriptor(field)
            .and_then(|field| field.contract.as_ref()),
        Some(&MirTypeContract::Primitive(PrimitiveTag::I64))
    );
}

#[test]
fn used_nonconstant_schema_default_keeps_the_frozen_error() {
    let error = prepare_source(
        r#"
struct Reward { amount: i64 = math::random() }
fn main() { return Reward {}; }
"#,
        FixtureRoots::Program,
    )
    .expect_err("a used runtime schema default must be rejected");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("non-constant schema default expression")
    ));
    assert!(error.span.is_some());
}
