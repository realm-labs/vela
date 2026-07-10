mod constructor_targets;

use vela_common::SourceId;
use vela_hir::body::HirExprKind;
use vela_hir::ids::HirExprId;
use vela_hir::module_graph::{ModuleGraph, ModulePath, ModuleSource};

use super::MemberTargetFact;
use crate::facts::AnalysisFacts;
use crate::registry::RegistryFacts;
use crate::type_fact::TypeFact;

#[test]
fn runtime_record_members_are_dynamic_only_when_the_owner_schema_is_absent() {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SourceId::new(81),
        ModulePath::from_qualified("game"),
        r#"
        struct SourceRecord { present: i64 }

        fn inspect(source: SourceRecord) {
            fixture::opaque().missing;
            fixture::known().missing;
            source.missing;
        }
        "#,
    ));
    graph.resolve_imports();
    assert_eq!(graph.diagnostics(), &[]);

    let mut schema = RegistryFacts::default();
    schema.insert_function(
        "fixture::opaque",
        TypeFact::function(Vec::new(), TypeFact::record("OpaqueRecord")),
    );
    schema.insert_function(
        "fixture::known",
        TypeFact::function(Vec::new(), TypeFact::record("KnownRecord")),
    );
    schema.insert_type("KnownRecord", TypeFact::record("KnownRecord"));
    let facts = AnalysisFacts::from_module_graph_and_schema(&graph, &schema);

    let opaque = field_with_receiver_fact(&graph, &facts, &TypeFact::record("OpaqueRecord"));
    let known = field_with_receiver_fact(&graph, &facts, &TypeFact::record("KnownRecord"));
    let source = field_with_receiver_fact(&graph, &facts, &TypeFact::record("game::SourceRecord"));

    assert_eq!(
        facts.member_target(opaque),
        Some(&MemberTargetFact::Dynamic)
    );
    assert_eq!(
        facts.member_target(known),
        Some(&MemberTargetFact::Unresolved)
    );
    assert_eq!(
        facts.member_target(source),
        Some(&MemberTargetFact::Unresolved)
    );
}

#[test]
fn nested_try_preserves_a_statically_never_payload() {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SourceId::new(82),
        ModulePath::from_qualified("game"),
        "fn main() { let nested = fixture::failure()??; }",
    ));
    graph.resolve_imports();
    assert_eq!(graph.diagnostics(), &[]);

    let mut schema = RegistryFacts::default();
    schema.insert_function(
        "fixture::failure",
        TypeFact::function(
            Vec::new(),
            TypeFact::union([
                TypeFact::option_none(),
                TypeFact::result_err(TypeFact::STRING),
            ]),
        ),
    );
    let facts = AnalysisFacts::from_module_graph_and_schema(&graph, &schema);
    let body = graph
        .bodies()
        .find(|body| matches!(body.owner, vela_hir::body::HirBodyOwner::Declaration(_)))
        .expect("main body");
    let outer_try = body
        .expressions
        .values()
        .find(|expression| {
            let HirExprKind::Try {
                expression: Some(inner),
            } = expression.kind
            else {
                return false;
            };
            body.expressions
                .get(&inner)
                .is_some_and(|inner| matches!(inner.kind, HirExprKind::Try { .. }))
        })
        .expect("nested outer try");

    assert_eq!(facts.expression(outer_try.id), Some(&TypeFact::Never));
}

fn field_with_receiver_fact(
    graph: &ModuleGraph,
    facts: &AnalysisFacts,
    receiver_fact: &TypeFact,
) -> HirExprId {
    graph
        .bodies()
        .flat_map(|body| body.expressions.values())
        .find_map(|expression| {
            let HirExprKind::Field(field) = &expression.kind else {
                return None;
            };
            (facts.expression(field.receiver) == Some(receiver_fact)).then_some(expression.id)
        })
        .unwrap_or_else(|| panic!("field with receiver {receiver_fact:?}"))
}
