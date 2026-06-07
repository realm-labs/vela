use std::error::Error;

use serde::{Deserialize, Serialize};
use vela_engine::prelude::*;
use vela_examples::example_file;

fn main() -> Result<(), Box<dyn Error>> {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()?;
    let program = engine.compile_file(example_file("serde_value", "main.vela"))?;
    let mut runtime = Runtime::new(engine, program);

    let event = DamageEvent {
        actor: DamageActor {
            name: "player-1001".to_owned(),
            level: 7,
        },
        amount: 9,
        multiplier: 3,
        reason: "slash".to_owned(),
    };

    let args = CallArgs::new().with_serde_value("event", &event)?;
    let output = runtime.call("handle_damage", args, CallOptions::unbounded())?;
    let result: DamageResult = runtime.from_value(&output)?;

    assert_eq!(event.amount, 9);
    assert_eq!(
        result,
        DamageResult {
            actor_name: "player-1001".to_owned(),
            applied: 34,
            label: "slash".to_owned(),
        }
    );

    println!(
        "serde_value actor={} applied={} label={} original_amount={}",
        result.actor_name, result.applied, result.label, event.amount
    );
    Ok(())
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
struct DamageActor {
    name: String,
    level: i64,
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
struct DamageEvent {
    actor: DamageActor,
    amount: i64,
    multiplier: i64,
    reason: String,
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
struct DamageResult {
    actor_name: String,
    applied: i64,
    label: String,
}
