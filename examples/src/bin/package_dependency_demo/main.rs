use std::fs;
use std::path::Path;
use vela_examples::temp_dir::TempDir;

use vela_engine::engine::Engine;
use vela_engine::runtime::{CallArgs, CallOptions, Runtime};
use vela_engine::source::PackageCompileRequest;
use vela_package::PackageId;
use vela_vm::owned_value::OwnedValue;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (before, after) = run_demo()?;
    println!("ordinary package dependency reload: {before} -> {after}");
    Ok(())
}

fn run_demo() -> Result<(i64, i64), Box<dyn std::error::Error>> {
    let owned = TempDir::new()?;
    let root = owned.path();
    write_fixture(root)?;
    let engine = Engine::builder().build()?;
    let snapshot = engine.load_package_workspace(root.join("app/vela.toml"))?;
    let app = PackageId::new("dev.vela.example.app")?;
    let request = PackageCompileRequest::for_root(&snapshot, &app);
    let initial = engine.compile_package_hot_reload_initial(&snapshot, &request)?;
    let mut runtime = Runtime::from_hot_reload_version(engine.clone(), initial.clone())
        .expect("runtime should initialize");
    let before = call_main(&mut runtime)?;

    fs::write(
        root.join("app/lib/src/api.vela"),
        include_str!("fixtures/updates/api.v2.vela"),
    )?;
    let update = engine.compile_package_workspace_hot_reload_update_from_previous(
        &initial,
        root.join("app/vela.toml"),
    )?;
    runtime.stage_reload_update(update)?;
    runtime.activate_reload()?.ok_or("missing reload report")?;
    let after = call_main(&mut runtime)?;

    Ok((before, after))
}

fn write_fixture(root: &Path) -> std::io::Result<()> {
    fs::create_dir_all(root.join("app/src"))?;
    fs::create_dir_all(root.join("app/lib/src"))?;
    fs::write(
        root.join("app/vela.toml"),
        include_str!("fixtures/app/vela.toml"),
    )?;
    fs::write(
        root.join("app/src/main.vela"),
        include_str!("fixtures/app/src/main.vela"),
    )?;
    fs::write(
        root.join("app/lib/vela.toml"),
        include_str!("fixtures/app/lib/vela.toml"),
    )?;
    fs::write(
        root.join("app/lib/src/api.vela"),
        include_str!("fixtures/app/lib/src/api.vela"),
    )
}

fn call_main(runtime: &mut Runtime) -> Result<i64, Box<dyn std::error::Error>> {
    let value = runtime.call("main::main", CallArgs::new(), CallOptions::unbounded())?;
    match runtime.value_to_owned(&value)? {
        OwnedValue::Scalar(vela_common::ScalarValue::I64(value)) => Ok(value),
        other => Err(format!("expected i64, got {other:?}").into()),
    }
}

#[test]
fn example_ordinary_package_dependency_compiles_runs_and_reloads() {
    assert_eq!(run_demo().expect("ordinary package demo"), (42, 43));
}
