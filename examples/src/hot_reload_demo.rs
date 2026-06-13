use std::error::Error;
use std::sync::Arc;

use vela_common::SourceId;
use vela_engine::engine::Engine;
use vela_engine::runtime::{CallOptions, Runtime};
use vela_host::access::HostAccess;
use vela_host::mock::MockStateAdapter;
use vela_hot_reload::version::ProgramVersion;
use vela_vm::owned_value::OwnedValue;

pub fn run(
    initial_label: &str,
    initial_source: &str,
    updated_label: &str,
    updated_source: &str,
) -> Result<(), Box<dyn Error>> {
    let engine = crate::gameplay::build_engine(crate::gameplay::GameEngineOptions::default())
        .map_err(|error| format!("{error:?}"))?;
    let initial = engine
        .compile_hot_reload_initial(SourceId::new(1), initial_source)
        .map_err(|error| {
            crate::diagnostics::render_hot_reload_error(initial_label, initial_source, &error)
        })?;
    let mut runtime = Runtime::from_hot_reload_version(engine, initial);
    let old = runtime
        .hot_reload_version()
        .ok_or("runtime must keep the initial hot reload version")?;
    let old_before = run_current_main(&mut runtime)?;

    let update = runtime
        .compile_hot_reload_update(SourceId::new(1), updated_source)
        .map_err(|error| format!("{error:?}"))?;
    runtime.stage_hot_update_result(update)?;
    let report = runtime
        .check_reload_at_tick_boundary()?
        .ok_or("staged hot reload update was not consumed at the safe point")?;
    let report_lines = report.render_lines();
    let new = report.version().ok_or_else(|| {
        crate::diagnostics::render_hot_reload_report(updated_label, updated_source, &report)
    })?;
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
        .run_linked_program(version.linked_program(), "main", &[])
        .map_err(|error| format!("{error:?}").into())
}
