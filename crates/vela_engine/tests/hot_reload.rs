use vela_engine::engine::Engine;
use vela_engine::runtime::{CallOptions, Runtime};
use vela_host::access::HostAccess;
use vela_host::mock::MockStateAdapter;
use vela_reflect::permissions::ReflectPolicy;
use vela_vm::owned_value::OwnedValue;

#[test]
fn runtime_hot_reload_update_waits_for_explicit_reload_safe_point() {
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial("fn main() { return 1; }")
        .expect("initial hot reload compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let initial_version = runtime
        .hot_reload_version()
        .expect("runtime should expose active hot reload version")
        .id;
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );

    let update = runtime
        .compile_hot_reload_update("fn main() { return 2; }")
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
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );

    let report = runtime
        .apply_hot_update(update)
        .expect("runtime should apply update at safe point");

    assert!(report.accepted);
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );
}

#[test]
fn hot_reload_runtime_reflection_tracks_script_metadata_after_reload() {
    let engine = Engine::builder()
        .reflection_policy(ReflectPolicy::all())
        .build()
        .expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial(
            r#"
enum QuestProgress {
    Active { count }
}

fn main() {
    let quest_type = reflect::type_info("QuestProgress");
    let quest = QuestProgress::Active { count: 1 };

    if reflect::kind(quest_type) == "script_enum"
        && reflect::has_function("main")
        && reflect::has_variant(quest_type, "Active")
        && reflect::variant_is(quest, "Active") {
        return 1;
    }

    return 0;
}
"#,
        )
        .expect("initial hot reload compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );

    let update = runtime
        .compile_hot_reload_update(
            r#"
enum QuestProgress {
    Active { count }
    Finished { count }
}

fn main() {
    let quest_type = reflect::type_info("QuestProgress");
    let quest = QuestProgress::Finished { count: 2 };

    if reflect::kind(quest_type) == "script_enum"
        && reflect::has_function("main")
        && reflect::has_variant(quest_type, "Finished")
        && reflect::variant_is(quest, "Finished") {
        return 2;
    }

    return 0;
}
"#,
        )
        .expect("runtime should be hot-reload enabled")
        .expect("compatible update should compile");

    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );

    let report = runtime
        .apply_hot_update(update)
        .expect("runtime should apply update at safe point");

    assert!(report.accepted);
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );
}

#[test]
fn hot_reload_runtime_preserves_script_method_dispatch_tables() {
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial(
            r#"
trait BonusSource {
    fn bonus(self, amount) -> i64;
}

struct Player {
    level: i64
}

impl BonusSource for Player {
    fn bonus(self, amount) -> i64 {
        return self.level + amount;
    }
}

fn main() {
    return Player { level: 7 }.bonus(5);
}
"#,
        )
        .expect("initial hot reload compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(12)))
    );

    let update = runtime
        .compile_hot_reload_update(
            r#"
trait BonusSource {
    fn bonus(self, amount) -> i64;
}

struct Player {
    level: i64
}

impl BonusSource for Player {
    fn bonus(self, amount) -> i64 {
        return self.level + amount * 2;
    }
}

fn main() {
    return Player { level: 7 }.bonus(5);
}
"#,
        )
        .expect("runtime should be hot-reload enabled")
        .expect("compatible update should compile");

    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(12)))
    );

    let report = runtime
        .apply_hot_update(update)
        .expect("runtime should apply update at safe point");

    assert!(report.accepted);
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(17)))
    );
}
