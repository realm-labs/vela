use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use vela_vm::error::VmResult;

use super::VelaValue;

/// A scoped, executor-neutral `Send` future for one Runtime execution.
///
/// The future borrows the Runtime and any call-scoped host bindings; it is not
/// required to be `'static`.
///
/// ```compile_fail
/// use vela_engine::prelude::{CallArgs, CallOptions, Engine, Runtime};
///
/// fn require_static<T: 'static>(_: T) {}
///
/// let engine = Engine::builder().build().unwrap();
/// let program = engine.compile_source("fn main() { return 1; }").unwrap();
/// let mut runtime = Runtime::new(engine, program);
/// let future = runtime.call_async("main", CallArgs::new(), CallOptions::unbounded());
/// require_static(future);
/// ```
pub struct RuntimeCallFuture<'call> {
    inner: Pin<Box<dyn Future<Output = VmResult<VelaValue>> + Send + 'call>>,
}

impl<'call> RuntimeCallFuture<'call> {
    pub(super) fn new(future: impl Future<Output = VmResult<VelaValue>> + Send + 'call) -> Self {
        Self {
            inner: Box::pin(future),
        }
    }
}

impl Future for RuntimeCallFuture<'_> {
    type Output = VmResult<VelaValue>;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        self.get_mut().inner.as_mut().poll(context)
    }
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::future::poll_fn;
    use std::pin::Pin;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::task::{Context, Poll, Waker};

    use vela_common::{HostMethodId, HostObjectId, HostTypeId, ScalarValue};
    use vela_def::{FieldId, FunctionId, TypeId};
    use vela_host::mock::MockStateAdapter;
    use vela_host::path::{HostPath, HostRef};
    use vela_host::value::HostValue;
    use vela_vm::owned_value::OwnedValue;

    use super::RuntimeCallFuture;
    use crate::engine::Engine;
    use crate::method::NativeMethodDesc;
    use crate::native::{EffectSet, FunctionAccess, NativeFunctionDesc, TypeHint};
    use crate::permission::Capability;
    use crate::runtime::{CallArgs, CallOptions, Runtime};
    use vela_reflect::permissions::ReflectPermissionSet;
    use vela_reflect::registry::{FieldDesc, TypeDesc, TypeKey};

    fn require_send<T: Send>(_: &T) {}

    fn run_to_completion(
        mut future: RuntimeCallFuture<'_>,
    ) -> vela_vm::error::VmResult<crate::runtime::VelaValue> {
        let mut context = Context::from_waker(Waker::noop());
        loop {
            if let Poll::Ready(value) = Pin::new(&mut future).poll(&mut context) {
                return value;
            }
        }
    }

    #[test]
    fn scoped_runtime_call_future_is_send_and_executes_sync_entry() {
        let engine = Engine::builder().build().expect("engine should build");
        let program = engine
            .compile_source("fn main(value) { return value + 1; }")
            .expect("program should compile");
        let mut runtime = Runtime::new(engine.clone(), program);

        let future = runtime.call_async(
            "main",
            CallArgs::new().with(41_i64),
            CallOptions::unbounded(),
        );
        require_send(&future);
        let value = run_to_completion(future).expect("call should complete");

        assert_eq!(runtime.value_to_owned(&value), Ok(OwnedValue::i64(42)));
    }

    #[test]
    fn scoped_runtime_call_future_accepts_bound_method_target() {
        let engine = Engine::builder().build().expect("engine should build");
        let program = engine
            .compile_source(
                r#"
struct Counter { value: i64 }
fn counter() { return Counter { value: 40 }; }
impl Counter {
    fn add(self, amount) { return self.value + amount; }
}
"#,
            )
            .expect("program should compile");
        let mut runtime = Runtime::new(engine, program);
        let receiver = runtime
            .call("counter", CallArgs::new(), CallOptions::unbounded())
            .expect("receiver factory should run");
        let target = runtime
            .bind_method(&receiver, "add")
            .expect("method target should bind");

        let future = runtime.call_async(
            target,
            CallArgs::new().with(2_i64),
            CallOptions::unbounded(),
        );
        require_send(&future);
        let value = run_to_completion(future).expect("method call should complete");

        assert_eq!(runtime.value_to_owned(&value), Ok(OwnedValue::i64(42)));
    }

    #[test]
    fn runtime_call_future_suspends_and_resumes_async_native() {
        struct WakeOnce {
            value: OwnedValue,
            pending: bool,
            polls: Arc<AtomicUsize>,
        }

        impl Future for WakeOnce {
            type Output = vela_vm::error::VmResult<OwnedValue>;

            fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
                self.polls.fetch_add(1, Ordering::SeqCst);
                if self.pending {
                    self.pending = false;
                    context.waker().wake_by_ref();
                    return Poll::Pending;
                }
                Poll::Ready(Ok(self.value.clone()))
            }
        }

        let polls = Arc::new(AtomicUsize::new(0));
        let factory_polls = Arc::clone(&polls);
        let engine = Engine::builder()
            .with_standard_natives()
            .register_async_fn(
                NativeFunctionDesc::new("async_identity", vela_def::FunctionId::new(0xA51C))
                    .param("value", TypeHint::Any)
                    .returns(TypeHint::Any),
                move |args| {
                    let value = args.first().cloned().unwrap_or(OwnedValue::Unit);
                    Box::pin(WakeOnce {
                        value,
                        pending: true,
                        polls: Arc::clone(&factory_polls),
                    })
                },
            )
            .build()
            .expect("engine should build");
        let program = engine
            .compile_source(
                r#"
async fn checked(value) {
    return result::ok(async_identity(value).await);
}

async fn invoke(callback: Function, value) {
    return callback(value).await;
}

async fn wrapped(value) {
    let resolved = checked(value).await?;
    let adjusted = invoke(|inner| result::ok(inner), resolved).await?;
    return result::ok(adjusted);
}

async fn main(value) {
    return wrapped(value).await.unwrap_or(-1);
}
"#,
            )
            .expect("async program should compile");
        let mut runtime = Runtime::new(engine.clone(), program);
        let mut future = runtime.call_async(
            "main",
            CallArgs::new().with(42_i64),
            CallOptions::unbounded(),
        );
        require_send(&future);
        let mut context = Context::from_waker(Waker::noop());

        assert!(matches!(
            Pin::new(&mut future).poll(&mut context),
            Poll::Pending
        ));
        assert_eq!(polls.load(Ordering::SeqCst), 1);
        let Poll::Ready(result) = Pin::new(&mut future).poll(&mut context) else {
            panic!("woken async native should complete on the next poll");
        };
        let value = result.expect("async call should complete");
        drop(future);

        assert_eq!(polls.load(Ordering::SeqCst), 2);
        assert_eq!(runtime.value_to_owned(&value), Ok(OwnedValue::i64(42)));
    }

    #[test]
    fn runtime_call_future_awaits_host_and_context_native_functions() {
        fn async_add_level<'call, 'host>(
            receiver: &'call HostPath,
            host: &'call mut vela_vm::HostExecution<'host>,
            amount: i64,
        ) -> crate::native::NativeCallFuture<'call> {
            Box::pin(async move {
                let path = receiver.clone().field(FieldId::new(1));
                let HostValue::Scalar(ScalarValue::I64(level)) = host
                    .access
                    .read_diagnostic_path_at(host.adapter, &path, None)?
                else {
                    return Ok(OwnedValue::Unit);
                };
                let level = level + amount;
                host.access.write_diagnostic_path(
                    host.adapter,
                    path,
                    HostValue::Scalar(ScalarValue::I64(level)),
                    None,
                )?;
                Ok(OwnedValue::i64(level))
            })
        }

        let engine = Engine::builder()
            .capability(Capability::HostWrite)
            .register_type(
                TypeDesc::new(TypeKey::new(TypeId::new(0xA522), "Player"))
                    .host_type(HostTypeId::new(1))
                    .field(FieldDesc::new(FieldId::new(1), "level").writable(true)),
            )
            .register_async_host_fn(
                NativeFunctionDesc::new("game::async_set_level", FunctionId::new(0xA520))
                    .param("player", TypeHint::Any)
                    .returns(TypeHint::unit())
                    .effects(EffectSet::host_write())
                    .access(FunctionAccess::public()),
                |args, host| {
                    Box::pin(async move {
                        let Some(OwnedValue::HostRef(player)) = args.first() else {
                            return Ok(OwnedValue::Unit);
                        };
                        let mut pending = true;
                        poll_fn(|context| {
                            if pending {
                                pending = false;
                                context.waker().wake_by_ref();
                                Poll::Pending
                            } else {
                                Poll::Ready(())
                            }
                        })
                        .await;
                        host.access.write_diagnostic_path(
                            host.adapter,
                            HostPath::new(*player).field(FieldId::new(1)),
                            HostValue::Scalar(ScalarValue::I64(11)),
                            None,
                        )?;
                        Ok(OwnedValue::Unit)
                    })
                },
            )
            .register_async_context_fn(
                NativeFunctionDesc::new("game::async_increment_level", FunctionId::new(0xA521))
                    .param("player", TypeHint::Any)
                    .returns(TypeHint::i64())
                    .effects(EffectSet::host_write())
                    .access(FunctionAccess::public().reflect_callable(true)),
                |args, context| {
                    Box::pin(async move {
                        let Some(OwnedValue::HostRef(player)) = args.first() else {
                            return Ok(OwnedValue::Unit);
                        };
                        let path = HostPath::new(*player).field(FieldId::new(1));
                        let HostValue::Scalar(ScalarValue::I64(level)) =
                            context.read_path(&path, None)?
                        else {
                            return Ok(OwnedValue::Unit);
                        };
                        let level = level + 1;
                        context.set_path(path, HostValue::Scalar(ScalarValue::I64(level)), None)?;
                        Ok(OwnedValue::i64(level))
                    })
                },
            )
            .register_typed_async_method_fn::<(i64,), _>(
                NativeMethodDesc::new(
                    TypeKey::new(TypeId::new(0xA522), "Player"),
                    HostMethodId::new(0xA523),
                    "async_add_level",
                )
                .param("amount", TypeHint::i64())
                .returns(TypeHint::i64())
                .effects(EffectSet::host_write())
                .access(FunctionAccess::public().reflect_callable(true)),
                async_add_level,
            )
            .reflection_permissions(ReflectPermissionSet::all())
            .build()
            .expect("engine should build");
        let program = engine
            .compile_source(
                r#"
async fn dynamic_add(player, amount) {
    return player.async_add_level(amount).await;
}

async fn main(player: Player) {
    game::async_set_level(player).await;
    game::async_increment_level(player).await;
    player.async_add_level(5).await;
    dynamic_add(player, 2).await;
    reflect::call(player, "async_add_level", 3).await;
    let increment = reflect::function("game::async_increment_level");
    return reflect::call(increment, player).await;
}
"#,
            )
            .expect("async host program should compile");
        let mut runtime = Runtime::new(engine.clone(), program);
        let player = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 1);
        let level_path = HostPath::new(player).field(FieldId::new(1));
        let mut adapter = MockStateAdapter::new();
        adapter.insert_diagnostic_path_value(
            level_path.clone(),
            HostValue::Scalar(ScalarValue::I64(3)),
        );
        let args = CallArgs::new()
            .with_host_handle("player", player)
            .with_fallback_adapter(&mut adapter);
        let mut future = runtime.call_async("main", args, CallOptions::unbounded());
        require_send(&future);
        let mut context = Context::from_waker(Waker::noop());

        assert!(matches!(
            Pin::new(&mut future).poll(&mut context),
            Poll::Pending
        ));
        let Poll::Ready(result) = Pin::new(&mut future).poll(&mut context) else {
            panic!("woken async host native should complete on the next poll");
        };
        let value = result.expect("async host call should complete");
        drop(future);

        assert_eq!(runtime.value_to_owned(&value), Ok(OwnedValue::i64(23)));
        assert_eq!(
            adapter.read_diagnostic_path(&level_path),
            Ok(HostValue::Scalar(ScalarValue::I64(23)))
        );

        let unawaited = engine
            .compile_source("fn main(player) { return player.async_add_level(1); }")
            .expect("dynamic unawaited call should defer asyncness to runtime");
        let mut unawaited_runtime = Runtime::new(engine.clone(), unawaited);
        let args = CallArgs::new()
            .with_host_handle("player", player)
            .with_fallback_adapter(&mut adapter);
        let error = unawaited_runtime
            .call("main", args, CallOptions::unbounded())
            .expect_err("dynamic async target should require await");
        assert!(matches!(
            error.kind(),
            vela_vm::error::VmErrorKind::AsyncCallRequiresAwait { name }
                if name == "async_add_level"
        ));

        let unawaited_reflection = engine
            .compile_source(
                r#"
fn main(player) {
    return reflect::call(player, "async_add_level", 1);
}
"#,
            )
            .expect("reflection asyncness should be resolved at runtime");
        let mut reflection_runtime = Runtime::new(engine, unawaited_reflection);
        let args = CallArgs::new()
            .with_host_handle("player", player)
            .with_fallback_adapter(&mut adapter);
        let error = reflection_runtime
            .call("main", args, CallOptions::unbounded())
            .expect_err("reflected async target should require await");
        assert!(matches!(
            error.kind(),
            vela_vm::error::VmErrorKind::AsyncCallRequiresAwait { name }
                if name == "async_add_level"
        ));
    }

    #[test]
    fn runtime_call_future_propagates_async_native_errors_and_releases_runtime() {
        let engine = Engine::builder()
            .register_async_fn(
                NativeFunctionDesc::new("async_fail", FunctionId::new(0xA524)),
                |_| {
                    Box::pin(async {
                        Err(vela_vm::error::VmError::new(
                            vela_vm::error::VmErrorKind::TypeMismatch {
                                operation: "async test failure",
                            },
                        ))
                    })
                },
            )
            .build()
            .expect("engine should build");
        let program = engine
            .compile_source(
                r#"
async fn failing() { return async_fail().await; }
fn healthy() { return 42; }
"#,
            )
            .expect("async error program should compile");
        let mut runtime = Runtime::new(engine, program);

        let error = run_to_completion(runtime.call_async(
            "failing",
            CallArgs::new(),
            CallOptions::unbounded(),
        ))
        .expect_err("async native error should propagate");
        assert!(matches!(
            error.kind(),
            vela_vm::error::VmErrorKind::TypeMismatch {
                operation: "async test failure"
            }
        ));
        let healthy = runtime
            .call("healthy", CallArgs::new(), CallOptions::unbounded())
            .expect("runtime should be reusable after async error");
        assert_eq!(runtime.value_to_owned(&healthy), Ok(OwnedValue::i64(42)));
    }
}
