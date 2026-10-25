use std::error::Error;
use std::path::PathBuf;

use vela_bindgen::RustBindingsBuilder;
use vela_engine::binding::VmResult;
use vela_engine::context::NativeCallContext;
use vela_engine::engine::Engine;
use vela_macros::export;

#[export(path = "test::deny_effect_expansion")]
pub fn deny_effect_expansion(_context: &mut NativeCallContext<'_>) -> VmResult<i64> {
    Ok(0)
}

#[export(path = "test::deny_context_random")]
pub fn deny_context_random(_context: &mut NativeCallContext<'_>) -> VmResult<i64> {
    Ok(0)
}

fn main() -> Result<(), Box<dyn Error>> {
    let engine = Engine::builder()
        .install_registration(vela_function_deny_effect_expansion())
        .install_registration(vela_function_deny_context_random())
        .with_controlled_random(7)
        .build()
        .map_err(|error| error.to_string())?;
    let program = engine
        .compile_source(include_str!("script.vela"))
        .map_err(|error| error.to_string())?;
    let generated = RustBindingsBuilder::new(program.binding_schema()).generate()?;
    let output = PathBuf::from(std::env::var_os("OUT_DIR").ok_or("OUT_DIR is not set")?)
        .join("vela_bindings.rs");
    std::fs::write(output, generated.code)?;
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=script.vela");
    Ok(())
}
