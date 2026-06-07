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

    let initial_state = runtime
        .call("make_state", CallArgs::new(), CallOptions::unbounded())?
        .into_value();
    runtime.insert_script_global(STATE_GLOBAL, initial_state)?;

    let first_tick = runtime
        .call(
            "handle_tick",
            CallArgs::from_positional([OwnedValue::Int(2), OwnedValue::Int(5)]),
            CallOptions::unbounded(),
        )?
        .into_value();

    runtime.update_script_global(STATE_GLOBAL, |value| {
        let OwnedValue::Record { fields, .. } = value else {
            panic!("state global should remain a ServerState record");
        };
        fields
            .set_existing("name", OwnedValue::String("rust-updated".to_owned()))
            .expect("name field should exist");
        fields
            .set_existing("level", OwnedValue::Int(10))
            .expect("level field should exist");
    })?;

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
    let final_state = runtime
        .script_global(STATE_GLOBAL)?
        .expect("state global should exist");

    assert_eq!(first_tick, OwnedValue::Int(8));
    assert_eq!(second_tick, OwnedValue::Int(19));
    assert_eq!(state_name, OwnedValue::String("rust-updated".to_owned()));
    assert_eq!(
        record_field(&final_state, "level"),
        Some(&OwnedValue::Int(11))
    );
    assert_eq!(
        record_field(&final_state, "total_gold"),
        Some(&OwnedValue::Int(8))
    );

    println!(
        "script_global first={first_tick:?} second={second_tick:?} name={state_name:?} \
         final_level={:?} final_gold={:?}",
        record_field(&final_state, "level").expect("level field should exist"),
        record_field(&final_state, "total_gold").expect("total_gold field should exist")
    );

    Ok(())
}

fn record_field<'value>(value: &'value OwnedValue, field: &str) -> Option<&'value OwnedValue> {
    let OwnedValue::Record { fields, .. } = value else {
        return None;
    };
    fields.get(field)
}
