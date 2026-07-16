use std::error::Error;
use std::path::PathBuf;

use vela_bindgen::{RustBindingGeneratorOptions, generate_rust_bindings};
use vela_engine::engine::Engine;

fn main() -> Result<(), Box<dyn Error>> {
    let engine = Engine::builder()
        .build()
        .map_err(|error| error.to_string())?;
    let program = engine
        .compile_source(include_str!("script.vela"))
        .map_err(|error| error.to_string())?;
    let generated = generate_rust_bindings(
        program.binding_schema(),
        &RustBindingGeneratorOptions::default(),
    )?;
    let output = PathBuf::from(std::env::var_os("OUT_DIR").ok_or("OUT_DIR is not set")?)
        .join("vela_bindings.rs");
    std::fs::write(output, generated.code)?;
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=script.vela");
    Ok(())
}
