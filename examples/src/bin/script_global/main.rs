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

    let first_tick = runtime.call_value(
        "handle_tick",
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

    let second_tick = runtime.call_value(
        "handle_tick",
        CallArgs::from_positional([OwnedValue::Int(1), OwnedValue::Int(3)]),
        CallOptions::unbounded(),
    )?;
    let state_name = runtime.call_value("state_name", CallArgs::new(), CallOptions::unbounded())?;
    let tick_count = runtime.call_value("tick_count", CallArgs::new(), CallOptions::unbounded())?;
    let state_snapshot =
        runtime.call_value("snapshot_state", CallArgs::new(), CallOptions::unbounded())?;
    let projected_score = runtime.call_value(
        "projected_score",
        CallArgs::new()
            .with_vela_value(state_snapshot.clone())
            .with(OwnedValue::Int(4)),
        CallOptions::unbounded(),
    )?;
    runtime.insert_global(STATE_GLOBAL, state_snapshot)?;
    let final_state = runtime
        .global(STATE_GLOBAL)?
        .expect("state global should exist");

    let first_tick = runtime.value_to_owned(&first_tick)?;
    let second_tick = runtime.value_to_owned(&second_tick)?;
    let state_name = runtime.value_to_owned(&state_name)?;
    let tick_count = runtime.value_to_owned(&tick_count)?;
    let projected_score = runtime.value_to_owned(&projected_score)?;

    assert_eq!(first_tick, OwnedValue::Int(9));
    assert_eq!(second_tick, OwnedValue::Int(27));
    assert_eq!(state_name, OwnedValue::String("rust-updated".to_owned()));
    assert_eq!(tick_count, OwnedValue::Int(8));
    assert_eq!(projected_score, OwnedValue::Int(31));
    assert_eq!(final_state.field("level"), Some(&OwnedValue::Int(11)));
    assert_eq!(final_state.field("total_gold"), Some(&OwnedValue::Int(8)));
    assert_eq!(
        final_state
            .field("stats")
            .and_then(|stats| stats.field("handled_ticks")),
        Some(&OwnedValue::Int(8))
    );

    println!(
        "script_global first={first_tick:?} second={second_tick:?} name={state_name:?} \
         projected={projected_score:?} final_level={:?} final_gold={:?} ticks={tick_count:?}",
        final_state
            .field("level")
            .expect("level field should exist"),
        final_state
            .field("total_gold")
            .expect("total_gold field should exist")
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
