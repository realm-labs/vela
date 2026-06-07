use std::error::Error;

use vela_engine::prelude::*;
use vela_examples::example_file;

const STATE_GLOBAL: &str = "main::state";

fn main() -> Result<(), Box<dyn Error>> {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()?;
    let program = engine.compile_file(example_file("script_global", "main.vela"))?;
    let mut runtime = Runtime::new(engine, program);

    let initial_state = OwnedValue::record(
        "ServerState",
        vec![
            ("level", 1.into()),
            ("name", "boot".into()),
            ("total_gold", 0.into()),
            (
                "stats",
                OwnedValue::record("ServerStats", [("handled_ticks", 0)]),
            ),
        ],
    );
    runtime.insert_script_global(STATE_GLOBAL, initial_state)?;

    let first_tick = runtime
        .call(
            "handle_tick",
            CallArgs::from_positional([OwnedValue::Int(2), OwnedValue::Int(5)]),
            CallOptions::unbounded(),
        )?
        .into_value();

    runtime.set_script_global(
        STATE_GLOBAL,
        owned_record!("ServerState", {
            "level" => 10,
            "name" => "rust-updated",
            "total_gold" => 5,
            "stats" => owned_record!("ServerStats", {
                "handled_ticks" => 7,
            }),
        }),
    )?;

    let second_tick = runtime
        .call(
            "handle_tick",
            CallArgs::from_positional([OwnedValue::Int(1), OwnedValue::Int(3)]),
            CallOptions::unbounded(),
        )?
        .into_value();
    let state_name = runtime
        .call("state_name", CallArgs::new(), CallOptions::unbounded())?
        .into_value();
    let tick_count = runtime
        .call("tick_count", CallArgs::new(), CallOptions::unbounded())?
        .into_value();
    let final_state = runtime
        .script_global(STATE_GLOBAL)?
        .expect("state global should exist");

    assert_eq!(first_tick, OwnedValue::Int(9));
    assert_eq!(second_tick, OwnedValue::Int(27));
    assert_eq!(state_name, OwnedValue::String("rust-updated".to_owned()));
    assert_eq!(tick_count, OwnedValue::Int(8));
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
         final_level={:?} final_gold={:?} ticks={tick_count:?}",
        final_state
            .field("level")
            .expect("level field should exist"),
        final_state
            .field("total_gold")
            .expect("total_gold field should exist")
    );

    Ok(())
}
