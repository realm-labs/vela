use std::error::Error;

#[path = "external_compare/config.rs"]
mod config;
#[path = "external_compare/embedded.rs"]
mod embedded;
#[path = "external_compare/process.rs"]
mod process;
#[path = "external_compare/report.rs"]
mod report;
#[path = "external_compare/support.rs"]
mod support;
#[path = "external_compare/vela.rs"]
mod vela;
#[path = "external_compare/version.rs"]
mod version;
#[path = "external_compare/workloads.rs"]
mod workloads;

use config::BenchConfig;
use workloads::WORKLOADS;

fn main() -> Result<(), Box<dyn Error>> {
    let config = BenchConfig::from_args();
    report::print_header(&config);

    let workloads = WORKLOADS
        .iter()
        .filter(|workload| config.should_run(workload.name))
        .collect::<Vec<_>>();
    if workloads.is_empty() {
        return Err(format!("no external workloads matched {}", config.filters_label()).into());
    }

    let vela_runtime = vela::VelaRuntime::new()?;
    for workload in &workloads {
        report::print_result(
            "vela",
            env!("CARGO_PKG_VERSION"),
            "internal_hot_loop",
            workload.name,
            vela_runtime.run(workload, config.params)?,
            config.params,
        );
    }

    let lua_runtime = embedded::LuaRuntime::new();
    for workload in &workloads {
        report::print_result(
            "lua54",
            "mlua-vendored-lua54",
            "embedded_hot_loop",
            workload.name,
            lua_runtime.run(workload, config.params)?,
            config.params,
        );
    }

    let rhai_runtime = embedded::RhaiRuntime::new();
    for workload in &workloads {
        report::print_result(
            "rhai",
            "rhai-crate",
            "embedded_hot_loop",
            workload.name,
            rhai_runtime.run(workload, config.params)?,
            config.params,
        );
    }

    for runtime in process::process_runtimes() {
        match runtime.locate() {
            Some(command) => {
                let version = runtime.version(&command);
                println!(
                    "runtime={} version=\"{}\" command={}",
                    runtime.name,
                    support::sanitize(&version),
                    command
                );
                for workload in &workloads {
                    match runtime.run(&command, workload, config.params) {
                        Ok(result) => report::print_result(
                            runtime.name,
                            &version,
                            "process_hot_loop",
                            workload.name,
                            result,
                            config.params,
                        ),
                        Err(error) => println!(
                            "runtime={} bench={} status=error command={} error=\"{}\"",
                            runtime.name,
                            workload.name,
                            command,
                            support::sanitize(&error.to_string())
                        ),
                    }
                }
            }
            None => println!(
                "runtime={} status=missing commands={}",
                runtime.name,
                runtime.commands.join(",")
            ),
        }
    }

    Ok(())
}
