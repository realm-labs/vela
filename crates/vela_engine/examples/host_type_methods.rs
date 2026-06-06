#![allow(clippy::result_large_err)]

#[path = "host_type_methods/support.rs"]
mod support;

use std::error::Error;

use support::{IntIntMap, Player, RewardSink, TagSet, host_engine, script_path};
use vela_engine::prelude::*;

fn main() -> Result<(), Box<dyn Error>> {
    let engine = host_engine()?;
    let program = engine.compile_file(script_path())?;
    let mut runtime = Runtime::new(engine, program);

    let mut player = Player::new();
    let mut scores = IntIntMap::default();
    let mut tags = TagSet::from(["vip"]);
    let mut rewards = RewardSink::default();

    let output = runtime.call(
        "main",
        CallArgs::new()
            .with_host_mut("player", &mut player)
            .with_host_mut("scores", &mut scores)
            .with_host_mut("tags", &mut tags)
            .with_host_mut("rewards", &mut rewards),
        CallOptions::new(10_000, 1024 * 1024, 64),
    )?;

    println!(
        "script_result={:?} final_count={} score={} reward_calls={}",
        output.value(),
        player.gold_count(),
        scores.get(1001).unwrap_or_default(),
        rewards.grant_count() + player.reward_sink_grant_count()
    );

    Ok(())
}
