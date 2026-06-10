use std::error::Error;
use std::path::Path;
use std::sync::Arc;

use vela_engine::engine::Engine;
use vela_engine::reload::EngineHotReloadSourceErrorKind;
use vela_engine::runtime::{CallOptions, Runtime};
use vela_host::access::HostAccess;
use vela_host::mock::MockStateAdapter;
use vela_hot_reload::version::ProgramVersion;
use vela_vm::owned_value::OwnedValue;

pub fn run(
    initial_path: impl AsRef<Path>,
    updated_path: impl AsRef<Path>,
) -> Result<(), Box<dyn Error>> {
    let engine = crate::game_server::hot_reload_engine().map_err(|error| format!("{error:?}"))?;
    let initial_path = initial_path.as_ref();
    let updated_path = updated_path.as_ref();
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

    if let Err(error) = runtime
        .stage_hot_reload_update_file(updated_path)
        .map_err(|error| format!("{error:?}"))?
    {
        match error.kind {
            EngineHotReloadSourceErrorKind::Source(error) => {
                return Err(
                    crate::diagnostics::render_engine_source_error(updated_path, &error).into(),
                );
            }
            EngineHotReloadSourceErrorKind::HotReload(error) => {
                return Err(format!("{error:?}").into());
            }
        }
    }
    let report = runtime
        .check_reload_at_tick_boundary()?
        .ok_or("staged hot reload update was not consumed at the safe point")?;
    let report_lines = report.render_lines();
    let new = report
        .version()
        .ok_or_else(|| crate::diagnostics::render_hot_reload_report(updated_path, &report))?;
    let old_after = run_version_main(runtime.engine(), &old)?;
    let new_after = run_current_main(&mut runtime)?;

    for line in &report_lines {
        println!("{}", line.text);
    }
    println!(
        "safe_point=tick_boundary abi=checked old_version={} new_version={} old_before={old_before:?} \
         old_after={old_after:?} new_after={new_after:?}",
        old.id.0, new.id.0,
    );
    Ok(())
}

fn run_current_main(runtime: &mut Runtime) -> Result<OwnedValue, Box<dyn Error>> {
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    runtime
        .call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx)
        .map_err(|error| format!("{error:?}").into())
}

fn run_version_main(
    engine: &Engine,
    version: &Arc<ProgramVersion>,
) -> Result<OwnedValue, Box<dyn Error>> {
    engine
        .into_vm()
        .run_linked_program(
            version
                .linked_program()
                .ok_or("hot reload version must own linked bytecode")?,
            "main",
            &[],
        )
        .map_err(|error| format!("{error:?}").into())
}
