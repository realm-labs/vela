use std::error::Error;

use vela_engine::engine::Engine;
use vela_engine::registration::VelaBindings;
use vela_macros::{ScriptHost, ScriptReflect, export, methods};

#[derive(Debug, ScriptHost, ScriptReflect)]
#[vela(path = "examples::interop::RoundTripStats")]
pub struct RoundTripStats {
    #[vela(get)]
    pub granted: i64,
}

#[methods(path = "examples::interop::RoundTripStats")]
impl RoundTripStats {
    pub fn record(&mut self, amount: i64) {
        self.granted += amount;
    }
}

#[derive(Debug, ScriptHost, ScriptReflect)]
#[vela(path = "examples::interop::RoundTripPlayer")]
pub struct RoundTripPlayer {
    #[vela(get, set)]
    pub level: i64,
    #[vela(skip)]
    pub stats: RoundTripStats,
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

    pub fn stats_mut(&mut self) -> &mut RoundTripStats {
        &mut self.stats
    }
}

#[export(path = "interop::normalize")]
pub fn normalize(amount: i64) -> i64 {
    amount.max(0)
}

pub fn build_engine() -> Result<Engine, Box<dyn Error>> {
    let mut bindings = VelaBindings::new();
    bindings
        .register_type(RoundTripStats::vela_type())
        .register_methods(RoundTripStats::vela_methods());
    bindings
        .register_type(RoundTripPlayer::vela_type())
        .register_methods(RoundTripPlayer::vela_methods());
    bindings.register_module(vela_function_normalize());

    Ok(Engine::builder()
        .register_bindings(bindings)
        .capability(vela_common::Capability::HostRead)
        .capability(vela_common::Capability::HostWrite)
        .build()?)
}
