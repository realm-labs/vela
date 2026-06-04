use vela_analysis::diagnostics::match_patterns::match_pattern_diagnostics;
use vela_analysis::diagnostics::member::member_access_diagnostics;
use vela_analysis::expression::ExprFactScope;
use vela_analysis::registry::RegistryFacts;
use vela_analysis::type_fact::TypeFact;
use vela_common::diagnostic_render::{DiagnosticSource, render_diagnostic};
use vela_common::{FieldId, SourceId, TypeId};
use vela_reflect::registry::{FieldDesc, TypeDesc, TypeKey, TypeRegistry};
use vela_syntax::ast::{Expr, ItemKind, StmtKind};
use vela_syntax::parser::parse_source;

const UNKNOWN_HOST_FIELD: &str =
    include_str!("../../../tests/fixtures/diagnostics/unknown_host_field.vela");
const UNKNOWN_HOST_FIELD_EXPECTED: &str =
    include_str!("../../../tests/fixtures/diagnostics/unknown_host_field.expected");
const TYPEFACT_UNKNOWN_OPTION_VARIANT: &str =
    include_str!("../../../tests/fixtures/diagnostics/typefact_unknown_option_variant.vela");
const TYPEFACT_UNKNOWN_OPTION_VARIANT_EXPECTED: &str =
    include_str!("../../../tests/fixtures/diagnostics/typefact_unknown_option_variant.expected");

#[test]
fn semantic_unknown_host_field_fixture_renders_candidates_and_access_hints() {
    let expr = first_expression(UNKNOWN_HOST_FIELD);
    let scope = ExprFactScope::new().with_path(["player"], TypeFact::host("Player"));
    let diagnostics = member_access_diagnostics(&expr, &scope, &registry_facts());

    assert_eq!(diagnostics.len(), 1);
    let rendered = render_diagnostic(
        &diagnostics[0],
        [DiagnosticSource::new(
            SourceId::new(1),
            "unknown_host_field.vela",
            UNKNOWN_HOST_FIELD,
        )],
    )
    .join("\n");

    assert_eq!(rendered.trim_end(), UNKNOWN_HOST_FIELD_EXPECTED.trim_end());
}

#[test]
fn typefact_unknown_option_variant_fixture_renders_dynamic_candidates() {
    let expr = first_expression(TYPEFACT_UNKNOWN_OPTION_VARIANT);
    let scope = ExprFactScope::new().with_path(["maybe"], TypeFact::option(TypeFact::Int));
    let diagnostics = match_pattern_diagnostics(&expr, &scope, &RegistryFacts::default());

    assert_eq!(diagnostics.len(), 1);
    let rendered = render_diagnostic(
        &diagnostics[0],
        [DiagnosticSource::new(
            SourceId::new(1),
            "typefact_unknown_option_variant.vela",
            TYPEFACT_UNKNOWN_OPTION_VARIANT,
        )],
    )
    .join("\n");

    assert_eq!(
        rendered.trim_end(),
        TYPEFACT_UNKNOWN_OPTION_VARIANT_EXPECTED.trim_end()
    );
}

fn registry_facts() -> RegistryFacts {
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
            .field(
                FieldDesc::new(FieldId::new(1), "level")
                    .type_hint("int")
                    .writable(true),
            )
            .field(FieldDesc::new(FieldId::new(2), "inventory").type_hint("map")),
    );
    RegistryFacts::from_registry(&registry)
}

fn first_expression(source: &str) -> Expr {
    let parsed = parse_source(SourceId::new(1), source);
    assert_eq!(parsed.diagnostics, []);
    let function = parsed
        .items
        .iter()
        .find_map(|item| match &item.kind {
            ItemKind::Function(function) => Some(function),
            _ => None,
        })
        .expect("fixture should contain a function");

    function
        .body
        .statements
        .iter()
        .find_map(|statement| match &statement.kind {
            StmtKind::Expr(expr) => Some(expr.clone()),
            StmtKind::Let {
                value: Some(expr), ..
            } => Some(expr.clone()),
            _ => None,
        })
        .expect("fixture should contain an expression")
}
