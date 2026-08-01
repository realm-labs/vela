use vela_common::{CallableAsyncness, SourceId};
use vela_def::FunctionId;
use vela_hir::ids::HirDeclId;
use vela_hir::module_graph::{ModuleGraph, ModuleSource};
use vela_package::ModulePath;

use crate::executable::{ExecutableAnalysisGeneration, ExecutableAnalysisInput};
use crate::facts::AnalysisFacts;
use crate::registry::{CallableSignatureFact, RegistryEffectFact, RegistryFacts};
use crate::type_fact::TypeFact;
use crate::validation::ExecutableValidationFacts;

#[test]
fn detached_task_diagnostics_name_nested_value_paths_and_continuation_abi() {
    let source = SourceId::new(115);
    let text = r#"
async fn worker(callback: Closure) -> Array<Closure> { return [callback]; }
fn continuation(result: Any) {}
fn main() {
    task::spawn_scoped_then(worker(|value| value), continuation);
}
"#;
    let (graph, main) = graph(source, text);
    let generation = ExecutableAnalysisGeneration::from_module_graph(
        &graph,
        [ExecutableAnalysisInput::new(
            FunctionId::new(11_501),
            graph.function_body(main).expect("main body").id,
        )],
    )
    .expect("task analysis");
    let view = generation.view(FunctionId::new(11_501)).expect("main view");
    let diagnostics = view.validation_diagnostics();

    assert_eq!(
        diagnostics
            .iter()
            .filter_map(|diagnostic| diagnostic.code.as_deref())
            .collect::<Vec<_>>(),
        [
            "analysis::task_value_not_detachable",
            "analysis::task_value_not_detachable",
            "analysis::task_value_not_detachable",
            "analysis::task_continuation_invalid",
        ]
    );
    assert!(diagnostics[0].message.contains("target `worker`"));
    assert!(diagnostics[0].message.contains("parameter `callback`"));
    assert!(diagnostics[0].message.contains("callable"));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("argument for parameter `callback`")
    }));
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("return value.element"))
    );
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("Result<Array<Closure>, task::Error>")
        }),
        "{diagnostics:#?}"
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.span.is_some())
    );
}

#[test]
fn erased_detached_values_and_exact_continuations_remain_valid() {
    let source = SourceId::new(116);
    let text = r#"
async fn worker(value: Any) -> Any { return value; }
fn continuation(result: Result<Any, task::Error>, turn: i64 = 0) {}
fn main(value: Any) {
    task::spawn_scoped_then(worker(value), continuation);
}
"#;
    let (graph, main) = graph(source, text);
    let generation = ExecutableAnalysisGeneration::from_module_graph(
        &graph,
        [ExecutableAnalysisInput::new(
            FunctionId::new(11_601),
            graph.function_body(main).expect("main body").id,
        )],
    )
    .expect("task analysis");

    assert!(
        generation
            .view(FunctionId::new(11_601))
            .expect("main view")
            .validation_diagnostics()
            .is_empty()
    );
}

#[test]
fn detached_task_effect_diagnostic_closes_over_worker_calls_and_engine_ceiling() {
    let source = SourceId::new(117);
    let text = r#"
async fn worker() -> i64 { return database::load().await; }
fn main() { task::spawn_scoped(worker()); }
"#;
    let (graph, main) = graph(source, text);
    let mut schema = RegistryFacts::default();
    schema.insert_function(
        "database::load",
        TypeFact::function(Vec::new(), TypeFact::I64),
    );
    schema.insert_function_signature(
        "database::load",
        CallableSignatureFact::new(Vec::new(), TypeFact::I64).asyncness(CallableAsyncness::Async),
    );
    schema.insert_function_effect(
        "database::load",
        RegistryEffectFact {
            reads_io: true,
            ..RegistryEffectFact::pure()
        },
    );
    schema.set_execution_effect_ceiling(RegistryEffectFact::pure());
    let facts = AnalysisFacts::from_module_graph_and_schema(&graph, &schema);
    let validation = ExecutableValidationFacts::for_bodies(
        &graph,
        Some(&schema),
        &facts,
        [graph.function_body(main).expect("main body").id],
    );
    let diagnostic = validation
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code.as_deref() == Some("analysis::task_effect_denied"))
        .expect("effect diagnostic");

    assert!(diagnostic.message.contains("target `worker`"));
    assert!(diagnostic.message.contains("`reads_io` / `io_read`"));
    assert!(diagnostic.message.contains("`spawns_tasks` / `task_spawn`"));
    assert!(diagnostic.labels[0].message.contains("TaskScope"));
}

fn graph(source: SourceId, text: &str) -> (ModuleGraph, HirDeclId) {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        source,
        vela_package::PackageId::anonymous(),
        ModulePath::from_qualified("game"),
        text,
    ));
    graph.resolve_imports();
    assert_eq!(graph.diagnostics(), &[]);
    let main = graph
        .declarations()
        .find(|declaration| declaration.name == "main")
        .map(|declaration| declaration.id)
        .expect("main declaration");
    (graph, main)
}
