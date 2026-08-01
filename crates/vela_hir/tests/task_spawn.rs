use vela_common::SourceId;
use vela_hir::binding::TaskLexicalCapability;
use vela_hir::source_ingestion::build_single_source;

#[test]
fn binds_host_scoped_task_operations_as_non_escaping_capabilities() {
    let sources = build_single_source(
        SourceId::new(1),
        r#"
async fn worker(value) { return value; }
fn continuation(result) { return result; }

fn main(value) {
    task::spawn_scoped(worker(value));
    task::spawn_scoped_then(worker(value), continuation);
}
"#,
    )
    .expect("static task calls should enter HIR");
    let main = sources
        .graph()
        .declarations()
        .find(|declaration| declaration.name == "main")
        .expect("main declaration");
    let bindings = sources.graph().bindings(main.id).expect("main bindings");

    assert_eq!(
        bindings
            .task_capabilities()
            .map(|(_, capability)| capability)
            .collect::<Vec<_>>(),
        [
            TaskLexicalCapability::SpawnScoped,
            TaskLexicalCapability::SpawnScopedThen,
        ]
    );
}

#[test]
fn rejects_non_static_task_shapes_during_hir_ingestion() {
    let cases = [
        (
            r#"
async fn worker() {}
fn main() { task::spawn_scoped(worker); }
"#,
            "hir::invalid_task_worker",
        ),
        (
            r#"
async fn worker() {}
fn main() { task::spawn_scoped("worker"); }
"#,
            "hir::invalid_task_worker",
        ),
        (
            r#"
async fn worker() {}
fn main() { task::spawn_scoped((|| worker())()); }
"#,
            "hir::invalid_task_worker",
        ),
        (
            r#"
async fn worker() {}
fn continuation(result) { return result; }
fn main() { task::spawn_scoped_then(worker(), continuation()); }
"#,
            "hir::invalid_task_continuation",
        ),
        (
            r#"
async fn worker() {}
fn continuation(result) { return result; }
fn main(continuation) { task::spawn_scoped_then(worker(), continuation); }
"#,
            "hir::invalid_task_continuation",
        ),
        (
            r#"
async fn worker() {}
fn main() { let spawn = task::spawn_scoped; spawn(worker()); }
"#,
            "hir::invalid_task_capability_use",
        ),
        (
            r#"
async fn worker() {}
fn main() { task::spawn(worker()); }
"#,
            "hir::unknown_task_operation",
        ),
        (
            r#"
async fn worker() {}
fn main() { task::spawn_scoped(); }
"#,
            "hir::invalid_task_argument_count",
        ),
        (
            r#"
fn worker() {}
fn main() { task::spawn_scoped(worker()); }
"#,
            "hir::task_worker_not_async",
        ),
        (
            r#"
async fn worker() {}
async fn continuation(result) { return result; }
fn main() { task::spawn_scoped_then(worker(), continuation); }
"#,
            "hir::task_continuation_async",
        ),
        (
            r#"
struct Worker {}
fn main() { task::spawn_scoped(Worker()); }
"#,
            "hir::task_worker_not_function",
        ),
    ];

    for (index, (source, expected_code)) in cases.into_iter().enumerate() {
        let error = build_single_source(SourceId::new(index as u32 + 10), source)
            .expect_err("invalid task syntax should fail HIR ingestion");
        assert!(
            error
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code.as_deref() == Some(expected_code)),
            "expected {expected_code}, got {:?}",
            error.diagnostics()
        );
    }
}

#[test]
fn task_root_does_not_reserve_ordinary_local_names() {
    build_single_source(
        SourceId::new(30),
        r#"
async fn worker(value) { return value; }
fn main(task) {
    let copied = task;
    task::spawn_scoped(worker(copied));
}
"#,
    )
    .expect("the bare task name remains available as a local");
}
