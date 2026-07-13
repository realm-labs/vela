use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use vela_vm::error::VmResult;

use super::VelaValue;

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
    use std::pin::Pin;
    use std::task::{Context, Poll, Waker};

    use vela_vm::owned_value::OwnedValue;

    use super::RuntimeCallFuture;
    use crate::engine::Engine;
    use crate::runtime::{CallArgs, CallOptions, Runtime};

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
        let mut runtime = Runtime::new(engine, program);

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
}
