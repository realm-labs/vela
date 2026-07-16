use std::error::Error;
use std::path::PathBuf;

use vela_bindgen::{RustBindingGeneratorOptions, generate_rust_bindings};

#[path = "src/interop_round_trip_model.rs"]
mod interop_round_trip_model;

fn main() -> Result<(), Box<dyn Error>> {
    let engine = interop_round_trip_model::build_engine()?;
    let source = std::fs::read_to_string("src/bin/interop_round_trip/main.vela")?;
    let program = engine.compile_source(&source)?;
    let generated = generate_rust_bindings(
        program.binding_schema(),
        &RustBindingGeneratorOptions::default(),
    )?;
    let output = PathBuf::from(std::env::var_os("OUT_DIR").ok_or("OUT_DIR is not set")?)
        .join("interop_round_trip_bindings.rs");
    std::fs::write(output, generated.code)?;
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=src/interop_round_trip_model.rs");
    println!("cargo::rerun-if-changed=src/bin/interop_round_trip/main.vela");
    Ok(())
}
