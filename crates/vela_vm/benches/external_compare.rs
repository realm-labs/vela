use std::error::Error;
use std::fs;
use std::hint::black_box;
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use vela_bytecode::compiler::compile_function_source;
use vela_common::SourceId;
use vela_vm::Vm;
use vela_vm::value::Value;

const QUICK_REPEATS: usize = 2;
const QUICK_ITERATIONS: usize = 8;
const QUICK_WARMUP: usize = 1;
const DEFAULT_REPEATS: usize = 5;
const DEFAULT_ITERATIONS: usize = 100;
const DEFAULT_WARMUP: usize = 3;

const VELA_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for value in 0..200 {
        if value % 3 == 0 {
            total += value * 2;
            continue;
        }
        if value > 180 {
            break;
        }
        total += (value * 5) % 17;
    }
    return total;
}
"#;

const NODE_SCALAR_SCRIPT: &str = r#"
const iterations = Number(process.env.VELA_BENCH_ITERATIONS || "1");
let checksum = 0;
function run() {
    let total = 0;
    for (let value = 0; value < 200; value += 1) {
        if (value % 3 === 0) {
            total += value * 2;
            continue;
        }
        if (value > 180) {
            break;
        }
        total += (value * 5) % 17;
    }
    return total;
}
for (let iteration = 0; iteration < iterations; iteration += 1) {
    checksum += run();
}
console.log(String(checksum));
"#;

const LUA_SCALAR_SCRIPT: &str = r#"
local iterations = tonumber(os.getenv("VELA_BENCH_ITERATIONS") or "1")
local checksum = 0
local function run()
    local total = 0
    for value = 0, 199 do
        if value % 3 == 0 then
            total = total + value * 2
        elseif value > 180 then
            break
        else
            total = total + (value * 5) % 17
        end
    end
    return total
end
for _ = 1, iterations do
    checksum = checksum + run()
end
print(string.format("%.0f", checksum))
"#;

const RHAI_SCALAR_SCRIPT: &str = r#"
let iterations = {iterations};
let checksum = 0;

fn run() {
    let total = 0;
    for value in 0..200 {
        if value % 3 == 0 {
            total += value * 2;
        } else if value > 180 {
            break;
        } else {
            total += (value * 5) % 17;
        }
    }
    total
}

for _ in 0..iterations {
    checksum += run();
}

print(checksum);
"#;

fn main() -> Result<(), Box<dyn Error>> {
    let params = BenchParams::from_args();
    println!(
        "vela_vm_external_compare profile={} target={}/{} repeats={} iterations={} warmup={}",
        profile(),
        std::env::consts::OS,
        std::env::consts::ARCH,
        params.repeats,
        params.iterations,
        params.warmup
    );

    let vela = run_vela(params)?;
    print_result(
        "vela",
        env!("CARGO_PKG_VERSION"),
        "internal",
        "scalar_branch_loop",
        vela,
    );

    for runtime in external_runtimes() {
        match runtime.locate() {
            Some(command) => {
                let version = runtime.version(&command);
                match runtime.run(&command, params) {
                    Ok(result) => print_result(
                        runtime.name,
                        &version,
                        "process",
                        "scalar_branch_loop",
                        result,
                    ),
                    Err(error) => println!(
                        "runtime={} status=error command={} error=\"{}\"",
                        runtime.name,
                        command,
                        sanitize(&error.to_string())
                    ),
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

#[derive(Clone, Copy)]
struct BenchParams {
    repeats: usize,
    iterations: usize,
    warmup: usize,
}

impl BenchParams {
    fn from_args() -> Self {
        if std::env::args().skip(1).any(|arg| arg == "--quick") {
            return Self {
                repeats: QUICK_REPEATS,
                iterations: QUICK_ITERATIONS,
                warmup: QUICK_WARMUP,
            };
        }
        Self {
            repeats: DEFAULT_REPEATS,
            iterations: DEFAULT_ITERATIONS,
            warmup: DEFAULT_WARMUP,
        }
    }
}

struct BenchResult {
    min_ns: u128,
    mean_ns: u128,
    median_ns: u128,
    p95_ns: u128,
    checksum: u64,
}

struct ExternalRuntime {
    name: &'static str,
    commands: &'static [&'static str],
    version_args: &'static [&'static str],
    run_args: &'static [&'static str],
    script: &'static str,
    script_mode: ScriptMode,
}

#[derive(Clone, Copy)]
enum ScriptMode {
    InlineArg,
    FileArg,
}

impl ExternalRuntime {
    fn locate(&self) -> Option<String> {
        self.commands
            .iter()
            .find(|command| command_available(command))
            .map(|command| (*command).to_owned())
    }

    fn version(&self, command: &str) -> String {
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

    fn run(&self, command: &str, params: BenchParams) -> Result<BenchResult, Box<dyn Error>> {
        for _ in 0..params.warmup {
            let checksum = self.run_process(command, params.iterations)?;
            black_box(checksum);
        }

        let mut samples = Vec::with_capacity(params.repeats);
        let mut checksum = bytes_checksum(self.name.as_bytes());
        for _ in 0..params.repeats {
            let started = Instant::now();
            let iteration_checksum = self.run_process(command, params.iterations)?;
            samples.push(started.elapsed());
            checksum = mix(checksum, iteration_checksum);
            black_box(iteration_checksum);
        }

        Ok(summarize(samples, checksum))
    }

    fn run_process(&self, command: &str, iterations: usize) -> Result<u64, Box<dyn Error>> {
        let script = self.script.replace("{iterations}", &iterations.to_string());
        let mut command_process = Command::new(command);
        command_process
            .args(self.run_args)
            .env("VELA_BENCH_ITERATIONS", iterations.to_string());

        let temp_script = match self.script_mode {
            ScriptMode::InlineArg => {
                command_process.arg(script);
                None
            }
            ScriptMode::FileArg => {
                let path = temp_script_path(self.name);
                fs::write(&path, script)?;
                command_process.arg(&path);
                Some(path)
            }
        };

        let output = command_process.output()?;
        if let Some(path) = temp_script {
            let _ = fs::remove_file(path);
        }
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

fn external_runtimes() -> Vec<ExternalRuntime> {
    vec![
        ExternalRuntime {
            name: "lua5",
            commands: &["lua", "lua5.4", "lua5.3"],
            version_args: &["-v"],
            run_args: &["-e"],
            script: LUA_SCALAR_SCRIPT,
            script_mode: ScriptMode::InlineArg,
        },
        ExternalRuntime {
            name: "luajit",
            commands: &["luajit"],
            version_args: &["-v"],
            run_args: &["-e"],
            script: LUA_SCALAR_SCRIPT,
            script_mode: ScriptMode::InlineArg,
        },
        ExternalRuntime {
            name: "node",
            commands: &["node"],
            version_args: &["--version"],
            run_args: &["-e"],
            script: NODE_SCALAR_SCRIPT,
            script_mode: ScriptMode::InlineArg,
        },
        ExternalRuntime {
            name: "rhai",
            commands: &["rhai-run"],
            version_args: &["--version"],
            run_args: &[],
            script: RHAI_SCALAR_SCRIPT,
            script_mode: ScriptMode::FileArg,
        },
    ]
}

fn run_vela(params: BenchParams) -> Result<BenchResult, Box<dyn Error>> {
    let code = compile_function_source(SourceId::new(1), VELA_SOURCE, "main")
        .map_err(|error| format!("{error:?}"))?;
    let vm = Vm::new();

    for _ in 0..params.warmup {
        let checksum = run_vela_iterations(&vm, &code, params.iterations)?;
        black_box(checksum);
    }

    let mut samples = Vec::with_capacity(params.repeats);
    let mut checksum = bytes_checksum(b"vela");
    for _ in 0..params.repeats {
        let started = Instant::now();
        let iteration_checksum = run_vela_iterations(&vm, &code, params.iterations)?;
        samples.push(started.elapsed());
        checksum = mix(checksum, iteration_checksum);
        black_box(iteration_checksum);
    }

    Ok(summarize(samples, checksum))
}

fn run_vela_iterations(
    vm: &Vm,
    code: &vela_bytecode::CodeObject,
    iterations: usize,
) -> Result<u64, Box<dyn Error>> {
    let mut checksum = 0_u64;
    for _ in 0..iterations {
        let value = vm.run(code)?;
        checksum = checksum.wrapping_add(value_checksum(&value));
    }
    Ok(checksum)
}

fn command_available(command: &str) -> bool {
    Command::new(command)
        .arg("--version")
        .output()
        .or_else(|_| Command::new(command).arg("-v").output())
        .is_ok()
}

fn temp_script_path(runtime: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    std::env::temp_dir().join(format!(
        "vela_external_compare_{}_{}_{}.rhai",
        runtime,
        std::process::id(),
        unique
    ))
}

fn print_result(runtime: &str, version: &str, mode: &str, bench: &str, result: BenchResult) {
    println!(
        "runtime={} version=\"{}\" bench={} mode={} min_ns={} mean_ns={} median_ns={} p95_ns={} checksum={}",
        runtime,
        sanitize(version),
        bench,
        mode,
        result.min_ns,
        result.mean_ns,
        result.median_ns,
        result.p95_ns,
        result.checksum
    );
}

fn summarize(mut samples: Vec<Duration>, checksum: u64) -> BenchResult {
    samples.sort_unstable();
    let min_ns = samples.first().map_or(0, Duration::as_nanos);
    let median_ns = percentile_ns(&samples, 50);
    let p95_ns = percentile_ns(&samples, 95);
    let mean_ns = if samples.is_empty() {
        0
    } else {
        samples.iter().map(Duration::as_nanos).sum::<u128>() / samples.len() as u128
    };
    BenchResult {
        min_ns,
        mean_ns,
        median_ns,
        p95_ns,
        checksum,
    }
}

fn percentile_ns(samples: &[Duration], percentile: usize) -> u128 {
    if samples.is_empty() {
        return 0;
    }
    let index = ((samples.len() - 1) * percentile).div_ceil(100);
    samples[index].as_nanos()
}

fn value_checksum(value: &Value) -> u64 {
    match value {
        Value::Int(value) => *value as u64,
        Value::Bool(value) => u64::from(*value),
        Value::Float(value) => value.to_bits(),
        Value::String(value) => bytes_checksum(value.as_bytes()),
        _ => 0,
    }
}

fn bytes_checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |checksum, byte| {
        (checksum ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

fn mix(lhs: u64, rhs: u64) -> u64 {
    lhs.rotate_left(5) ^ rhs.wrapping_mul(0x9e37_79b9_7f4a_7c15)
}

fn sanitize(value: &str) -> String {
    value
        .replace(['\r', '\n', '"'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn profile() -> &'static str {
    if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    }
}
