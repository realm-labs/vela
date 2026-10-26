use std::error::Error;
use std::hint::black_box;
use std::process::Command;
use std::time::{Duration, Instant};

use super::bounded_process;
use super::config::BenchParams;
use super::preflight;
use super::support::{BenchResult, bytes_checksum, mix, sanitize, summarize};
use super::version::{node_version_text_is_supported, python_major_from_version_text};
use super::workloads::Workload;

pub(crate) struct ProcessRuntime {
    pub(crate) name: &'static str,
    pub(crate) commands: &'static [&'static str],
    version_args: &'static [&'static str],
    run_args: &'static [&'static str],
}

impl ProcessRuntime {
    pub(crate) fn locate(&self) -> Result<Option<String>, Box<dyn Error>> {
        for command in self.commands {
            match self.version(command) {
                Err(error)
                    if error
                        .downcast_ref::<std::io::Error>()
                        .is_some_and(|error| error.kind() == std::io::ErrorKind::NotFound) =>
                {
                    continue;
                }
                Err(error) => return Err(error),
                Ok(version)
                    if self.name == "python3"
                        && python_major_from_version_text(&version) == Some(2) =>
                {
                    continue;
                }
                Ok(version)
                    if self.name == "python3"
                        && python_major_from_version_text(&version) != Some(3) =>
                {
                    return Err(format!("{command}: invalid Python version: {version}").into());
                }
                Ok(_) => return Ok(Some((*command).to_owned())),
            }
        }
        Ok(None)
    }

    pub(crate) fn version(&self, command: &str) -> Result<String, Box<dyn Error>> {
        let output = bounded_process::output(
            Command::new(command).args(self.version_args),
            Duration::from_secs(5),
        )?;
        if !output.status.success() {
            return Err(format!("{command}: version probe exited with {}", output.status).into());
        }
        let stdout = String::from_utf8(output.stdout)?;
        let stderr = String::from_utf8(output.stderr)?;
        let version = if stdout.trim().is_empty() {
            stderr.trim()
        } else {
            stdout.trim()
        };
        if self.name == "node" && !node_version_text_is_supported(version) {
            return Err(format!("{command}: invalid Node version: {}", sanitize(version)).into());
        }
        if version.is_empty() {
            return Err(format!("{command}: empty version response").into());
        }
        Ok(sanitize(version))
    }

    pub(crate) fn preflight(
        &self,
        command: &str,
        workload: &Workload,
    ) -> Result<(), Box<dyn Error>> {
        for iterations in preflight::ITERATIONS {
            let actual = self.run_process(
                command,
                self.script_for(workload),
                iterations,
                Duration::from_secs(10),
            )?;
            preflight::validate(self.name, workload.name, iterations, i64::try_from(actual)?)?;
        }
        Ok(())
    }

    pub(crate) fn run(
        &self,
        command: &str,
        workload: &Workload,
        params: BenchParams,
    ) -> Result<BenchResult, Box<dyn Error>> {
        let script = self.script_for(workload);
        for _ in 0..params.warmup {
            let checksum =
                self.run_process(command, script, params.iterations, Duration::from_secs(120))?;
            black_box(checksum);
        }

        let mut samples = Vec::with_capacity(params.repeats);
        let mut checksum =
            bytes_checksum(self.name.as_bytes()) ^ bytes_checksum(workload.name.as_bytes());
        for _ in 0..params.repeats {
            let started = Instant::now();
            let iteration_checksum =
                self.run_process(command, script, params.iterations, Duration::from_secs(120))?;
            samples.push(started.elapsed());
            checksum = mix(checksum, iteration_checksum);
            black_box(iteration_checksum);
        }

        Ok(summarize(samples, checksum))
    }

    fn script_for<'a>(&self, workload: &'a Workload) -> &'a str {
        match self.name {
            "node" => workload.node,
            "python3" => workload.python,
            _ => unreachable!("only Node and Python process runtimes are registered"),
        }
    }

    fn run_process(
        &self,
        command: &str,
        script: &str,
        iterations: usize,
        timeout: Duration,
    ) -> Result<u64, Box<dyn Error>> {
        let mut command_process = Command::new(command);
        command_process
            .args(self.run_args)
            .env("VELA_BENCH_ITERATIONS", iterations.to_string())
            .arg(script);

        let output = bounded_process::output(&mut command_process, timeout)?;
        if !output.status.success() {
            return Err(format!(
                "process exited with {}; stderr={}",
                output.status,
                sanitize(&String::from_utf8_lossy(&output.stderr))
            )
            .into());
        }
        let stdout = String::from_utf8(output.stdout)?;
        let checksum = stdout
            .trim()
            .parse::<u64>()
            .map_err(|error| format!("invalid checksum `{}`: {error}", stdout.trim()))?;
        Ok(checksum)
    }
}

#[cfg(all(test, unix))]
#[test]
fn node_version_probe_rejects_successful_non_node_output() {
    let runtime = ProcessRuntime {
        name: "node",
        commands: &["sh"],
        version_args: &["-c", "printf not-a-node-version"],
        run_args: &["-e"],
    };
    let error = runtime
        .locate()
        .expect_err("invalid available Node must not be skipped");
    assert!(
        error.to_string().contains("invalid Node version"),
        "{error}"
    );
}

pub(crate) fn process_runtimes() -> Vec<ProcessRuntime> {
    vec![
        ProcessRuntime {
            name: "node",
            commands: &["node"],
            version_args: &["--version"],
            run_args: &["-e"],
        },
        ProcessRuntime {
            name: "python3",
            commands: &["python3", "python"],
            version_args: &["--version"],
            run_args: &["-c"],
        },
    ]
}
