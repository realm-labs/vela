use vela_common::{PrimitiveTag, ScalarValue, Severity};
use vela_mir::{
    CompileGuardKey, CompileGuardTarget, MirEvaluatedConstant, MirGuardLocation, MirTypeContract,
};

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
        .type_by_name(&format!("{}::Reward", vela_package::PackageId::anonymous()))
        .expect("record descriptor");
    assert_eq!(reward.fields.len(), 1);
    assert!(
        reward
            .fields
            .iter()
            .all(|field| targets.field_descriptor(*field).is_some())
    );
    let state = targets
        .type_by_name(&format!("{}::State", vela_package::PackageId::anonymous()))
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
fn semantic_guards_retain_true_boundaries_indices_and_clean_names() {
    let fixture = prepare_source(
        r#"
struct Reward { amount: i64 }
global current: i64;
fn guarded(dynamic, required: i64, fallback: i64 = dynamic) -> i64 {
    let local: i64 = dynamic;
    return fallback;
}
"#,
        FixtureRoots::Program,
    )
    .expect("guard boundary metadata should be closed before MIR");
    let targets = fixture.input.targets();
    let function = targets
        .function_for_declaration(fixture.declarations["guarded"])
        .expect("guarded function descriptor");

    assert_guard_context(
        targets
            .guard(CompileGuardKey::Parameter {
                function,
                parameter: 1,
            })
            .expect("required parameter guard"),
        MirGuardLocation::Parameter { index: 1 },
        "required",
    );
    assert_guard_context(
        targets
            .guard(CompileGuardKey::Parameter {
                function,
                parameter: 2,
            })
            .expect("defaulted parameter guard"),
        MirGuardLocation::Parameter { index: 2 },
        "fallback",
    );
    assert_guard_context(
        targets
            .guard(CompileGuardKey::Return(function))
            .expect("return guard"),
        MirGuardLocation::Return,
        "return",
    );

    let global = fixture.declarations["current"];
    assert_guard_context(
        targets
            .guard(CompileGuardKey::Global(global))
            .expect("global guard"),
        MirGuardLocation::Global,
        "current",
    );
    let field = targets
        .type_by_name(&format!("{}::Reward", vela_package::PackageId::anonymous()))
        .expect("Reward descriptor")
        .fields[0];
    assert_guard_context(
        targets
            .guard(CompileGuardKey::Field(field))
            .expect("field guard"),
        MirGuardLocation::Field,
        "amount",
    );

    let function_targets = targets
        .function_targets(function)
        .expect("guarded function targets");
    let expression_guards = fixture
        .expression_sources
        .iter()
        .filter(|(_, _, source)| source == "dynamic")
        .filter_map(|(_, expression, _)| function_targets.expression_guard(*expression))
        .collect::<Vec<_>>();
    assert!(
        expression_guards.iter().any(|guard| {
            guard.context.location == MirGuardLocation::Parameter { index: 2 }
                && guard.context.debug_name == "fallback"
        }),
        "the parameter-default guard must retain its owning signature index"
    );
    assert!(
        expression_guards.iter().any(|guard| {
            guard.context.location == MirGuardLocation::Local && guard.context.debug_name == "local"
        }),
        "the typed-let guard must retain its local boundary"
    );
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
        .type_by_name(&format!("{}::Reward", vela_package::PackageId::anonymous()))
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
fn used_nonconstant_schema_default_is_a_source_spanned_validation_diagnostic() {
    let error = prepare_source(
        r#"
struct Reward { amount: i64 = math::random() }
fn main() { return Reward {}; }
"#,
        FixtureRoots::Program,
    )
    .expect_err("a used runtime schema default must be rejected");

    let CompileErrorKind::SemanticDiagnostics(diagnostics) = error.kind else {
        panic!("expected semantic diagnostics, got {error:?}");
    };
    let [diagnostic] = diagnostics.as_slice() else {
        panic!("expected one diagnostic, got {diagnostics:?}");
    };
    assert_eq!(diagnostic.severity, Severity::Error);
    assert_eq!(
        diagnostic.code.as_deref(),
        Some("compiler::non_constant_schema_default")
    );
    assert_eq!(
        diagnostic.message,
        "schema field default must be compile-time evaluable"
    );
    assert!(diagnostic.span.is_some());
    assert_eq!(diagnostic.labels.len(), 1);
    assert_eq!(diagnostic.labels[0].span, diagnostic.span.expect("span"));
}

fn assert_guard_context(guard: &CompileGuardTarget, location: MirGuardLocation, debug_name: &str) {
    assert_eq!(guard.context.location, location);
    assert_eq!(guard.context.debug_name, debug_name);
}
