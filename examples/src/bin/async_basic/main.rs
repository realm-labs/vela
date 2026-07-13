#![cfg_attr(not(test), deny(clippy::wildcard_imports))]

use std::error::Error;

use vela_engine::prelude::*;
use vela_examples::async_executor::block_on;

const SOURCE: &str = include_str!("main.vela");

fn main() -> Result<(), Box<dyn Error>> {
    let engine = Engine::builder().build()?;
    let program = engine.compile_source(SOURCE)?;
    let mut runtime = Runtime::new(engine, program);
    let output = block_on(runtime.call_async(
        "main",
        CallArgs::new(),
        CallOptions::unbounded(),
    ))?;

    println!("async_basic result={:?}", runtime.value_to_owned(&output)?);
    Ok(())
}
