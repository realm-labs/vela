#![cfg_attr(not(test), deny(clippy::wildcard_imports))]
#![allow(clippy::result_large_err)]

use std::error::Error;
use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::thread::{self, Thread};

use vela_engine::prelude::*;
use vela_macros::{ScriptHost, script_methods};

const SOURCE: &str = include_str!("main.vela");

fn main() -> Result<(), Box<dyn Error>> {
    let engine = Engine::builder()
        .register_script_host::<WorkflowState>()
        .register_script_host::<RuleService>()
        .capability(Capability::HostRead)
        .capability(Capability::HostWrite)
        .build()?;
    let program = engine.compile_source(SOURCE)?;

    // Runtime and host state are separate fields, so the call can borrow both.
    let mut actor = WorkflowActor {
        runtime: Runtime::new(engine, program),
        state: WorkflowState { total: 1 },
        service: RuleService { multiplier: 2 },
    };
    let WorkflowActor {
        runtime,
        state,
        service,
    } = &mut actor;

    let output = block_on(
        runtime.call_async(
            "run",
            CallArgs::new()
                .with_host_mut("state", state)
                .with_host_ref("service", service),
            CallOptions::unbounded(),
        ),
    )?;

    println!(
        "async_stateful_reentry result={:?} total={}",
        runtime.value_to_owned(&output)?,
        state.total
    );
    Ok(())
}

struct WorkflowActor {
    runtime: Runtime,
    state: WorkflowState,
    service: RuleService,
}

#[derive(Debug, ScriptHost)]
#[script(path = "examples::async_stateful_reentry::WorkflowState")]
struct WorkflowState {
    #[script(get, set)]
    total: i64,
}

#[derive(Debug, ScriptHost)]
#[script(path = "examples::async_stateful_reentry::RuleService")]
struct RuleService {
    #[script(get)]
    multiplier: i64,
}

impl RuleService {
    async fn approve(&self, amount: i64) -> i64 {
        let mut first_poll = true;
        std::future::poll_fn(move |context| {
            if first_poll {
                first_poll = false;
                context.waker().wake_by_ref();
                Poll::Pending
            } else {
                Poll::Ready(amount * self.multiplier)
            }
        })
        .await
    }
}

#[script_methods]
impl RuleService {}

#[script_methods]
impl WorkflowState {
    #[script_method(effect = "write_host")]
    async fn advance(
        &mut self,
        context: &mut NativeCallContext<'_, '_>,
        service: &RuleService,
        amount: i64,
    ) -> vela_vm::error::VmResult<i64> {
        self.total += service.approve(amount).await;

        // The child receives a fresh HostRef for an explicit mutable reborrow.
        let _ = context
            .call_async(
                "after_advance",
                CallArgs::new()
                    .with_host_mut("state", &mut *self)
                    .with_value("bonus", 3_i64),
            )
            .await?;
        Ok(self.total)
    }
}

struct ThreadWake(Thread);

impl Wake for ThreadWake {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

fn block_on<F: Future>(future: F) -> F::Output {
    let waker = Waker::from(Arc::new(ThreadWake(thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = pin!(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => thread::park(),
        }
    }
}
