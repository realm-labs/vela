use std::collections::BTreeMap;

use vela_common::SourceId;
use vela_hir::body::{HirBinaryOp, HirExprKind};
use vela_hir::module_graph::{ModuleGraph, ModulePath, ModuleSource};

use crate::facts::AnalysisFacts;
use crate::semantic_facts::OperatorTargetFact;

#[test]
fn unknown_and_any_operands_remain_dynamic_while_typed_operands_are_proven() {
    let source = SourceId::new(94);
    let text = r#"
fn main(unknown, dynamic: Any, typed: i64) {
    let from_unknown = unknown + 1;
    let from_dynamic = dynamic + 2;
    let from_typed = typed + 3;
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

    let facts = AnalysisFacts::from_module_graph(&graph);
    let targets = graph
        .bodies()
        .flat_map(|body| body.expressions.values())
        .filter_map(|expression| {
            let HirExprKind::Binary {
                op: Some(HirBinaryOp::Add),
                ..
            } = expression.kind
            else {
                return None;
            };
            let span = expression.origin.span;
            Some((
                text[span.start as usize..span.end as usize].to_owned(),
                facts.operator_target(expression.id),
            ))
        })
        .collect::<BTreeMap<_, _>>();

    assert_eq!(targets["unknown + 1"], Some(OperatorTargetFact::Dynamic));
    assert_eq!(targets["dynamic + 2"], Some(OperatorTargetFact::Dynamic));
    assert_eq!(
        targets["typed + 3"],
        Some(OperatorTargetFact::Binary(HirBinaryOp::Add))
    );
}
