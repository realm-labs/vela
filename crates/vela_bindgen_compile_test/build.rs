use std::error::Error;
use std::path::PathBuf;

use vela_bindgen::{RustBindingGeneratorOptions, generate_rust_bindings};
use vela_engine::binding::VmResult;
use vela_engine::context::NativeCallContext;
use vela_engine::engine::Engine;
use vela_macros::export;

#[path = "src/model.rs"]
mod model;

#[export(path = "test::reenter_player")]
pub fn reenter_player(
    _context: &mut NativeCallContext<'_, '_>,
    player: &mut model::Player,
    amount: i64,
) -> VmResult<i64> {
    Ok(player.level + amount)
}

#[export(path = "test::reject_unrelated")]
pub fn reject_unrelated(
    _context: &mut NativeCallContext<'_, '_>,
    player: &mut model::Player,
) -> VmResult<i64> {
    Ok(player.level)
}

fn main() -> Result<(), Box<dyn Error>> {
    let engine = Engine::builder()
        .register_host_type::<model::Player>()
        .register_exports(vela_export_bundle_reenter_player())
        .register_exports(vela_export_bundle_reject_unrelated())
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
