use std::error::Error;
use std::hint::black_box;
use std::process::Command;
use std::time::Instant;

use super::config::BenchParams;
use super::support::{BenchResult, bytes_checksum, mix, sanitize, summarize};
use super::version::python_major_from_version_text;
use super::workloads::Workload;

pub(crate) struct ProcessRuntime {
    pub(crate) name: &'static str,
    pub(crate) commands: &'static [&'static str],
    version_args: &'static [&'static str],
    run_args: &'static [&'static str],
}

impl ProcessRuntime {
    pub(crate) fn locate(&self) -> Option<String> {
        self.commands
            .iter()
            .find(|command| self.command_usable(command))
            .map(|command| (*command).to_owned())
    }

    pub(crate) fn version(&self, command: &str) -> String {
        Command::new(command)
            .args(self.version_args)
            .output()
            .ok()
            .map(|output| {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                sanitize(if stdout.trim().is_empty() {
                    stderr.trim()
                } else {
                    stdout.trim()
                })
            })
            .filter(|version| !version.is_empty())
            .unwrap_or_else(|| "unknown".to_owned())
    }

    pub(crate) fn run(
        &self,
        command: &str,
        workload: &Workload,
        params: BenchParams,
    ) -> Result<BenchResult, Box<dyn Error>> {
        let script = self.script_for(workload);
        for _ in 0..params.warmup {
            let checksum = self.run_process(command, script, params.iterations)?;
            black_box(checksum);
        }

        let mut samples = Vec::with_capacity(params.repeats);
        let mut checksum =
            bytes_checksum(self.name.as_bytes()) ^ bytes_checksum(workload.name.as_bytes());
        for _ in 0..params.repeats {
            let started = Instant::now();
            let iteration_checksum = self.run_process(command, script, params.iterations)?;
            samples.push(started.elapsed());
            checksum = mix(checksum, iteration_checksum);
            black_box(iteration_checksum);
        }

        Ok(summarize(samples, checksum))
    }

    fn command_usable(&self, command: &str) -> bool {
        if self.name == "python3" {
            return python_major_version(command) == Some(3);
        }
        Command::new(command)
            .arg("--version")
            .output()
            .or_else(|_| Command::new(command).arg("-v").output())
            .is_ok()
    }

    fn script_for<'a>(&self, workload: &'a Workload) -> &'a str {
        match self.name {
            "node" => workload.node,
            "python3" => workload.python,
            _ => "",
        }
    }

    fn run_process(
        &self,
        command: &str,
        script: &str,
        iterations: usize,
    ) -> Result<u64, Box<dyn Error>> {
        let mut command_process = Command::new(command);
        command_process
            .args(self.run_args)
            .env("VELA_BENCH_ITERATIONS", iterations.to_string())
            .arg(script);

        let output = command_process.output()?;
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

fn python_major_version(command: &str) -> Option<u32> {
    let output = Command::new(command).arg("--version").output().ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    python_major_from_version_text(if stdout.trim().is_empty() {
        stderr.trim()
    } else {
        stdout.trim()
    })
}
