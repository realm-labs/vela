use vela_common::{CallableAsyncness, Detachability, NonDetachableValueKind};
use vela_mir::{CompileCalleeTarget, CompileTaskOperation, MirStatementKind};

use super::{FixtureRoots, prepare_source};

#[test]
fn scoped_task_targets_preserve_static_worker_and_continuation_identity() {
    let fixture = prepare_source(
        r#"
async fn repair(value: i64) -> i64 { value }
state current: i64 = 0;
async fn repair_with_state(value: i64) -> i64 { current + value }
fn complete(result: Result<i64, task::Error>, turn: Any) { current = 1; }

fn main() {
    task::spawn_scoped(repair(1));
    task::spawn_scoped_then(repair_with_state(2), complete);
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
    assert_eq!(first.detachability.parameters, [Detachability::Detachable]);
    assert_eq!(first.detachability.result, Detachability::Detachable);

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
    let continuation = second.continuation.as_ref().expect("continuation ABI");
    assert_eq!(
        continuation.outcome_contract,
        vela_mir::MirTypeContract::Result {
            ok: Some(Box::new(vela_mir::MirTypeContract::Primitive(
                vela_common::PrimitiveTag::I64,
            ))),
            err: Some(Box::new(vela_mir::MirTypeContract::TaskError)),
        }
    );
    assert_eq!(continuation.resume_parameters.len(), 1);
    assert_eq!(continuation.resume_parameters[0].name, "turn");
    let worker_call = function_targets
        .call(second.worker_call)
        .expect("worker call placement");
    assert!(matches!(
        &worker_call.callee,
        CompileCalleeTarget::ScriptFunction { function, debug_name }
            if *function == second.worker && debug_name == "repair_with_state"
    ));
    assert_eq!(
        function_targets
            .function_descriptor(second.worker)
            .expect("worker descriptor")
            .signature
            .asyncness,
        CallableAsyncness::Async
    );

    let programs = fixture
        .input
        .lowering_inputs(&fixture.graph, vela_mir::MirLoweringConfig::default())
        .expect("task lowering inputs")
        .into_iter()
        .map(|input| vela_mir::build_mir(input).expect("task MIR builds"))
        .collect::<Vec<_>>();
    let main_program = programs
        .iter()
        .find(|program| {
            program.functions().any(|(_, function)| {
                matches!(function.owner(), vela_mir::MirFunctionOwner::Function(id) if *id == root)
            })
        })
        .expect("main MIR program");
    let tasks = main_program
        .functions()
        .flat_map(|(_, function)| function.statements())
        .filter_map(|(_, statement)| match &statement.kind {
            MirStatementKind::Task(task) => Some((task, statement)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(tasks.len(), 2);
    assert!(
        tasks
            .iter()
            .all(|(_, statement)| { statement.effect.task_spawn && statement.safepoint.is_some() })
    );
    assert!(tasks[0].0.continuation.is_none());
    assert_eq!(
        tasks[1]
            .0
            .continuation
            .as_ref()
            .map(|continuation| continuation.debug_name.as_str()),
        Some("complete")
    );
    vela_mir::verify_owned_mir(main_program.clone()).expect("task MIR verifies");

    let bundle = vela_mir::OwnedVerifiedMirBundle::new(
        programs
            .into_iter()
            .map(|program| vela_mir::verify_owned_mir(program).expect("task MIR root verifies")),
    );
    let effective = crate::binding_schema::effective_effects(&bundle)
        .get(&root)
        .copied()
        .expect("main effective effects");
    assert!(effective.task_spawn);
    assert!(effective.state_read);
    assert!(effective.state_write);
}

#[test]
fn scoped_task_rejects_statically_known_callable_arguments_and_results() {
    for (source, path) in [
        (
            r#"
async fn worker(callback: Closure) {}
fn main() { task::spawn_scoped(worker(|value| value)); }
"#,
            "parameter `callback`",
        ),
        (
            r#"
async fn worker() -> Closure { |value| value }
fn main() { task::spawn_scoped(worker()); }
"#,
            "return value",
        ),
    ] {
        let error = prepare_source(source, FixtureRoots::Program)
            .expect_err("callable task transfer must be rejected");
        assert_eq!(
            error.to_diagnostic().and_then(|diagnostic| diagnostic.code),
            Some("compiler::task_value_not_detachable".to_owned())
        );
        let crate::compiler::error::CompileErrorKind::TaskValueNotDetachable {
            target,
            path: actual_path,
            kind,
        } = error.kind
        else {
            panic!("unexpected task detachment error: {error:?}");
        };
        assert_eq!(target, "worker");
        assert_eq!(actual_path, path);
        assert_eq!(kind, NonDetachableValueKind::Callable);
    }
}

#[test]
fn erased_task_values_preserve_mandatory_runtime_detachment_checks() {
    let fixture = prepare_source(
        r#"
async fn worker(value: Any) -> Any { value }
fn main(value: Any) { task::spawn_scoped(worker(value)); }
"#,
        FixtureRoots::Program,
    )
    .expect("erased values remain valid with runtime checks");
    let root = fixture
        .declarations
        .get("main")
        .and_then(|declaration| {
            fixture
                .input
                .targets()
                .function_for_declaration(*declaration)
        })
        .expect("main function identity");
    let targets = fixture
        .input
        .targets()
        .function_targets(root)
        .expect("main targets");
    let task = fixture
        .expression_sources
        .iter()
        .find_map(|(_, expression, _)| targets.task(*expression))
        .expect("task target");

    assert_eq!(
        task.detachability.parameters,
        [Detachability::RuntimeChecked]
    );
    assert_eq!(task.detachability.result, Detachability::RuntimeChecked);
}

#[test]
fn scoped_task_continuation_requires_the_exact_owned_outcome_abi() {
    for (parameter, expected) in [
        ("result: Any", "Result<i64, task::Error>"),
        (
            "result: Result<String, task::Error>",
            "Result<i64, task::Error>",
        ),
        ("", "Result<i64, task::Error>"),
    ] {
        let source = format!(
            r#"
async fn worker() -> i64 {{ 1 }}
fn continuation({parameter}) {{}}
fn main() {{ task::spawn_scoped_then(worker(), continuation); }}
"#
        );
        let error = prepare_source(&source, FixtureRoots::Program)
            .expect_err("continuation outcome ABI mismatch must be rejected");
        let diagnostic = error.to_diagnostic().expect("continuation diagnostic");
        assert_eq!(
            diagnostic.code.as_deref(),
            Some("compiler::task_continuation_invalid")
        );
        assert!(diagnostic.message.contains(expected));
    }
}
