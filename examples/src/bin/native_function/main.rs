#![cfg_attr(not(test), deny(clippy::wildcard_imports))]
#![allow(clippy::result_large_err)]

use std::collections::BTreeMap;
use std::error::Error;

use vela_engine::prelude::*;
use vela_macros::{ScriptHost, export};

fn main() -> Result<(), Box<dyn Error>> {
    let mut bindings = VelaBindings::new();
    bindings.register_type(Player::vela_type());
    bindings.register_module(vela_function_bonus_macro());
    bindings.register_module(vela_function_collection_bonus());
    bindings.register_module(vela_function_grant_level());

    let engine = Engine::builder()
        .capability(Capability::HostRead)
        .capability(Capability::HostWrite)
        .register_bindings(bindings)
        .register_typed_native_fn::<(i64, i64), _>(
            NativeFunctionDesc::new("game::bonus_manual", NativeFunctionId::new(10_001))
                .param("amount", TypeHint::i64())
                .param("multiplier", TypeHint::i64())
                .returns(TypeHint::i64())
                .effects(EffectSet::pure())
                .access(FunctionAccess::public().reflect_callable(true)),
            bonus_manual,
        )
        .build()?;
    let program = engine.compile_source(include_str!("main.vela"))?;
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");
    let mut player = Player { level: 1 };

    let output = runtime.call(
        "main",
        CallArgs::new().with_host_mut("player", &mut player),
        CallOptions::new(10_000, 1024 * 1024, 64),
    )?;

    println!(
        "native_function_result={:?} final_level={}",
        runtime.value_to_owned(&output)?,
        player.level
    );
    Ok(())
}

#[derive(Debug, ScriptHost)]
#[vela(path = "examples::native_function::Player")]
pub struct Player {
    #[vela(get, set, hint = "i64")]
    level: i64,
}

fn bonus_manual(amount: i64, multiplier: i64) -> i64 {
    amount * multiplier
}

#[export(path = "game::bonus_macro")]
pub fn bonus_macro(amount: i64, extra: i64) -> i64 {
    amount + extra
}

#[export(path = "game::collection_bonus")]
pub fn collection_bonus(scores: BTreeMap<String, i64>, tags: Vec<String>) -> i64 {
    scores.values().sum::<i64>() + i64::try_from(tags.len()).unwrap_or_default()
}

#[export(path = "game::grant_level")]
pub fn grant_level(player: &mut Player, amount: i64) -> i64 {
    player.level += amount;
    player.level
}
