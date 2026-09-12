use std::collections::BTreeMap;

use crate::facts::AnalysisFacts;
use crate::semantic_facts::local_flow::{LocalFlow, body_local_environment};
use crate::type_fact::TypeFact;
use vela_common::{SourceId, Span};
use vela_hir::module_graph::{ModuleGraph, ModuleSource};
use vela_package::{ModulePath, PackageId};

#[test]
fn exhausted_loop_budget_erases_carried_refinements_and_keeps_local_resets() {
    for budget in [0, 1] {
        for (body_text, expected) in [
            (
                "for item in values { probe(value); value = \"later\"; }",
                TypeFact::Unknown,
            ),
            (
                "for item in values { value = \"reset\"; probe(value); value = 1; }",
                TypeFact::STRING,
            ),
            (
                "for outer in values { for inner in values { probe(value); value = \"later\"; } }",
                TypeFact::Unknown,
            ),
            (
                "for item in values { let other = value; probe(value); other = \"local\"; }",
                TypeFact::I64,
            ),
        ] {
            let source = SourceId::new(102);
            let text = format!(
                "fn main(values: Array) {{ let value = 1; {body_text} }} fn probe(arg) {{}}"
            );
            let mut graph = ModuleGraph::new();
            graph.add_source(ModuleSource::new(
                source,
                PackageId::anonymous(),
                ModulePath::from_qualified("flow"),
                text.clone(),
            ));
            graph.resolve_imports();
            assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
            let start = text.find("probe(value)").expect("probe") + "probe(".len();
            let id = graph
                .expression_at_span(Span::new(source, start as u32, (start + 5) as u32))
                .expect("value use");
            let body = graph
                .body_containing_offset(source, start as u32)
                .expect("main body");
            let base = AnalysisFacts::from_module_graph(&graph);
            let locals = base
                .locals()
                .map(|(id, value)| (id, value.clone()))
                .collect();
            let expression_types = base
                .expressions()
                .map(|(id, value)| (id, value.clone()))
                .collect();
            let source_origins = body
                .expressions
                .keys()
                .filter_map(|id| base.source_origins(*id).map(|value| (*id, value.clone())))
                .collect();
            let mut environment = body_local_environment(&locals, &BTreeMap::new(), body, &base);
            let mut flow = LocalFlow {
                graph: &graph,
                body,
                schema: None,
                base: &base,
                expression_types: &expression_types,
                source_origins: &source_origins,
                uses: BTreeMap::new(),
                loop_exits: Vec::new(),
                loop_passes_left: budget,
            };
            assert!(flow.visit_root(&mut environment));
            assert_eq!(flow.uses[&id].fact, expected, "{body_text}");
            assert_eq!(flow.loop_passes_left, 0);
            assert!(flow.loop_exits.is_empty());
        }
    }
}
