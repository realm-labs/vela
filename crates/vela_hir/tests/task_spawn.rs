use vela_common::SourceId;
use vela_hir::source_ingestion::build_single_source;

#[test]
fn removed_task_paths_and_aliases_fail_source_ingestion() {
    for (index, source) in [
        "fn main() { task::spawn_scoped(worker()); }",
        "fn main() { task::spawn_scoped_then(worker(), continuation); }",
        "fn main() { let spawn = task::spawn_scoped; spawn(worker()); }",
        "use task::spawn_scoped as spawn; fn main() { spawn(worker()); }",
        "use task::spawn_scoped_then as then; fn main() { then(worker(), continuation); }",
        "use task as jobs; fn main() { jobs::spawn_scoped(worker()); }",
        "use task as jobs; use jobs as queue; fn main() { queue::spawn_scoped(worker()); }",
        "use jobs as task; fn main() { task::spawn_scoped(worker()); }",
        "use task as jobs; fn main() { jobs::spawn_scoped_then(worker(), continuation); }",
        "use task::Error as Failure; fn main(value: Result<i64, Failure>) {}",
        "fn main(value: Result<i64, task::Error>) {}",
    ]
    .into_iter()
    .enumerate()
    {
        let error = build_single_source(SourceId::new(index as u32 + 1), source)
            .expect_err("removed task paths must fail before compilation");
        assert!(
            error.diagnostics().iter().any(|diagnostic| {
                diagnostic.code.as_deref() == Some("syntax::removed_task")
                    && diagnostic.span.is_some()
            }),
            "{source}: {:?}",
            error.diagnostics()
        );
    }
}

#[test]
fn async_declarations_and_await_fail_source_ingestion() {
    for source in [
        "async fn worker() {}",
        "pub async fn worker() {}",
        "trait Worker { async fn run(self); }",
        "struct Worker {} impl Worker { async fn run(self) {} }",
        "fn main() { worker().await; }",
        "fn main() { let callback = || worker().await; }",
    ] {
        let error = build_single_source(SourceId::new(20), source)
            .expect_err("removed async syntax must fail before HIR lowering");
        assert!(
            error
                .diagnostics()
                .iter()
                .any(|diagnostic| { diagnostic.code.as_deref() == Some("syntax::removed_async") }),
            "{source}: {:?}",
            error.diagnostics()
        );
    }
}

#[test]
fn ordinary_task_and_operation_names_remain_available() {
    build_single_source(
        SourceId::new(30),
        r#"
fn spawn_scoped(value) { return value; }
fn spawn_scoped_then(value, continuation) { return continuation(value); }
fn main(task) {
    let then = |value| value;
    return spawn_scoped_then(spawn_scoped(task), then);
}
"#,
    )
    .expect("ordinary task, then, and unqualified operation names remain valid");
}

#[test]
fn ordinary_task_modules_and_unrelated_operation_paths_still_resolve() {
    use vela_hir::module_graph::ModuleSource;
    use vela_hir::source_ingestion::build_module_source_set;
    use vela_package::{ModulePath, PackageId};

    build_module_source_set(&[
        ModuleSource::new(SourceId::new(40), PackageId::anonymous(), ModulePath::root(),
            "use task::ordinary; use jobs::spawn_scoped as spawn; fn main() { return spawn(ordinary(1)); }"),
        ModuleSource::new(SourceId::new(41), PackageId::anonymous(), ModulePath::new(["task"]),
            "pub fn ordinary(value) { return value; }"),
        ModuleSource::new(SourceId::new(42), PackageId::anonymous(), ModulePath::new(["jobs"]),
            "pub fn spawn_scoped(value) { return value; }"),
    ]).expect("only removed compiler-owned paths are reserved");
}
