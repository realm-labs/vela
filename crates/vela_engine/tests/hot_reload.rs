use vela_engine::engine::Engine;
use vela_engine::runtime::{CallArgs, CallOptions, Runtime};
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
fn retained_closure_pins_old_generation_across_handle_layout_reload() {
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial(
            r#"
fn helper(value: i64) -> i64 { return value + 1; }
fn make() { return |value: i64| helper(value) + 10; }
fn invoke(callback, value: i64) -> i64 { return callback(value); }
"#,
        )
        .expect("initial closure generation should compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let old_closure = runtime
        .call("make", CallArgs::new(), CallOptions::unbounded())
        .expect("old closure should be retained");

    let update = runtime
        .compile_hot_reload_update(
            r#"
fn alpha_private(value: i64) -> i64 { return value * 1000; }
fn helper(value: i64) -> i64 { return value + 100; }
fn make() { return |value: i64| helper(value) + 20; }
fn invoke(callback, value: i64) -> i64 { return callback(value); }
"#,
        )
        .expect("runtime should compile closure reload")
        .expect("closure reload should be compatible");
    runtime
        .apply_hot_update(update)
        .expect("closure reload should apply at the safe point");

    let mut old_args = CallArgs::from_values([old_closure]);
    old_args.push(5_i64);
    let old_result = runtime
        .call("invoke", old_args, CallOptions::unbounded())
        .expect("old closure should execute against its creation generation");
    let new_closure = runtime
        .call("make", CallArgs::new(), CallOptions::unbounded())
        .expect("new closure should use the active generation");
    let mut new_args = CallArgs::from_values([new_closure]);
    new_args.push(5_i64);
    let new_result = runtime
        .call("invoke", new_args, CallOptions::unbounded())
        .expect("new closure should execute active code");

    assert_eq!(runtime.value_to_owned(&old_result), Ok(OwnedValue::i64(16)));
    assert_eq!(
        runtime.value_to_owned(&new_result),
        Ok(OwnedValue::i64(125))
    );
}

#[test]
fn retained_old_closure_keeps_native_dispatch_and_rejected_reload_owner() {
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial(
            r#"
fn make() { return |value: String| value.starts_with("old"); }
fn invoke(callback, value: String) -> bool { return callback(value); }
"#,
        )
        .expect("initial native closure generation should compile");
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let old_generation = runtime
        .hot_reload_version()
        .expect("hot reload version")
        .executable_generation_id();
    let old_closure = runtime
        .call("make", CallArgs::new(), CallOptions::unbounded())
        .expect("old closure should be retained");

    let rejected = runtime
        .compile_hot_reload_update(
            r#"
fn make(extra) { return |value: String| value.ends_with("new"); }
fn invoke(callback, value: String) -> bool { return callback(value); }
"#,
        )
        .expect("incompatible source should be reported, not fail compilation");
    assert!(rejected.is_err());
    assert_eq!(
        runtime
            .hot_reload_version()
            .expect("rejected reload keeps version")
            .executable_generation_id(),
        old_generation
    );

    let mut args = CallArgs::from_values([old_closure]);
    args.push("old-generation".to_owned());
    let result = runtime
        .call("invoke", args, CallOptions::unbounded())
        .expect("old closure should retain native dispatch after rejection");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::Bool(true)));
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
