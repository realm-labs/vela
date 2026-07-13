use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

use vela_host::mock::MockStateAdapter;

use super::CallArgs;

type ScopedSendFuture<'call, T> = Pin<Box<dyn Future<Output = T> + Send + 'call>>;

#[derive(Default)]
struct ActorState {
    score: i64,
}

#[derive(Default)]
struct ProofRuntime {
    calls: usize,
}

fn scoped_runtime_call<'call>(
    runtime: &'call mut ProofRuntime,
    actor: &'call mut ActorState,
) -> ScopedSendFuture<'call, i64> {
    Box::pin(async move {
        runtime.calls += 1;
        actor.score += 1;
        actor.score
    })
}

trait ProofFactory: Send + Sync + 'static {
    fn invoke<'call>(&self, actor: &'call mut ActorState) -> ScopedSendFuture<'call, i64>;
}

struct RegisteredFactory;

impl ProofFactory for RegisteredFactory {
    fn invoke<'call>(&self, actor: &'call mut ActorState) -> ScopedSendFuture<'call, i64> {
        Box::pin(async move {
            actor.score += 1;
            actor.score
        })
    }
}

struct BindingScopes<'host> {
    actor: Option<&'host mut ActorState>,
}

struct ProofSession<'host> {
    nested_calls: usize,
    bindings: BindingScopes<'host>,
}

struct PreparedLease<'host> {
    actor: &'host mut ActorState,
}

struct ResumePacket<'host> {
    lease: PreparedLease<'host>,
    value: i64,
}

impl<'host> ProofSession<'host> {
    fn prepare(&mut self) -> PreparedLease<'host> {
        PreparedLease {
            actor: self
                .bindings
                .actor
                .take()
                .expect("proof binding should be available"),
        }
    }

    fn restore(&mut self, lease: PreparedLease<'host>) {
        assert!(self.bindings.actor.replace(lease.actor).is_none());
    }
}

struct ProofNativeContext<'session> {
    nested_calls: &'session mut usize,
}

impl ProofNativeContext<'_> {
    fn call_async<'call>(
        &'call mut self,
        actor: &'call mut ActorState,
    ) -> ScopedSendFuture<'call, i64> {
        Box::pin(async move {
            PendingOnce::new().await;
            *self.nested_calls += 1;
            actor.score += 10;
            actor.score
        })
    }
}

fn invoke_prepared<'call, 'host, 'session>(
    context: &'call mut ProofNativeContext<'session>,
    lease: PreparedLease<'host>,
) -> ScopedSendFuture<'call, ResumePacket<'host>>
where
    'host: 'call,
    'session: 'call,
{
    Box::pin(async move {
        PendingOnce::new().await;
        let value = context.call_async(&mut *lease.actor).await;
        ResumePacket { lease, value }
    })
}

fn drive_prepared<'call, 'host>(
    session: &'call mut ProofSession<'host>,
) -> ScopedSendFuture<'call, i64>
where
    'host: 'call,
{
    Box::pin(async move {
        let lease = session.prepare();
        let mut context = ProofNativeContext {
            nested_calls: &mut session.nested_calls,
        };
        let packet = invoke_prepared(&mut context, lease).await;
        let value = packet.value;
        session.restore(packet.lease);
        value
    })
}

struct PendingOnce {
    polled: bool,
}

impl PendingOnce {
    fn new() -> Self {
        Self { polled: false }
    }
}

impl Future for PendingOnce {
    type Output = ();

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        if this.polled {
            Poll::Ready(())
        } else {
            this.polled = true;
            context.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

fn require_send<T: Send>(_: &T) {}

fn run_to_completion<T>(mut future: ScopedSendFuture<'_, T>) -> T {
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
    }
}

#[test]
fn scoped_runtime_and_registered_factory_futures_are_send() {
    let mut runtime = ProofRuntime::default();
    let mut actor = ActorState::default();
    let future = scoped_runtime_call(&mut runtime, &mut actor);
    require_send(&future);
    assert_eq!(run_to_completion(future), 1);

    let factory: Arc<dyn ProofFactory> = Arc::new(RegisteredFactory);
    let future = factory.invoke(&mut actor);
    require_send(&future);
    assert_eq!(run_to_completion(future), 2);
}

#[test]
fn direct_call_args_preserve_send_after_trait_object_erasure() {
    let shared = vec![1_i64, 2, 3];
    let shared_args = CallArgs::new().with_host_ref("shared", &shared);
    require_send(&shared_args);

    let mut mutable = vec![1_i64, 2, 3];
    let mutable_args = CallArgs::new().with_host_mut("mutable", &mut mutable);
    require_send(&mutable_args);

    let mut adapter = MockStateAdapter::new();
    let adapter_args = CallArgs::new().with_fallback_adapter(&mut adapter);
    require_send(&adapter_args);
}

#[test]
fn prepared_lease_can_reenter_and_return_to_the_same_send_session() {
    let mut actor = ActorState { score: 5 };
    {
        let mut session = ProofSession {
            nested_calls: 0,
            bindings: BindingScopes {
                actor: Some(&mut actor),
            },
        };

        let future = drive_prepared(&mut session);
        require_send(&future);
        assert_eq!(run_to_completion(future), 15);
        assert_eq!(session.nested_calls, 1);
    }
    assert_eq!(actor.score, 15);
}
