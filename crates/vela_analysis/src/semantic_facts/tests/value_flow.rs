use vela_common::{SourceId, Span};
use vela_hir::module_graph::{ModuleGraph, ModuleSource};
use vela_package::{ModulePath, PackageId};

use crate::facts::AnalysisFacts;
use crate::semantic_facts::ControlFlowFact;
use crate::type_fact::TypeFact;

#[test]
fn reachable_block_values_and_control_flags_agree_at_abrupt_boundaries() {
    for (expression, expected, falls_through, returns) in [
        ("{ return; 3 }", TypeFact::Never, false, true),
        (
            "{ consume(({ return 1; 2 })); 3 }",
            TypeFact::Never,
            false,
            true,
        ),
        (
            "{ let value = ({ return 1; 2 }); 3 }",
            TypeFact::Never,
            false,
            true,
        ),
        (
            "{ flag && { return 1; false }; 3 }",
            TypeFact::I64,
            true,
            true,
        ),
        (
            "{ flag || { return 1; false }; 3 }",
            TypeFact::I64,
            true,
            true,
        ),
        (
            "{ let callback = || { return 1; }; 3 }",
            TypeFact::I64,
            true,
            false,
        ),
        (
            "{ for item in [1] { break; return 2; } 3 }",
            TypeFact::I64,
            true,
            false,
        ),
        (
            "{ for item in [1] { continue; return 2; } 3 }",
            TypeFact::I64,
            true,
            false,
        ),
        (
            "{ for item in ({ return 1; [1] }) { return 2; } 3 }",
            TypeFact::Never,
            false,
            true,
        ),
        (
            "{ if ({ return 1; true }) { 3 } else { 4 } }",
            TypeFact::Never,
            false,
            true,
        ),
    ] {
        let (facts, id) = analyze(expression);
        assert_eq!(facts.expression(id), Some(&expected), "{expression}");
        assert_eq!(
            facts.control_flow(id),
            Some(&ControlFlowFact {
                can_fallthrough: falls_through,
                may_return: returns,
                may_break: false,
                may_continue: false,
            }),
            "{expression}"
        );
    }
}

#[test]
fn lambda_returns_respect_argument_order_and_nested_invocation_boundaries() {
    for (expression, expected) in [
        (
            "(|| { pair(({ return 1; 2 }), ({ return \"unreachable\"; 0 })); false })()",
            TypeFact::I64,
        ),
        (
            "(|| { let callback = || { return \"separate\"; }; return 1; false })()",
            TypeFact::I64,
        ),
        ("(|| { return; 3 })()", TypeFact::UNIT),
    ] {
        let (facts, id) = analyze(expression);
        assert_eq!(facts.expression(id), Some(&expected), "{expression}");
    }
}

#[test]
fn match_results_and_control_flags_exclude_unreachable_arms() {
    for (expression, expected, normal, returns) in [
        (
            "match flag { _ => 1, true => \"unreachable\" }",
            TypeFact::I64,
            true,
            false,
        ),
        (
            "match flag { bound => 1, true => { return; } }",
            TypeFact::I64,
            true,
            false,
        ),
        (
            "match flag { _ => { return; }, true => 1 }",
            TypeFact::Never,
            false,
            true,
        ),
        (
            "match flag { _ if ({ return; true }) => 1, _ => 2 }",
            TypeFact::Never,
            false,
            true,
        ),
        (
            "match flag { true if ({ return; true }) => 1, _ => 2 }",
            TypeFact::I64,
            true,
            true,
        ),
        (
            "match flag { _ if flag => { return; }, _ => 1 }",
            TypeFact::I64,
            true,
            true,
        ),
        (
            "(|| { match flag { _ => 1, true => { return \"unreachable\"; } } })()",
            TypeFact::I64,
            true,
            false,
        ),
        (
            "(|| { match flag { _ if ({ return 1; true }) => \"unreachable\", _ => false } })()",
            TypeFact::I64,
            true,
            false,
        ),
    ] {
        let (facts, id) = analyze(expression);
        assert_eq!(facts.expression(id), Some(&expected), "{expression}");
        assert_eq!(
            facts.control_flow(id),
            Some(&ControlFlowFact {
                can_fallthrough: normal,
                may_return: returns,
                may_break: false,
                may_continue: false
            }),
            "{expression}"
        );
    }
}

fn analyze(expression: &str) -> (AnalysisFacts, vela_hir::ids::HirExprId) {
    let source = SourceId::new(99);
    let prefix = "fn consume(value) { value } fn pair(first, second) { first } fn main(flag: bool) { let probe = ";
    let text = format!("{prefix}{expression}; }}");
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        source,
        PackageId::anonymous(),
        ModulePath::from_qualified("flow"),
        text,
    ));
    graph.resolve_imports();
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let id = graph
        .expression_containing_span(Span::new(
            source,
            u32::try_from(prefix.len()).expect("prefix span"),
            u32::try_from(prefix.len() + expression.len()).expect("expression span"),
        ))
        .expect("probe expression");
    let facts = AnalysisFacts::from_module_graph(&graph);
    (facts, id)
}
