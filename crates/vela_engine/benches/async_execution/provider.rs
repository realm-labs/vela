use std::error::Error;
use std::fs;
use std::path::PathBuf;

use vela_engine::engine::Engine;
use vela_engine::runtime::{CallArgs, CallOptions, ProviderMethodTarget, Runtime};
use vela_engine::source::ProviderCompileRequest;

use super::{poll_to_completion, report};

pub(super) struct ProviderBench {
    root: PathBuf,
    runtime: Runtime,
    sync: ProviderMethodTarget,
    async_target: ProviderMethodTarget,
}

impl ProviderBench {
    pub(super) fn new() -> Result<Self, Box<dyn Error>> {
        let root =
            std::env::temp_dir().join(format!("vela_async_bench_provider_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src"))?;
        fs::write(
            root.join("vela.toml"),
            "[package]\nid = \"dev.vela.bench\"\nname = \"bench\"\nversion = \"0.1.0\"\n[source]\nroots = [\"src\"]\n",
        )?;
        fs::write(
            root.join("src/api.vela"),
            r#"
pub trait BenchProvider {
    fn sync_run(self) -> i64;
    async fn async_run(self) -> i64;
}
pub struct Bench {}
#[provider(id = "bench")]
impl BenchProvider for Bench {
    pub fn sync_run(self) -> i64 { return 42; }
    pub async fn async_run(self) -> i64 { return 42; }
}
"#,
        )?;
        let engine = Engine::builder().build()?;
        let snapshot = engine.load_package_workspace(root.join("vela.toml"))?;
        let catalog = engine.discover_providers(&snapshot)?;
        let descriptor = catalog
            .providers()
            .first()
            .ok_or("benchmark provider was not discovered")?;
        let key = descriptor.key().clone();
        let sync_method = descriptor
            .methods()
            .iter()
            .find(|method| method.name() == "sync_run")
            .ok_or("sync provider method missing")?
            .id();
        let async_method = descriptor
            .methods()
            .iter()
            .find(|method| method.name() == "async_run")
            .ok_or("async provider method missing")?
            .id();
        let selection = catalog.select([key.clone()])?;
        let request = ProviderCompileRequest::for_selection(&snapshot, selection);
        let artifact = engine.compile_provider_selection(&snapshot, &request)?;
        let runtime = Runtime::from_linked_artifact(engine, artifact);
        let handle = runtime.provider_handle(&key)?;
        Ok(Self {
            root,
            runtime,
            sync: handle.method(sync_method),
            async_target: handle.method(async_method),
        })
    }

    pub(super) fn report(&mut self, iterations: usize) -> Result<(), Box<dyn Error>> {
        report("provider_sync", iterations, || {
            let output =
                self.runtime
                    .call(self.sync.clone(), CallArgs::new(), CallOptions::unbounded())?;
            result_i64(&mut self.runtime, &output)
        })?;
        report("provider_async", iterations, || {
            let output = poll_to_completion(self.runtime.call_async(
                self.async_target.clone(),
                CallArgs::new(),
                CallOptions::unbounded(),
            ))?;
            result_i64(&mut self.runtime, &output)
        })
    }
}

impl Drop for ProviderBench {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn result_i64(
    runtime: &mut Runtime,
    value: &vela_engine::runtime::VelaValue,
) -> Result<i64, Box<dyn Error>> {
    match runtime.value_to_owned(value)? {
        vela_vm::owned_value::OwnedValue::Scalar(vela_common::ScalarValue::I64(value)) => Ok(value),
        other => Err(format!("expected provider i64, got {other:?}").into()),
    }
}
