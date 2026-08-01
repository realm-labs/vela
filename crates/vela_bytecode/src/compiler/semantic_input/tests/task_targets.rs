use vela_common::CallableAsyncness;
use vela_mir::{CompileCalleeTarget, CompileTaskOperation};

use super::{FixtureRoots, prepare_source};

#[test]
fn scoped_task_targets_preserve_static_worker_and_continuation_identity() {
    let fixture = prepare_source(
        r#"
async fn repair(value: i64) -> i64 { value }
fn complete(result: Any) {}

fn main() {
    task::spawn_scoped(repair(1));
    task::spawn_scoped_then(repair(2), complete);
}
"#,
        FixtureRoots::Program,
    )
    .expect("static task targets should prepare");
    let targets = fixture.input.targets();
    let root = fixture
        .declarations
        .get("main")
        .and_then(|declaration| targets.function_for_declaration(*declaration))
        .expect("main function identity");
    let function_targets = targets.function_targets(root).expect("main targets");
    let task_expressions = fixture
        .expression_sources
        .iter()
        .filter(|(_, expression, _)| function_targets.task(*expression).is_some())
        .map(|(_, expression, _)| *expression)
        .collect::<Vec<_>>();
    assert_eq!(task_expressions.len(), 2);

    let first = function_targets
        .task(task_expressions[0])
        .expect("spawn_scoped target");
    assert_eq!(first.operation, CompileTaskOperation::SpawnScoped);
    assert!(first.continuation.is_none());

    let second = function_targets
        .task(task_expressions[1])
        .expect("spawn_scoped_then target");
    assert_eq!(second.operation, CompileTaskOperation::SpawnScopedThen);
    assert_eq!(
        second
            .continuation
            .as_ref()
            .map(|continuation| continuation.debug_name.as_str()),
        Some("complete")
    );
    let worker_call = function_targets
        .call(second.worker_call)
        .expect("worker call placement");
    assert!(matches!(
        &worker_call.callee,
        CompileCalleeTarget::ScriptFunction { function, debug_name }
            if *function == second.worker && debug_name == "repair"
    ));
    assert_eq!(
        function_targets
            .function_descriptor(second.worker)
            .expect("worker descriptor")
            .signature
            .asyncness,
        CallableAsyncness::Async
    );
}
