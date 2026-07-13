use std::fs;
use std::path::{Path, PathBuf};

use vela_engine::engine::Engine;
use vela_engine::runtime::{CallArgs, CallOptions, ProviderHandle, Runtime};
use vela_engine::source::ProviderCompileRequest;
use vela_vm::owned_value::OwnedValue;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (before, after) = run_demo()?;
    println!("plugin provider reload: {before} -> {after}");
    Ok(())
}

fn run_demo() -> Result<(i64, i64), Box<dyn std::error::Error>> {
    let root = fixture_root("plugin_provider_demo");
    write_fixture(&root, 1)?;
    let engine = Engine::builder().build()?;
    let snapshot = engine.load_package_workspace(root.join("plugin/vela.toml"))?;
    let catalog = engine.discover_providers(&snapshot)?;
    let descriptor = catalog.providers().first().ok_or("missing provider")?;
    let key = descriptor.key().clone();
    let method = descriptor.methods().first().ok_or("missing method")?.id();
    let selection = catalog.select([key.clone()])?;
    let request = ProviderCompileRequest::for_selection(&snapshot, selection);
    let initial = engine.compile_provider_hot_reload_initial(&snapshot, &request)?;
    let mut runtime = Runtime::from_hot_reload_version(engine.clone(), initial.clone());
    let handle = runtime.provider_handle(&key)?;
    let before = call_provider(&mut runtime, &handle, method)?;

    write_provider_source(&root, 2)?;
    let update = engine.compile_package_workspace_hot_reload_update_from_previous(
        &initial,
        root.join("plugin/vela.toml"),
    )?;
    runtime.stage_hot_update(update)?;
    runtime.check_reload()?.ok_or("missing reload report")?;
    let after = call_provider(&mut runtime, &handle, method)?;
    fs::remove_dir_all(root)?;
    Ok((before, after))
}

fn write_fixture(root: &Path, increment: i64) -> std::io::Result<()> {
    fs::create_dir_all(root.join("plugin/api/src"))?;
    fs::create_dir_all(root.join("plugin/src"))?;
    fs::write(
        root.join("plugin/api/vela.toml"),
        "[package]\nid = \"dev.vela.example.api\"\nname = \"api\"\nversion = \"0.1.0\"\n[source]\nroots = [\"src\"]\n",
    )?;
    fs::write(
        root.join("plugin/api/src/api.vela"),
        "pub trait CommandProvider { fn run(self, value: i64) -> i64; }\n",
    )?;
    fs::write(
        root.join("plugin/vela.toml"),
        "[package]\nid = \"dev.vela.example.plugin\"\nname = \"plugin\"\nversion = \"0.1.0\"\n[source]\nroots = [\"src\"]\n[dependencies]\napi = { path = \"api\" }\n",
    )?;
    write_provider_source(root, increment)
}

fn write_provider_source(root: &Path, increment: i64) -> std::io::Result<()> {
    fs::write(
        root.join("plugin/src/plugin.vela"),
        format!(
            "use api::api::CommandProvider\npub struct Command {{}}\n#[provider(id = \"command\")]\nimpl CommandProvider for Command {{ pub fn run(self, value: i64) -> i64 {{ return value + {increment}; }} }}\n"
        ),
    )
}

fn call_provider(
    runtime: &mut Runtime,
    handle: &ProviderHandle,
    method: vela_def::MethodId,
) -> Result<i64, Box<dyn std::error::Error>> {
    let value = runtime.call(
        handle.method(method),
        CallArgs::new().with_value("value", 40_i64),
        CallOptions::unbounded(),
    )?;
    match runtime.value_to_owned(&value)? {
        OwnedValue::Scalar(vela_common::ScalarValue::I64(value)) => Ok(value),
        other => Err(format!("expected i64, got {other:?}").into()),
    }
}

fn fixture_root(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("vela_{name}_{}", std::process::id()))
}

#[test]
fn example_plugin_provider_discovers_compiles_runs_and_reloads() {
    assert_eq!(run_demo().expect("provider demo"), (41, 42));
}
