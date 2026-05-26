use std::error::Error;
use std::path::Path;
use std::sync::Arc;

use vela_engine::{CallOptions, Engine, EngineHotReloadSourceErrorKind, Runtime};
use vela_host::{MockStateAdapter, PatchTx};
use vela_hot_reload::ProgramVersion;
use vela_vm::Value;

pub(crate) fn run(initial_path: &str, updated_path: &str) -> Result<(), Box<dyn Error>> {
    let engine = crate::demo::hot_reload_engine().map_err(|error| format!("{error:?}"))?;
    let initial_path = Path::new(initial_path);
    let updated_path = Path::new(updated_path);
    let initial = engine
        .compile_hot_reload_initial_file(initial_path)
        .map_err(|error| {
            crate::diagnostics::render_hot_reload_source_error(initial_path, &error)
        })?;
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let old = runtime
        .hot_reload_version()
        .ok_or("runtime must keep the initial hot reload version")?;
    let old_before = run_current_main(&mut runtime)?;

    let update = match runtime
        .engine()
        .compile_hot_reload_update_file(&old, updated_path)
    {
        Ok(update) => Ok(update),
        Err(error) => match error.kind {
            EngineHotReloadSourceErrorKind::Source(error) => {
                return Err(
                    crate::diagnostics::render_engine_source_error(updated_path, &error).into(),
                );
            }
            EngineHotReloadSourceErrorKind::HotReload(error) => Err(error),
        },
    };
    let report = runtime.apply_hot_update_result_report(update)?;
    let report_lines = report.render_lines();
    let new = report.version().ok_or_else(|| {
        format!(
            "hot reload rejected:\n{}",
            report_lines
                .iter()
                .map(|line| line.text.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        )
    })?;
    let old_after = run_version_main(runtime.engine(), &old)?;
    let new_after = run_current_main(&mut runtime)?;

    for line in &report_lines {
        println!("{}", line.text);
    }
    println!(
        "abi=checked old_version={} new_version={} old_before={old_before:?} \
         old_after={old_after:?} new_after={new_after:?}",
        old.id.0, new.id.0,
    );
    Ok(())
}

fn run_current_main(runtime: &mut Runtime) -> Result<Value, Box<dyn Error>> {
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    runtime
        .call("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx)
        .map_err(|error| format!("{error:?}").into())
}

fn run_version_main(
    engine: &Engine,
    version: &Arc<ProgramVersion>,
) -> Result<Value, Box<dyn Error>> {
    engine
        .into_vm()
        .run_program(&version.to_program(), "main", &[])
        .map_err(|error| format!("{error:?}").into())
}
