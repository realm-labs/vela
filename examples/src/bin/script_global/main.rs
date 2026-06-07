use std::error::Error;

use serde::{Deserialize, Serialize};
use vela_engine::prelude::*;
use vela_examples::example_file;

const STATE_GLOBAL: &str = "main::state";

fn main() -> Result<(), Box<dyn Error>> {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()?;
    let program = engine.compile_file(example_file("script_global", "main.vela"))?;
    let mut runtime = Runtime::new(engine, program);

    let initial_state = ServerState {
        level: 1,
        name: "boot".to_owned(),
        total_gold: 0,
        stats: ServerStats { handled_ticks: 0 },
    };
    runtime.insert_global(STATE_GLOBAL, &initial_state)?;
    let handle_tick = runtime.entry("handle_tick")?;
    let state_name_fn = runtime.entry("state_name")?;
    let tick_count_fn = runtime.entry("tick_count")?;
    let snapshot_state = runtime.entry("snapshot_state")?;
    let projected_score_fn = runtime.entry("projected_score")?;

    let first_tick = runtime.call(
        &handle_tick,
        CallArgs::from_positional([OwnedValue::Int(2), OwnedValue::Int(5)]),
        CallOptions::unbounded(),
    )?;

    let rust_updated_state = ServerState {
        level: 10,
        name: "rust-updated".to_owned(),
        total_gold: 5,
        stats: ServerStats { handled_ticks: 7 },
    };
    runtime.set_global(STATE_GLOBAL, &rust_updated_state)?;

    let second_tick = runtime.call(
        &handle_tick,
        CallArgs::from_positional([OwnedValue::Int(1), OwnedValue::Int(3)]),
        CallOptions::unbounded(),
    )?;
    let state_name = runtime.call(&state_name_fn, CallArgs::new(), CallOptions::unbounded())?;
    let tick_count = runtime.call(&tick_count_fn, CallArgs::new(), CallOptions::unbounded())?;
    let state_snapshot =
        runtime.call(&snapshot_state, CallArgs::new(), CallOptions::unbounded())?;
    let projected_score = runtime.call(
        &projected_score_fn,
        CallArgs::new()
            .with_vela_value(state_snapshot.clone())
            .with(OwnedValue::Int(4)),
        CallOptions::unbounded(),
    )?;
    runtime.insert_global(STATE_GLOBAL, state_snapshot)?;
    let final_state: ServerState = runtime
        .global_as(STATE_GLOBAL)?
        .expect("state global should exist");

    let first_tick: i64 = runtime.from_value(&first_tick)?;
    let second_tick: i64 = runtime.from_value(&second_tick)?;
    let state_name: String = runtime.from_value(&state_name)?;
    let tick_count: i64 = runtime.from_value(&tick_count)?;
    let projected_score: i64 = runtime.from_value(&projected_score)?;

    assert_eq!(first_tick, 9);
    assert_eq!(second_tick, 27);
    assert_eq!(state_name, "rust-updated");
    assert_eq!(tick_count, 8);
    assert_eq!(projected_score, 31);
    assert_eq!(final_state.level, 11);
    assert_eq!(final_state.total_gold, 8);
    assert_eq!(final_state.stats.handled_ticks, 8);

    println!(
        "script_global first={first_tick} second={second_tick} name={state_name} \
         projected={projected_score} final_level={} final_gold={} ticks={tick_count}",
        final_state.level, final_state.total_gold
    );

    Ok(())
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ServerStats {
    handled_ticks: i64,
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ServerState {
    level: i64,
    name: String,
    total_gold: i64,
    stats: ServerStats,
}
