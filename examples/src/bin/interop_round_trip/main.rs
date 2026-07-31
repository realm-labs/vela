#![cfg_attr(not(test), deny(clippy::wildcard_imports))]

use std::error::Error;

use vela_engine::runtime::Runtime;
use vela_examples::interop_round_trip_model::{RoundTripPlayer, RoundTripStats, build_engine};

include!(concat!(env!("OUT_DIR"), "/interop_round_trip_bindings.rs"));

fn main() -> Result<(), Box<dyn Error>> {
    let engine = build_engine()?;
    let program = engine.compile_source(include_str!("main.vela"))?;
    let mut runtime = Runtime::new(engine, program)?;
    let mut player = RoundTripPlayer {
        level: 1,
        stats: RoundTripStats { granted: 0 },
    };

    let mut package = vela_bindings::bind(&mut runtime)?;
    let mut module = package.dev_vela_anonymous_root_module();
    let result = module.apply(&mut player, 5)?;

    println!(
        "interop_round_trip result={result} level={} recorded={}",
        player.level, player.stats.granted
    );
    Ok(())
}
