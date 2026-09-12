use crate::{facts::AnalysisFacts, type_fact::TypeFact};
use vela_common::{SourceId, Span};
use vela_hir::module_graph::{ModuleGraph, ModuleSource};
use vela_package::{ModulePath, PackageId};

#[test]
fn loop_uses_include_later_iterations_but_exclude_terminal_exits() {
    for (code, expected) in [
        (
            "for item in values { probe(value); value = \"later\"; }",
            None,
        ),
        (
            "for item in values { probe(value); value = \"later\"; continue; }",
            None,
        ),
        (
            "for item in values { probe(value); if flag { value = \"later\"; continue; } }",
            None,
        ),
        (
            "for item in values { probe(value); value = \"later\"; break; }",
            Some(TypeFact::I64),
        ),
        (
            "for item in values { probe(value); value = \"later\"; return; }",
            Some(TypeFact::I64),
        ),
        (
            "for item in values { value = \"reset\"; probe(value); value = 1; }",
            Some(TypeFact::STRING),
        ),
        (
            "let next = 1; for item in values { probe(value); value = next; next = \"later\"; }",
            None,
        ),
        (
            "for outer in values { for inner in values { probe(value); value = \"later\"; } }",
            None,
        ),
        (
            "for outer in values { probe(value); for inner in values { value = \"later\"; break; } }",
            None,
        ),
        (
            "for outer in values { probe(value); for inner in { value = \"later\"; continue; values } {} }",
            None,
        ),
        (
            "for outer in values { probe(value); for inner in { value = \"later\"; break; values } {} }",
            Some(TypeFact::I64),
        ),
        (
            "for value in [1, 2] { probe(value); value = \"local\"; }",
            Some(TypeFact::I64),
        ),
        (
            "for item in values { let value = 1; probe(value); value = \"local\"; }",
            Some(TypeFact::I64),
        ),
    ] {
        let source = SourceId::new(101);
        let text = format!(
            "fn main(flag: bool, values: Array) {{ let value = 1; {code} }} fn probe(arg) {{}}"
        );
        let mut graph = ModuleGraph::new();
        graph.add_source(ModuleSource::new(
            source,
            PackageId::anonymous(),
            ModulePath::from_qualified("flow"),
            text.clone(),
        ));
        graph.resolve_imports();
        assert!(
            graph.diagnostics().is_empty(),
            "{code}: {:?}",
            graph.diagnostics()
        );
        let start = text.find("probe(value)").expect("probe") + "probe(".len();
        let id = graph
            .expression_at_span(Span::new(source, start as u32, (start + 5) as u32))
            .expect("value use");
        let facts = AnalysisFacts::from_module_graph(&graph);
        assert_eq!(facts.expression(id).cloned(), expected, "{code}");
    }
}
