use vela_engine::engine::Engine;
use vela_engine::runtime::{CallArgs, CallOptions, Runtime};
use vela_engine::source::EngineSourceErrorKind;
use vela_vm::owned_value::OwnedValue;

const REMOVED_SOURCES: &[&str] = &[
    "async fn unused() { return 1; } pub fn main() { return 42; }",
    "trait Worker { async fn run(self); } pub fn main() { return 42; }",
    "struct Worker {} impl Worker { async fn run(self) { return 1; } }",
    "fn value() { return 1; } pub fn main() { return value().await; }",
    "pub fn main() { task::spawn_scoped(worker()); } fn worker() { return 1; }",
    "pub fn main() { task::spawn_scoped_then(worker(), done); } fn worker() { return 1; } fn done(value) { return value; }",
    "use task::spawn_scoped as start; pub fn main() { start(worker()); } fn worker() { return 1; }",
    "fn unused(error: task::Error) { return error; } pub fn main() { return 42; }",
];

#[test]
fn removed_async_and_task_source_fails_before_backend_compilation() {
    let engine = Engine::builder().build().expect("engine");
    for source in REMOVED_SOURCES {
        let error = engine.compile_source(source).expect_err(source);
        let EngineSourceErrorKind::Frontend(error) = error.kind else {
            panic!("removed execution must fail at the source boundary: {source}");
        };
        assert!(
            error
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.span.is_some()),
            "removed execution needs a source-spanned diagnostic: {source}"
        );
    }
}

#[test]
fn removed_execution_reload_leaves_active_generation_and_state_unchanged() {
    let engine = Engine::builder().build().expect("engine");
    let program = engine
        .compile_source("state value: i64 = 41; pub fn main() { value += 1; return value; }")
        .expect("synchronous source");
    let mut runtime = Runtime::builder(engine, program)
        .expect("runtime image")
        .with_hot_reload()
        .expect("reload owner")
        .build()
        .expect("runtime");
    let old = runtime.hot_reload_version().expect("active owner");
    for source in REMOVED_SOURCES {
        runtime.stage_reload(*source).expect_err(source);
        assert!(runtime.activate_reload().expect("safe point").is_none());
        assert_eq!(
            runtime
                .hot_reload_version()
                .expect("unchanged owner")
                .executable_generation_id(),
            old.executable_generation_id()
        );
    }
    let value = runtime
        .call(
            "main",
            CallArgs::new(),
            CallOptions::new(1_000, 1 << 20, 32),
        )
        .expect("retained synchronous call");
    assert_eq!(runtime.value_to_owned(&value), Ok(OwnedValue::i64(42)));
}
