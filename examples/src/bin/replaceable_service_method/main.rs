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
#[script(path = "examples::pricing::PricingService")]
struct PricingService {
    #[script(get)]
    base: i64,
}

struct PricingTurn {
    dispatch: DispatchRoot,
    runtime: SharedRuntime,
}

impl DispatchAuthority for PricingTurn {
    fn vela_dispatch_root(&self) -> &DispatchRoot {
        &self.dispatch
    }

    fn vela_dispatch_invocation(&mut self) -> VmResult<DispatchInvocation<'_>> {
        let Self { dispatch, runtime } = self;
        dispatch.invocation(runtime)
    }
}

#[methods(path = "host::pricing::PricingService")]
impl PricingService {
    #[vela_macros::replaceable(
        path = "host::pricing::PricingService::quote",
        authority = "turn",
        index = 0
    )]
    pub fn quote(&self, turn: &mut PricingTurn, value: i64) -> VmResult<i64> {
        let _ = turn;
        Ok(self.adjacent(value))
    }

    pub fn adjacent(&self, value: i64) -> i64 {
        value + self.base
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let slots = PricingService::vela_replaceable_slots();
    let engine = Engine::builder()
        .register_host_type::<PricingService>()
        .register_exports(PricingService::vela_inherent_exports())
        .register_replaceable_slots(slots.clone())
        .capability(vela_common::Capability::HostRead)
        .build()?;
    let program = engine.compile_source(include_str!("main.vela"))?;
    let image = RuntimeImage::new_compiled(engine, program).into_shared();
    let runtime = SharedRuntime::from_shared_image(image.clone())?;
    let controller = DispatchController::new(slots)?;

    let fallback_service = PricingService { base: 1 };
    let mut fallback_turn = PricingTurn {
        dispatch: DispatchRoot::pin(&controller),
        runtime: SharedRuntime::from_shared_image(image.clone())?,
    };
    let fallback = fallback_service.quote(&mut fallback_turn, 40)?;

    let candidate = controller.stage_current(&runtime)?;
    let previous = controller.activate(candidate)?;
    let active_service = PricingService { base: 1 };
    let mut active_turn = PricingTurn {
        dispatch: DispatchRoot::pin(&controller),
        runtime: SharedRuntime::from_shared_image(image.clone())?,
    };
    let active = active_service.quote(&mut active_turn, 40)?;
    let adjacent = active_service.adjacent(40);

    controller.rollback(previous)?;
    let rolled_back_service = PricingService { base: 1 };
    let mut rolled_back_turn = PricingTurn {
        dispatch: DispatchRoot::pin(&controller),
        runtime: SharedRuntime::from_shared_image(image)?,
    };
    let rolled_back = rolled_back_service.quote(&mut rolled_back_turn, 40)?;

    println!(
        "replaceable_service_method fallback={fallback} active={active} adjacent={adjacent} rollback={rolled_back}"
    );
    Ok(())
}
