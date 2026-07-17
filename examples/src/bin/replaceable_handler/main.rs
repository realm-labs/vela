#![cfg_attr(not(test), deny(clippy::wildcard_imports))]

use std::error::Error;
use vela_engine::binding::VmResult;
use vela_engine::dispatch::{
    DispatchAuthority, DispatchController, DispatchInvocation, DispatchRoot,
};
use vela_engine::engine::Engine;
use vela_engine::runtime::{RuntimeImage, SharedRuntime};
use vela_macros::{ScriptHost, ScriptReflect, methods};

#[derive(ScriptHost, ScriptReflect)]
#[script(path = "examples::handler::TurnContext")]
struct TurnContext {
    #[script(get, set)]
    calls: i64,
    #[script(skip)]
    dispatch: DispatchRoot,
    #[script(skip)]
    runtime: SharedRuntime,
}

impl DispatchAuthority for TurnContext {
    fn vela_dispatch_root(&self) -> &DispatchRoot {
        &self.dispatch
    }

    fn vela_dispatch_invocation(&mut self) -> VmResult<DispatchInvocation<'_>> {
        let Self {
            dispatch, runtime, ..
        } = self;
        dispatch.invocation(runtime)
    }
}

#[methods(path = "examples::handler::TurnContext")]
impl TurnContext {
    pub fn calls(&self) -> i64 {
        self.calls
    }
}

#[derive(ScriptHost, ScriptReflect)]
#[script(path = "examples::handler::MessageHandler")]
struct MessageHandler {
    #[script(get)]
    bonus: i64,
}

#[methods(path = "host::handler::MessageHandler")]
impl MessageHandler {
    #[vela_macros::replaceable(
        path = "host::handler::MessageHandler::handle",
        authority = "context",
        index = 0
    )]
    pub fn handle(&self, context: &mut TurnContext, message: i64) -> VmResult<i64> {
        context.calls += 10;
        Ok(self.adjacent(message))
    }

    pub fn adjacent(&self, message: i64) -> i64 {
        message + self.bonus
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let slots = MessageHandler::vela_replaceable_slots();
    let engine = Engine::builder()
        .register_host_type::<TurnContext>()
        .register_host_type::<MessageHandler>()
        .register_exports(MessageHandler::vela_inherent_exports())
        .register_replaceable_slots(slots.clone())
        .capability(vela_common::Capability::HostRead)
        .capability(vela_common::Capability::HostWrite)
        .build()?;
    let program = engine.compile_source(include_str!("main.vela"))?;
    let image = RuntimeImage::new_compiled(engine, program).into_shared();
    let runtime = SharedRuntime::from_shared_image(image.clone())?;
    let controller = DispatchController::new(slots)?;
    let handler = MessageHandler { bonus: 1 };

    let mut fallback_context = TurnContext {
        calls: 0,
        dispatch: DispatchRoot::pin(&controller),
        runtime: SharedRuntime::from_shared_image(image.clone())?,
    };
    let fallback = handler.handle(&mut fallback_context, 40)?;

    let candidate = controller.stage_current(&runtime)?;
    let previous = controller.activate(candidate)?;
    let mut active_context = TurnContext {
        calls: 0,
        dispatch: DispatchRoot::pin(&controller),
        runtime: SharedRuntime::from_shared_image(image.clone())?,
    };
    let active = handler.handle(&mut active_context, 40)?;
    let adjacent = handler.adjacent(40);

    controller.rollback(previous)?;
    let mut rolled_back_context = TurnContext {
        calls: 0,
        dispatch: DispatchRoot::pin(&controller),
        runtime: SharedRuntime::from_shared_image(image)?,
    };
    let rolled_back = handler.handle(&mut rolled_back_context, 40)?;

    println!(
        "replaceable_handler fallback={fallback}/{} active={active}/{} adjacent={adjacent} rollback={rolled_back}/{}",
        fallback_context.calls(),
        active_context.calls(),
        rolled_back_context.calls()
    );
    Ok(())
}
