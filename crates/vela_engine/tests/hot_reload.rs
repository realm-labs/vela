use vela_common::SourceId;
use vela_engine::{CallOptions, Engine, Runtime, Value};
use vela_host::{MockStateAdapter, PatchTx};

#[test]
fn runtime_hot_reload_update_waits_for_explicit_apply_safe_point() {
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial(SourceId::new(1), "fn main() { return 1; }")
        .expect("initial hot reload compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let initial_version = runtime
        .hot_reload_version()
        .expect("runtime should expose active hot reload version")
        .id;
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();

    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(1))
    );

    let update = runtime
        .compile_hot_reload_update(SourceId::new(2), "fn main() { return 2; }")
        .expect("runtime should be hot-reload enabled")
        .expect("compatible update should compile");

    assert_eq!(
        runtime
            .hot_reload_version()
            .expect("runtime should keep active version until apply")
            .id,
        initial_version
    );
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(1))
    );

    let report = runtime
        .apply_hot_update(update)
        .expect("runtime should apply update at safe point");

    assert!(report.accepted);
    assert_eq!(
        runtime.call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(Value::Int(2))
    );
}
