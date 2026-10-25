use vela_common::SourceId;
use vela_hir::module_graph::{ModuleGraph, ModuleSource};
use vela_package::{ModulePath, PackageId};

use crate::facts::AnalysisFacts;
use crate::hints::{type_fact_from_hint, type_fact_from_path};
use crate::registry::RegistryEffectFact;
use crate::semantic_facts::CallTargetFact;
use crate::type_fact::TypeFact;

#[test]
fn removed_async_syntax_is_rejected_before_executable_analysis() {
    for text in [
        "async fn worker() -> i64 { 1 }",
        "fn main() { worker().await; }",
        "trait Loader { async fn load(self) -> i64; }",
    ] {
        let graph = graph(text);
        assert!(
            !graph.diagnostics().is_empty(),
            "removed syntax must be rejected: {text}"
        );
        assert!(
            graph
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.span.is_some())
        );
    }
}

#[test]
fn removed_task_calls_have_no_builtin_targets_or_effects() {
    for text in [
        "fn main() { task::spawn_scoped(worker()); }",
        "fn main() { task::spawn_scoped_then(worker(), continuation); }",
        "use task as jobs; fn main() { jobs::spawn_scoped(worker()); }",
        "use task::spawn_scoped as spawn; fn main() { spawn(worker()); }",
    ] {
        let graph = graph(text);
        let facts = AnalysisFacts::from_module_graph(&graph);
        let main = graph
            .declarations()
            .find(|declaration| declaration.name == "main")
            .expect("main");
        let body = graph.function_body(main.id).expect("main body");
        let (call, _) = body
            .calls()
            .find(|(_, call)| {
                body.expression(call.callee).is_some_and(|expression| {
                    let span = expression.origin.span;
                    text[span.start as usize..span.end as usize].contains("spawn")
                })
            })
            .expect("removed task call");
        assert_eq!(
            facts.call_target(call),
            Some(&CallTargetFact::Unresolved),
            "{text}"
        );
        assert_eq!(
            facts.effect(call),
            Some(&RegistryEffectFact::pure()),
            "{text}"
        );
    }
}

#[test]
fn removed_task_error_has_no_builtin_type_fact() {
    let graph = graph("fn main(value: task::Error) {}");
    let path = vec!["task".to_owned(), "Error".to_owned()];
    assert_eq!(type_fact_from_path(&graph, &path), TypeFact::Unknown);
    let main = graph
        .declarations()
        .find(|declaration| declaration.name == "main")
        .expect("main");
    let signature = graph.function_signature(main.id).expect("signature");
    assert_eq!(
        type_fact_from_hint(
            &graph,
            signature.params[0].type_hint.as_ref().expect("hint")
        ),
        TypeFact::Unknown
    );
}

fn graph(text: &str) -> ModuleGraph {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SourceId::new(115),
        PackageId::anonymous(),
        ModulePath::from_qualified("app"),
        text,
    ));
    graph.resolve_imports();
    graph
}
