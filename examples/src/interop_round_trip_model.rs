use std::error::Error;

use vela_engine::engine::Engine;
use vela_macros::{ScriptHost, ScriptReflect, export, methods};

#[derive(Debug, ScriptHost, ScriptReflect)]
#[script(path = "examples::interop::RoundTripPlayer")]
pub struct RoundTripPlayer {
    #[script(get, set)]
    pub level: i64,
}

#[methods(path = "examples::interop::RoundTripPlayer")]
impl RoundTripPlayer {
    pub fn grant(&mut self, amount: i64) -> i64 {
        self.level += amount;
        self.level
    }

    pub fn level(&self) -> i64 {
        self.level
    }
}

#[export(path = "interop::normalize")]
pub fn normalize(amount: i64) -> i64 {
    amount.max(0)
}

pub fn build_engine() -> Result<Engine, Box<dyn Error>> {
    Ok(Engine::builder()
        .register_host_type::<RoundTripPlayer>()
        .register_exports(RoundTripPlayer::vela_inherent_exports())
        .register_exports(vela_export_bundle_normalize())
        .capability(vela_common::Capability::HostRead)
        .capability(vela_common::Capability::HostWrite)
        .build()?)
}
