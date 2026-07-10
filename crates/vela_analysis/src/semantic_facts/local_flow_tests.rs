use vela_common::{SourceId, Span};
use vela_hir::body::HirExprKind;
use vela_hir::ids::HirExprId;
use vela_hir::module_graph::{ModuleGraph, ModulePath, ModuleSource};

use super::CallTargetFact;
use crate::facts::AnalysisFacts;
use crate::registry::RegistryFacts;
use crate::type_fact::TypeFact;

#[test]
fn local_use_facts_track_straight_line_writes_and_invalidate_control_flow_joins() {
    let source = SourceId::new(84);
    let text = r#"
fn main() {
    let straight = 1;
    straight.trim();
    straight = "ready:gold";
    straight.contains(needle = ":");

    let looped = 1;
    for item in [] { looped = "ready:gold"; }
    looped.contains(needle = ":");

    let matched = 1;
    match matched {
        1 => { matched = "ready:gold"; }
        _ => {}
    }
    matched.contains(needle = ":");

    let shorted = 1;
    true || { shorted = "ready:gold"; true };
    shorted.contains(needle = ":");

    let interpolated = 1;
    let display = f"{interpolated = "ready:gold"}";
    interpolated.contains(needle = ":");

    42.trim();
    42.touch(arg = 0);
}
"#;
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        source,
        ModulePath::from_qualified("game"),
        text,
    ));
    graph.resolve_imports();
    assert_eq!(graph.diagnostics(), &[]);

    let mut schema = RegistryFacts::default();
    schema.insert_type("I64", TypeFact::I64);
    schema.insert_type("String", TypeFact::STRING);
    schema.insert_method(
        "I64",
        "touch",
        TypeFact::function(vec![TypeFact::I64], TypeFact::BOOL),
    );
    let facts = AnalysisFacts::from_module_graph_and_schema(&graph, &schema);

    let first = expression_exact(&graph, source, text, "straight.trim()");
    let straight = expression_exact(&graph, source, text, "straight.contains(needle = \":\")");
    let looped = expression_exact(&graph, source, text, "looped.contains(needle = \":\")");
    let matched = expression_exact(&graph, source, text, "matched.contains(needle = \":\")");
    let shorted = expression_exact(&graph, source, text, "shorted.contains(needle = \":\")");
    let interpolated = expression_exact(
        &graph,
        source,
        text,
        "interpolated.contains(needle = \":\")",
    );
    let literal_miss = expression_exact(&graph, source, text, "42.trim()");
    let literal_registry = expression_exact(&graph, source, text, "42.touch(arg = 0)");

    assert_eq!(receiver_fact(&graph, &facts, first), Some(&TypeFact::I64));
    assert_eq!(
        receiver_fact(&graph, &facts, straight),
        Some(&TypeFact::STRING)
    );
    assert_eq!(receiver_fact(&graph, &facts, looped), None);
    assert_eq!(receiver_fact(&graph, &facts, matched), None);
    assert_eq!(receiver_fact(&graph, &facts, shorted), None);
    assert_eq!(
        receiver_fact(&graph, &facts, interpolated),
        Some(&TypeFact::STRING)
    );
    assert!(matches!(
        facts.call_target(first),
        Some(CallTargetFact::KnownReceiverMiss {
            receiver: TypeFact::Primitive(_),
            method,
            ..
        }) if method == "trim"
    ));
    assert!(matches!(
        facts.call_target(straight),
        Some(CallTargetFact::StdlibMethod { name }) if name == "contains"
    ));
    assert_eq!(facts.call_target(looped), Some(&CallTargetFact::Dynamic));
    assert_eq!(facts.call_target(matched), Some(&CallTargetFact::Dynamic));
    assert_eq!(facts.call_target(shorted), Some(&CallTargetFact::Dynamic));
    assert!(matches!(
        facts.call_target(interpolated),
        Some(CallTargetFact::StdlibMethod { name }) if name == "contains"
    ));
    assert!(matches!(
        facts.call_target(literal_miss),
        Some(CallTargetFact::KnownReceiverMiss { method, .. }) if method == "trim"
    ));
    assert!(matches!(
        facts.call_target(literal_registry),
        Some(CallTargetFact::RegistryMethod { owner, name })
            if owner == "I64" && name == "touch"
    ));
}

fn receiver_fact<'a>(
    graph: &ModuleGraph,
    facts: &'a AnalysisFacts,
    call: HirExprId,
) -> Option<&'a TypeFact> {
    let body = graph
        .bodies()
        .find(|body| body.expressions.contains_key(&call))?;
    let call = body.call(call)?;
    let field = body.field(call.callee)?;
    facts.expression(field.receiver)
}

fn expression_exact(
    graph: &ModuleGraph,
    source: SourceId,
    text: &str,
    expression: &str,
) -> HirExprId {
    let start = text.find(expression).expect("expression source offset");
    let id = graph
        .expression_at_span(Span::new(
            source,
            u32::try_from(start).expect("expression start"),
            u32::try_from(start + expression.len()).expect("expression end"),
        ))
        .expect("expression at source span");
    assert!(matches!(
        graph
            .bodies()
            .find_map(|body| body.expressions.get(&id))
            .map(|expression| &expression.kind),
        Some(HirExprKind::Call(_))
    ));
    id
}
