use crate::{facts::AnalysisFacts, type_fact::TypeFact};
use vela_common::{SourceId, Span};
use vela_hir::module_graph::{ModuleGraph, ModuleSource};
use vela_package::{ModulePath, PackageId};

#[test]
fn local_exit_facts_keep_only_reachable_successors() {
    for (statements, expected) in [
        (
            "if flag { if ({ value = \"exit\"; return; true }) { value = false; } }",
            Some(TypeFact::I64),
        ),
        ("if flag { value = \"exit\"; return; }", Some(TypeFact::I64)),
        (
            "if flag { value = \"kept\"; } else { return; }",
            Some(TypeFact::STRING),
        ),
        (
            "if flag { { value = \"exit\"; return; } value = false; }",
            Some(TypeFact::I64),
        ),
        (
            "for item in [1] { value = \"exit\"; return; }",
            Some(TypeFact::I64),
        ),
        (
            "for item in [1] { break; value = \"tail\"; }",
            Some(TypeFact::I64),
        ),
        (
            "for item in [1] { continue; value = \"tail\"; }",
            Some(TypeFact::I64),
        ),
        (
            "for item in [1] { value = \"kept\"; break; value = 1; }",
            None,
        ),
        (
            "for item in [1] { value = \"kept\"; continue; value = 1; }",
            None,
        ),
        (
            "for item in [1] { if flag { value = \"kept\"; break; } value = 1; }",
            None,
        ),
        (
            "for item in [1] { if flag { value = \"kept\"; continue; } value = 1; }",
            None,
        ),
        (
            "for outer in [1] { for inner in [1] { break; } value = 1; }",
            Some(TypeFact::I64),
        ),
        (
            "for outer in [1] { for inner in { break; [1] } {} value = \"tail\"; }",
            Some(TypeFact::I64),
        ),
        (
            "for outer in [1] { for inner in { continue; [1] } {} value = \"tail\"; }",
            Some(TypeFact::I64),
        ),
        (
            "flag && { value = \"exit\"; return; true };",
            Some(TypeFact::I64),
        ),
        (
            "flag || { value = \"exit\"; return; false };",
            Some(TypeFact::I64),
        ),
        (
            "match flag { true => { return; } _ => { value = \"kept\"; } }",
            Some(TypeFact::STRING),
        ),
        (
            "match flag { true => { return; } other => { value = \"kept\"; } }",
            Some(TypeFact::STRING),
        ),
        (
            "match flag { _ if { value = \"kept\"; flag } => { return; } _ => {} }",
            Some(TypeFact::STRING),
        ),
        (
            "match flag { true if { value = \"exit\"; return; true } => {} _ => {} }",
            Some(TypeFact::I64),
        ),
        (
            "match flag { _ => {} true => { value = \"unreachable\"; } }",
            Some(TypeFact::I64),
        ),
        (
            "let callback = || { value = \"separate\"; return; };",
            Some(TypeFact::I64),
        ),
        (
            "if flag { pair({ return; 0 }, { value = \"tail\"; 0 }); }",
            Some(TypeFact::I64),
        ),
        (
            "for item in [1] { pair({ value = \"kept\"; break; 0 }, { value = 1; 0 }); }",
            None,
        ),
        (
            "for item in [1] { [({ break; 0 }), ({ value = \"tail\"; 0 })]; }",
            Some(TypeFact::I64),
        ),
    ] {
        let source = SourceId::new(100);
        let text = format!(
            "fn pair(first, second) {{ first }} fn main(flag: bool) {{ let value = 1; {statements} probe(value); }} fn probe(arg) {{}}"
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
            "{statements}: {:?}",
            graph.diagnostics()
        );
        let start = text.find("probe(value)").expect("probe") + "probe(".len();
        let id = graph
            .expression_at_span(Span::new(
                source,
                start as u32,
                (start + "value".len()) as u32,
            ))
            .expect("value use");
        let facts = AnalysisFacts::from_module_graph(&graph);
        assert_eq!(facts.expression(id).cloned(), expected, "{statements}");
    }
}
