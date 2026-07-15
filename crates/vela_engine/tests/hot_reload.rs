use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll, Waker};

use vela_def::FunctionId;
use vela_engine::engine::Engine;
use vela_engine::native::{NativeFunctionDesc, TypeHint};
use vela_engine::runtime::{CallArgs, CallOptions, Runtime};
use vela_host::access::HostAccess;
use vela_host::mock::MockStateAdapter;
use vela_reflect::permissions::ReflectPolicy;
use vela_vm::owned_value::OwnedValue;

struct ReloadGateFuture {
    ready: Arc<AtomicBool>,
    value: OwnedValue,
}

impl Future for ReloadGateFuture {
    type Output = vela_vm::error::VmResult<OwnedValue>;

    fn poll(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<Self::Output> {
        if self.ready.load(Ordering::SeqCst) {
            Poll::Ready(Ok(self.value.clone()))
        } else {
            Poll::Pending
        }
    }
}

fn reload_gate_engine(ready: Arc<AtomicBool>) -> Engine {
    Engine::builder()
        .register_async_fn(
            NativeFunctionDesc::new("reload_gate", FunctionId::new(0xA5D0))
                .param("value", TypeHint::i64())
                .returns(TypeHint::i64()),
            move |args| {
                Box::pin(ReloadGateFuture {
                    ready: Arc::clone(&ready),
                    value: args.first().cloned().unwrap_or(OwnedValue::Unit),
                })
            },
        )
        .build()
        .expect("reload gate engine should build")
}

fn call_raw(
    runtime: &mut Runtime,
    entry: &str,
    args: &[OwnedValue],
    options: CallOptions,
    adapter: &mut MockStateAdapter,
    _access: &mut HostAccess,
) -> vela_vm::error::VmResult<OwnedValue> {
    let args = CallArgs::from_positional(args.iter().cloned()).with_fallback_adapter(adapter);
    let value = runtime.call(entry, args, options)?;
    runtime.value_to_owned(&value)
}

#[test]
fn runtime_hot_reload_update_waits_for_explicit_reload_safe_point() {
    let engine = Engine::builder().build().expect("engine should build");
    let initial = engine
        .compile_hot_reload_initial("fn main() { return 1; }")
        .expect("initial hot reload compile");
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");
    let initial_version = runtime
        .hot_reload_version()
        .expect("runtime should expose active hot reload version")
        .id;
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    assert_eq!(
        call_raw(
            &mut runtime,
            "main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
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
        call_raw(
            &mut runtime,
            "main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );

    let report = runtime
        .apply_hot_update(update)
        .expect("runtime should apply update at safe point");

    assert!(report.accepted);
    assert_eq!(
        call_raw(
            &mut runtime,
            "main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );
}

#[test]
fn suspended_async_call_keeps_old_generation_until_completion_safe_point() {
    let ready = Arc::new(AtomicBool::new(false));
    let engine = reload_gate_engine(Arc::clone(&ready));
    let initial = engine
        .compile_hot_reload_initial(
            r#"
async fn wait_value() -> i64 { return reload_gate(10).await; }
async fn main() -> i64 { return (wait_value().await) + 1; }
fn version() -> i64 { return 1; }
"#,
        )
        .expect("initial async generation should compile");
    let update = engine
        .compile_hot_reload_update(
            &initial,
            r#"
async fn wait_value() -> i64 { return reload_gate(10).await; }
async fn main() -> i64 { return (wait_value().await) + 100; }
fn version() -> i64 { return 2; }
"#,
        )
        .expect("ABI-compatible async update should compile");
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");
    let staging = runtime
        .hot_reload_staging_handle()
        .expect("hot reload runtime should expose staging handle");
    let mut future = runtime.call_async("main", CallArgs::new(), CallOptions::unbounded());
    let mut context = Context::from_waker(Waker::noop());

    assert!(matches!(
        Pin::new(&mut future).poll(&mut context),
        Poll::Pending
    ));
    assert_eq!(staging.stage_hot_update(update), None);
    assert!(staging.has_pending_update());

    ready.store(true, Ordering::SeqCst);
    let Poll::Ready(result) = Pin::new(&mut future).poll(&mut context) else {
        panic!("released native gate should resume the old generation");
    };
    let old_result = result.expect("old generation should complete");
    drop(future);

    assert_eq!(runtime.value_to_owned(&old_result), Ok(OwnedValue::i64(11)));
    assert_eq!(
        runtime
            .hot_reload_version()
            .expect("active generation should remain available")
            .id
            .0,
        0
    );

    let report = runtime
        .check_reload()
        .expect("reload check should succeed")
        .expect("staged update should activate after completion");
    assert!(report.accepted);

    let mut next = runtime.call_async("main", CallArgs::new(), CallOptions::unbounded());
    let Poll::Ready(result) = Pin::new(&mut next).poll(&mut context) else {
        panic!("ready native gate should complete the new generation");
    };
    let new_result = result.expect("new generation should complete");
    drop(next);
    assert_eq!(
        runtime.value_to_owned(&new_result),
        Ok(OwnedValue::i64(110))
    );
}

#[test]
fn cancelling_suspended_call_defers_reload_and_releases_runtime() {
    let ready = Arc::new(AtomicBool::new(false));
    let engine = reload_gate_engine(ready);
    let initial = engine
        .compile_hot_reload_initial(
            r#"
async fn main() -> i64 { return reload_gate(10).await; }
fn version() -> i64 { return 1; }
"#,
        )
        .expect("initial async generation should compile");
    let update = engine
        .compile_hot_reload_update(
            &initial,
            r#"
async fn main() -> i64 { return reload_gate(20).await; }
fn version() -> i64 { return 2; }
"#,
        )
        .expect("ABI-compatible async update should compile");
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");
    let staging = runtime
        .hot_reload_staging_handle()
        .expect("hot reload runtime should expose staging handle");
    let mut future = runtime.call_async("main", CallArgs::new(), CallOptions::unbounded());
    let mut context = Context::from_waker(Waker::noop());

    assert!(matches!(
        Pin::new(&mut future).poll(&mut context),
        Poll::Pending
    ));
    assert_eq!(staging.stage_hot_update(update), None);
    drop(future);

    let old_version = runtime
        .call("version", CallArgs::new(), CallOptions::unbounded())
        .expect("cancellation should release Runtime while update remains staged");
    assert_eq!(runtime.value_to_owned(&old_version), Ok(OwnedValue::i64(1)));

    runtime
        .check_reload()
        .expect("reload check should succeed")
        .expect("staged update should activate after cancellation");
    let new_version = runtime
        .call("version", CallArgs::new(), CallOptions::unbounded())
        .expect("Runtime should execute the activated generation");
    assert_eq!(runtime.value_to_owned(&new_version), Ok(OwnedValue::i64(2)));
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
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");
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

    let mut old_args = CallArgs::from_values([old_closure.clone()]);
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

    let mut budgeted_args = CallArgs::from_values([old_closure]);
    budgeted_args.push(5_i64);
    let error = runtime
        .call(
            "invoke",
            budgeted_args,
            CallOptions::new(1, usize::MAX, usize::MAX),
        )
        .expect_err("nested old-generation call needs two semantic call units");
    assert!(matches!(
        error.kind(),
        vela_vm::error::VmErrorKind::BudgetExceeded {
            budget: vela_vm::budget::ExecutionBudgetKind::ExecutionUnits,
            limit: 1
        }
    ));
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
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");
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
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    assert_eq!(
        call_raw(
            &mut runtime,
            "main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
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
        call_raw(
            &mut runtime,
            "main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );

    let report = runtime
        .apply_hot_update(update)
        .expect("runtime should apply update at safe point");

    assert!(report.accepted);
    assert_eq!(
        call_raw(
            &mut runtime,
            "main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
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
    let mut runtime =
        Runtime::from_hot_reload_version(engine, initial).expect("runtime should initialize");
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    assert_eq!(
        call_raw(
            &mut runtime,
            "main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
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
        call_raw(
            &mut runtime,
            "main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(12)))
    );

    let report = runtime
        .apply_hot_update(update)
        .expect("runtime should apply update at safe point");

    assert!(report.accepted);
    assert_eq!(
        call_raw(
            &mut runtime,
            "main",
            &[],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(17)))
    );
}
