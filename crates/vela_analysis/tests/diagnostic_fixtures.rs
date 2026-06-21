use vela_analysis::diagnostics::member::member_access_diagnostics;
use vela_analysis::expression::ExprFactScope;
use vela_analysis::registry::RegistryFacts;
use vela_analysis::type_fact::TypeFact;
use vela_common::SourceId;
use vela_common::diagnostic_render::{DiagnosticSource, render_diagnostic};
use vela_def::{FieldId, TypeId};
use vela_reflect::registry::{FieldDesc, TypeDesc, TypeKey, TypeRegistry};
use vela_syntax::ast::{Expr, ItemKind, StmtKind};
use vela_syntax::parser::parse_source;

const UNKNOWN_HOST_FIELD: &str =
    include_str!("../../../tests/fixtures/diagnostics/unknown_host_field.vela");
const UNKNOWN_HOST_FIELD_EXPECTED: &str =
    include_str!("../../../tests/fixtures/diagnostics/unknown_host_field.expected");
const FLOW_NARROWING_NULL_MEMBER: &str =
    include_str!("../../../tests/fixtures/diagnostics/flow_narrowing_null_member.vela");
const FLOW_NARROWING_NULL_MEMBER_EXPECTED: &str =
    include_str!("../../../tests/fixtures/diagnostics/flow_narrowing_null_member.expected");

#[test]
fn semantic_unknown_host_field_fixture_renders_candidates_and_access_hints() {
    let source = normalized_fixture(UNKNOWN_HOST_FIELD);
    let expr = first_expression(&source);
    let scope = ExprFactScope::new().with_path(["player"], TypeFact::host("Player"));
    let diagnostics = member_access_diagnostics(&expr, &scope, &registry_facts());

    assert_eq!(diagnostics.len(), 1);
    let rendered = render_diagnostic(
        &diagnostics[0],
        [diagnostic_source("unknown_host_field.vela", source)],
    )
    .join("\n");

    assert_rendered_eq(&rendered, UNKNOWN_HOST_FIELD_EXPECTED);
}

#[test]
fn flow_narrowing_null_check_fixture_renders_member_diagnostic() {
    let source = normalized_fixture(FLOW_NARROWING_NULL_MEMBER);
    let expr = first_expression(&source);
    let scope = ExprFactScope::new().with_path(
        ["player"],
        TypeFact::union([TypeFact::NULL, TypeFact::host("Player")]),
    );
    let diagnostics = member_access_diagnostics(&expr, &scope, &registry_facts());

    assert_eq!(diagnostics.len(), 1);
    let rendered = render_diagnostic(
        &diagnostics[0],
        [diagnostic_source("flow_narrowing_null_member.vela", source)],
    )
    .join("\n");

    assert_rendered_eq(&rendered, FLOW_NARROWING_NULL_MEMBER_EXPECTED);
}

fn registry_facts() -> RegistryFacts {
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
            .field(
                FieldDesc::new(FieldId::new(1), "level")
                    .type_hint("i64")
                    .writable(true),
            )
            .field(FieldDesc::new(FieldId::new(2), "inventory").type_hint("Map")),
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

fn diagnostic_source(name: &str, source: String) -> DiagnosticSource {
    DiagnosticSource::new(SourceId::new(1), name, source)
}

fn normalized_fixture(source: &str) -> String {
    source.replace("\r\n", "\n")
}

fn assert_rendered_eq(rendered: &str, expected: &str) {
    assert_eq!(rendered.trim_end(), normalized_fixture(expected).trim_end());
}
