#![cfg_attr(not(test), deny(clippy::wildcard_imports))]

use std::error::Error;
use std::sync::Arc;

use parking_lot::Mutex;
use vela_engine::binding::VmResult;
use vela_engine::dispatch::{DispatchAuthority, DispatchController, DispatchRoot};
use vela_engine::engine::Engine;
use vela_engine::runtime::Runtime;
use vela_macros::{ScriptHost, ScriptReflect, methods};

#[derive(ScriptHost, ScriptReflect)]
#[script(path = "examples::handler::TurnContext")]
struct TurnContext {
    #[script(get, set)]
    calls: i64,
    #[script(skip)]
    dispatch: DispatchRoot,
}

impl DispatchAuthority for TurnContext {
    fn vela_dispatch_root(&self) -> &DispatchRoot {
        &self.dispatch
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
    let engine = Engine::builder()
        .register_host_type::<TurnContext>()
        .register_host_type::<MessageHandler>()
        .register_exports(MessageHandler::vela_inherent_exports())
        .capability(vela_common::Capability::HostRead)
        .capability(vela_common::Capability::HostWrite)
        .build()?;
    let program = engine.compile_source(include_str!("main.vela"))?;
    let runtime = Arc::new(Mutex::new(Runtime::new(engine, program)?));
    let controller = DispatchController::new(vec![MessageHandler::vela_replaceable_slot_handle()])?;
    let handler = MessageHandler { bonus: 1 };

    let mut fallback_context = TurnContext {
        calls: 0,
        dispatch: DispatchRoot::pin(&controller),
    };
    let fallback = handler.handle(&mut fallback_context, 40)?;

    let candidate = controller.stage_current(&runtime)?;
    let previous = controller.activate(candidate)?;
    let mut active_context = TurnContext {
        calls: 0,
        dispatch: DispatchRoot::pin(&controller),
    };
    let active = handler.handle(&mut active_context, 40)?;
    let adjacent = handler.adjacent(40);

    controller.rollback(previous)?;
    let mut rolled_back_context = TurnContext {
        calls: 0,
        dispatch: DispatchRoot::pin(&controller),
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
