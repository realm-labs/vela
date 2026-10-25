#![cfg_attr(not(test), deny(clippy::wildcard_imports))]

use std::error::Error;

use vela_engine::prelude::*;
use vela_examples::expect_error;

const SOURCE_LABEL: &str = "generic_type_hint_denied.vela";
const SOURCE: &str = include_str!("generic_type_hint_denied.vela");

fn main() -> Result<(), Box<dyn Error>> {
    expect_error(
        || {
            let engine = Engine::builder().build()?;
            let program = engine.compile_source(SOURCE).map_err(|error| {
                vela_examples::diagnostics::render_engine_source_error(SOURCE_LABEL, SOURCE, &error)
            })?;
            let mut runtime = Runtime::new(engine, program)?;
            runtime
                .call(
                    "main",
                    CallArgs::new(),
                    CallOptions::new(10_000, 1024 * 1024, 64),
                )
                .map_err(|error| {
                    vela_examples::diagnostics::render_vm_error(SOURCE_LABEL, SOURCE, &error)
                })?;
            Ok(())
        },
        "only builtin container, Option, and Result type hints support type arguments",
    )
}
