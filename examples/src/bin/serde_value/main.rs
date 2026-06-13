use std::error::Error;

use serde::{Deserialize, Serialize};
use vela_common::SourceId;
use vela_engine::prelude::*;

fn main() -> Result<(), Box<dyn Error>> {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()?;
    let program = engine.compile_source(SourceId::new(1), include_str!("main.vela"))?;
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
    let score_method = runtime.method(&output, "score")?;
    let score = runtime.call_method(
        &output,
        &score_method,
        CallArgs::new().with_value("bonus", 5_i64),
        CallOptions::unbounded(),
    )?;
    let result: DamageResult = runtime.from_value(&output)?;
    let score: i64 = runtime.from_value(&score)?;

    assert_eq!(event.amount, 9);
    assert_eq!(
        result,
        DamageResult {
            actor_name: "player-1001".to_owned(),
            applied: 34,
            label: "slash".to_owned(),
        }
    );
    assert_eq!(score, 39);

    println!(
        "serde_value actor={} applied={} score={} label={} original_amount={}",
        result.actor_name, result.applied, score, result.label, event.amount
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
