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
#[script(path = "examples::pricing::PricingService")]
struct PricingService {
    #[script(get)]
    base: i64,
    #[script(skip)]
    dispatch: DispatchRoot,
}

impl DispatchAuthority for PricingService {
    fn vela_dispatch_root(&self) -> &DispatchRoot {
        &self.dispatch
    }
}

#[methods(path = "host::pricing::PricingService")]
impl PricingService {
    #[vela_macros::replaceable(
        path = "host::pricing::PricingService::quote",
        authority = "self",
        index = 0
    )]
    pub fn quote(&self, value: i64) -> VmResult<i64> {
        Ok(self.adjacent(value))
    }

    pub fn adjacent(&self, value: i64) -> i64 {
        value + self.base
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let engine = Engine::builder()
        .register_host_type::<PricingService>()
        .register_exports(PricingService::vela_inherent_exports())
        .capability(vela_common::Capability::HostRead)
        .build()?;
    let program = engine.compile_source(include_str!("main.vela"))?;
    let runtime = Arc::new(Mutex::new(Runtime::new(engine, program)?));
    let controller = DispatchController::new(vec![PricingService::vela_replaceable_slot_quote()])?;

    let fallback_service = PricingService {
        base: 1,
        dispatch: DispatchRoot::pin(&controller),
    };
    let fallback = fallback_service.quote(40)?;

    let candidate = controller.stage_current(&runtime)?;
    let previous = controller.activate(candidate);
    let active_service = PricingService {
        base: 1,
        dispatch: DispatchRoot::pin(&controller),
    };
    let active = active_service.quote(40)?;
    let adjacent = active_service.adjacent(40);

    controller.rollback(previous);
    let rolled_back_service = PricingService {
        base: 1,
        dispatch: DispatchRoot::pin(&controller),
    };
    let rolled_back = rolled_back_service.quote(40)?;

    println!(
        "replaceable_service_method fallback={fallback} active={active} adjacent={adjacent} rollback={rolled_back}"
    );
    Ok(())
}
