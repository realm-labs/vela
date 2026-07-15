use std::sync::{Arc, Mutex};

use super::*;

#[test]
fn async_provider_target_uses_unified_call_and_reentry_drivers() {
    let root = package_fixture("async_provider_runtime_call");
    write_package(
        &root,
        "dev.vela.plugin",
        "plugin",
        "",
        r#"pub trait CommandProvider { async fn run(self, value: i64) -> i64; }
pub struct Command {}
#[provider(id = "command")]
impl CommandProvider for Command {
    pub async fn run(self, value: i64) -> i64 {
        test::mark();
        return value + 1;
    }
}

pub async fn main() -> i64 { return test::enter_provider().await; }
"#,
    );
    let target = Arc::new(Mutex::new(None::<ProviderMethodTarget>));
    let reentry_target = Arc::clone(&target);
    let marks = Arc::new(AtomicU64::new(0));
    let native_marks = Arc::clone(&marks);
    let engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("test::mark", FunctionId::new(0xA540))
                .returns(TypeHint::unit())
                .access(FunctionAccess::public()),
            move |_args| {
                native_marks.fetch_add(1, Ordering::SeqCst);
                Ok(OwnedValue::Unit)
            },
        )
        .register_async_context_fn(
            NativeFunctionDesc::new("test::enter_provider", FunctionId::new(0xA541))
                .returns(TypeHint::i64())
                .access(FunctionAccess::public()),
            move |_args, context| {
                let target = reentry_target
                    .lock()
                    .expect("provider target slot should not be poisoned")
                    .clone()
                    .expect("provider target should be installed before execution");
                Box::pin(async move {
                    let _ = context
                        .call_async(target, CallArgs::new().with_value("value", 40_i64))
                        .await?;
                    Ok(OwnedValue::i64(7))
                })
            },
        )
        .build()
        .expect("engine");
    let snapshot = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("snapshot");
    let catalog = engine.discover_providers(&snapshot).expect("catalog");
    let descriptor = &catalog.providers()[0];
    let key = descriptor.key().clone();
    let method = descriptor.methods()[0].id();
    let selection = catalog.select([key.clone()]).expect("selection");
    let request = ProviderCompileRequest::for_selection(&snapshot, selection);
    let artifact = engine
        .compile_provider_selection(&snapshot, &request)
        .expect("selected async provider compiles");
    let mut runtime =
        Runtime::from_linked_artifact(engine, artifact).expect("runtime should initialize");
    let handle = runtime.provider_handle(&key).expect("provider handle");
    *target
        .lock()
        .expect("provider target slot should not be poisoned") = Some(handle.method(method));

    let error = runtime
        .call(
            handle.method(method),
            CallArgs::new().with_value("value", 41_i64),
            CallOptions::unbounded(),
        )
        .expect_err("sync Runtime call must reject an async provider target");
    assert!(matches!(
        error.kind(),
        VmErrorKind::AsyncEntryRequiresCallAsync { .. }
    ));

    let output = poll_to_completion(runtime.call_async(
        handle.method(method),
        CallArgs::new().with_value("value", 41_i64),
        CallOptions::unbounded(),
    ))
    .expect("direct async provider target should run");
    assert_eq!(runtime.value_to_owned(&output), Ok(OwnedValue::i64(42)));

    let reentered = poll_to_completion(runtime.call_async(
        "api::main",
        CallArgs::new(),
        CallOptions::unbounded(),
    ))
    .expect("provider target should run through NativeCallContext reentry");
    assert_eq!(runtime.value_to_owned(&reentered), Ok(OwnedValue::i64(7)));
    assert_eq!(marks.load(Ordering::SeqCst), 2);
    remove_fixture(root);
}
