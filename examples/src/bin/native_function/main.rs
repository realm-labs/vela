#![cfg_attr(not(test), deny(clippy::wildcard_imports))]
#![allow(clippy::result_large_err)]

use std::collections::BTreeMap;
use std::error::Error;

use vela_engine::prelude::*;
use vela_macros::export;

fn main() -> Result<(), Box<dyn Error>> {
    let mut bindings = VelaBindings::new();
    bindings.register_module(vela_function_bonus_macro());
    bindings.register_module(vela_function_collection_bonus());

    let engine = Engine::builder()
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

    let output = runtime.call(
        "main",
        CallArgs::new(),
        CallOptions::new(10_000, 1024 * 1024, 64),
    )?;

    println!(
        "native_function_result={:?}",
        runtime.value_to_owned(&output)?
    );
    Ok(())
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
