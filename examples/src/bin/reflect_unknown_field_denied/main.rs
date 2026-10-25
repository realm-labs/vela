#![cfg_attr(not(test), deny(clippy::wildcard_imports))]

use std::error::Error;

use vela_engine::prelude::*;
use vela_examples::expect_error;
use vela_reflect::permissions::ReflectPolicy;

const SOURCE_LABEL: &str = "reflect_unknown_field_denied.vela";
const SOURCE: &str = include_str!("reflect_unknown_field_denied.vela");

fn main() -> Result<(), Box<dyn Error>> {
    expect_error(
        || {
            let engine = Engine::builder()
                .with_standard_natives()
                .reflection_policy(ReflectPolicy::all())
                .capability(Capability::ReflectionRead)
                .capability(Capability::ReflectionWrite)
                .build()?;
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
        "unknown reflected field `leve`",
    )
}
