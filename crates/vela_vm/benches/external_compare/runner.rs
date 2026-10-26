use std::error::Error;

use super::{config::BenchConfig, embedded, process, report, support, vela, workloads};

pub(crate) fn run(args: impl IntoIterator<Item = String>) -> Result<(), Box<dyn Error>> {
    let config = BenchConfig::from_iter(args)?;
    report::print_header(&config);

    let workloads = workloads::all_workloads()
        .filter(|workload| config.should_run(workload.name))
        .collect::<Vec<_>>();
    if workloads.is_empty() {
        return Err(format!("no external workloads matched {}", config.filters_label()).into());
    }

    if config.should_run_runtime("vela") {
        let vela_runtime = vela::VelaRuntime::new()?;
        for workload in &workloads {
            vela_runtime.preflight(workload, false)?;
            if config.preflight_only {
                continue;
            }
            report::print_result(
                "vela",
                env!("CARGO_PKG_VERSION"),
                "internal_hot_loop",
                workload.name,
                vela_runtime.run(workload, config.params)?,
                config.params,
            );
        }
    }

    if config.should_run_runtime("vela-cache") {
        let vela_runtime = vela::VelaRuntime::new()?;
        for workload in &workloads {
            vela_runtime.preflight(workload, true)?;
            if config.preflight_only {
                continue;
            }
            report::print_result(
                "vela-cache",
                env!("CARGO_PKG_VERSION"),
                "internal_hot_loop",
                workload.name,
                vela_runtime.run_cache_enabled(workload, config.params)?,
                config.params,
            );
        }
    }

    if config.should_run_runtime("lua54") {
        let lua_runtime = embedded::LuaRuntime::new();
        for workload in &workloads {
            lua_runtime.preflight(workload)?;
            if config.preflight_only {
                continue;
            }
            report::print_result(
                "lua54",
                "mlua-vendored-lua54",
                "embedded_hot_loop",
                workload.name,
                lua_runtime.run(workload, config.params)?,
                config.params,
            );
        }
    }

    if config.should_run_runtime("rhai") {
        let rhai_runtime = embedded::RhaiRuntime::new();
        for workload in &workloads {
            rhai_runtime.preflight(workload)?;
            if config.preflight_only {
                continue;
            }
            report::print_result(
                "rhai",
                "rhai-crate",
                "embedded_hot_loop",
                workload.name,
                rhai_runtime.run(workload, config.params)?,
                config.params,
            );
        }
    }

    for runtime in process::process_runtimes() {
        if !config.should_run_runtime(runtime.name) {
            continue;
        }
        match runtime.locate()? {
            Some(command) => {
                let version = runtime.version(&command)?;
                println!(
                    "runtime={} version=\"{}\" command={}",
                    runtime.name,
                    support::sanitize(&version),
                    command
                );
                for workload in &workloads {
                    runtime.preflight(&command, workload)?;
                    if config.preflight_only {
                        continue;
                    }
                    match runtime.run(&command, workload, config.params) {
                        Ok(result) => report::print_result(
                            runtime.name,
                            &version,
                            "process_hot_loop",
                            workload.name,
                            result,
                            config.params,
                        ),
                        Err(error) => {
                            println!(
                                "runtime={} bench={} status=error command={} error=\"{}\"",
                                runtime.name,
                                workload.name,
                                command,
                                support::sanitize(&error.to_string())
                            );
                            return Err(error);
                        }
                    }
                }
            }
            None if config.runtime_required(runtime.name) => {
                return Err(format!("selected runtime {} is unavailable", runtime.name).into());
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

#[cfg(test)]
mod tests {
    #[test]
    fn rejects_unknown_only_mixed_and_empty_runtime_selections_before_workloads() {
        use super::run;
        for preflight_only in [false, true] {
            for (selector, message) in [
                ("not-a-runtime", "unknown runtime selector `not-a-runtime`"),
                (
                    "vela,not-a-runtime",
                    "unknown runtime selector `not-a-runtime`",
                ),
                ("", "nonempty runtime selection"),
            ] {
                for equals_form in [false, true] {
                    let mut args = if equals_form {
                        vec![format!("--runtime={selector}")]
                    } else {
                        vec!["--runtime".to_owned(), selector.to_owned()]
                    };
                    if preflight_only {
                        args.push("--preflight-only".to_owned());
                    }
                    // Invalid selectors must fail before even workload discovery,
                    // so a valid prefix cannot trigger partial execution.
                    args.push("no-workload-matches-this-regression".to_owned());
                    let error = run(args).expect_err("invalid explicit selection must fail");
                    assert!(error.to_string().contains(message), "{error}");
                }
            }
        }
    }
}
