use std::error::Error;
use std::fs;
use std::hint::black_box;
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use vela_bytecode::compiler::compile_program_source_with_registry;
use vela_bytecode::{LinkedProgram, Linker, UnlinkedProgram};
use vela_common::SourceId;
use vela_vm::Vm;
use vela_vm::owned_value::OwnedValue;

const QUICK_REPEATS: usize = 2;
const QUICK_ITERATIONS: usize = 500;
const QUICK_WARMUP: usize = 1;
const DEFAULT_REPEATS: usize = 3;
const DEFAULT_ITERATIONS: usize = 5_000;
const DEFAULT_WARMUP: usize = 1;

struct Workload {
    name: &'static str,
    vela: &'static str,
    lua: &'static str,
    node: &'static str,
    rhai: &'static str,
}

const WORKLOADS: &[Workload] = &[
    Workload {
        name: "scalar_branch_loop",
        vela: r#"
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
"#,
        lua: r#"
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
"#,
        node: r#"
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
"#,
        rhai: r#"
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
"#,
    },
    Workload {
        name: "function_calls",
        vela: r#"
fn add_one(value) {
    return value + 1;
}

fn mix_pair(left, right) {
    return left * 3 + right;
}

fn main() {
    let total = 0;
    for tick in 0..240 {
        total += add_one(tick);
        total += mix_pair(tick, total % 17);
    }
    return total;
}
"#,
        lua: r#"
local iterations = tonumber(os.getenv("VELA_BENCH_ITERATIONS") or "1")
local checksum = 0
local function add_one(value)
    return value + 1
end
local function mix_pair(left, right)
    return left * 3 + right
end
local function run()
    local total = 0
    for tick = 0, 239 do
        total = total + add_one(tick)
        total = total + mix_pair(tick, total % 17)
    end
    return total
end
for _ = 1, iterations do
    checksum = checksum + run()
end
print(string.format("%.0f", checksum))
"#,
        node: r#"
const iterations = Number(process.env.VELA_BENCH_ITERATIONS || "1");
let checksum = 0;
function addOne(value) {
    return value + 1;
}
function mixPair(left, right) {
    return left * 3 + right;
}
function run() {
    let total = 0;
    for (let tick = 0; tick < 240; tick += 1) {
        total += addOne(tick);
        total += mixPair(tick, total % 17);
    }
    return total;
}
for (let iteration = 0; iteration < iterations; iteration += 1) {
    checksum += run();
}
console.log(String(checksum));
"#,
        rhai: r#"
let iterations = {iterations};
let checksum = 0;

fn add_one(value) {
    value + 1
}

fn mix_pair(left, right) {
    left * 3 + right
}

fn run() {
    let total = 0;
    for tick in 0..240 {
        total += add_one(tick);
        total += mix_pair(tick, total % 17);
    }
    total
}

for _ in 0..iterations {
    checksum += run();
}

print(checksum);
"#,
    },
    Workload {
        name: "array_scan",
        vela: r#"
fn main() {
    let values = [3, 1, 4, 1, 5, 9, 2, 6];
    let total = 0;
    for tick in 0..200 {
        for value in values {
            if value % 2 == 0 {
                total += (value * tick) % 17;
            } else {
                total += value + tick % 5;
            }
        }
    }
    return total;
}
"#,
        lua: r#"
local iterations = tonumber(os.getenv("VELA_BENCH_ITERATIONS") or "1")
local checksum = 0
local values = {3, 1, 4, 1, 5, 9, 2, 6}
local function run()
    local total = 0
    for tick = 0, 199 do
        for _, value in ipairs(values) do
            if value % 2 == 0 then
                total = total + (value * tick) % 17
            else
                total = total + value + tick % 5
            end
        end
    end
    return total
end
for _ = 1, iterations do
    checksum = checksum + run()
end
print(string.format("%.0f", checksum))
"#,
        node: r#"
const iterations = Number(process.env.VELA_BENCH_ITERATIONS || "1");
let checksum = 0;
const values = [3, 1, 4, 1, 5, 9, 2, 6];
function run() {
    let total = 0;
    for (let tick = 0; tick < 200; tick += 1) {
        for (const value of values) {
            if (value % 2 === 0) {
                total += (value * tick) % 17;
            } else {
                total += value + tick % 5;
            }
        }
    }
    return total;
}
for (let iteration = 0; iteration < iterations; iteration += 1) {
    checksum += run();
}
console.log(String(checksum));
"#,
        rhai: r#"
let iterations = {iterations};
let checksum = 0;
let values = [3, 1, 4, 1, 5, 9, 2, 6];

fn run(values) {
    let total = 0;
    for tick in 0..200 {
        for value in values {
            if value % 2 == 0 {
                total += (value * tick) % 17;
            } else {
                total += value + tick % 5;
            }
        }
    }
    total
}

for _ in 0..iterations {
    checksum += run(values);
}

print(checksum);
"#,
    },
    Workload {
        name: "string_methods",
        vela: r#"
fn main() {
    let total = 0;
    let labels = ["quest", "raid", "daily", "bonus"];
    for tick in 0..50 {
        for label in labels {
            if label.starts_with("q") || label.contains("i") {
                total += label.len() + tick % 7;
            } else {
                total += label.len();
            }
        }
    }
    return total;
}
"#,
        lua: r#"
local iterations = tonumber(os.getenv("VELA_BENCH_ITERATIONS") or "1")
local checksum = 0
local function run()
    local total = 0
    local labels = {"quest", "raid", "daily", "bonus"}
    for tick = 0, 49 do
        for _, label in ipairs(labels) do
            if string.sub(label, 1, 1) == "q" or string.find(label, "i", 1, true) ~= nil then
                total = total + #label + tick % 7
            else
                total = total + #label
            end
        end
    end
    return total
end
for _ = 1, iterations do
    checksum = checksum + run()
end
print(string.format("%.0f", checksum))
"#,
        node: r#"
const iterations = Number(process.env.VELA_BENCH_ITERATIONS || "1");
let checksum = 0;
function run() {
    let total = 0;
    const labels = ["quest", "raid", "daily", "bonus"];
    for (let tick = 0; tick < 50; tick += 1) {
        for (const label of labels) {
            if (label.startsWith("q") || label.includes("i")) {
                total += label.length + tick % 7;
            } else {
                total += label.length;
            }
        }
    }
    return total;
}
for (let iteration = 0; iteration < iterations; iteration += 1) {
    checksum += run();
}
console.log(String(checksum));
"#,
        rhai: r#"
let iterations = {iterations};
let checksum = 0;

fn run() {
    let total = 0;
    let labels = ["quest", "raid", "daily", "bonus"];
    for tick in 0..50 {
        for label in labels {
            if label.starts_with("q") || label.contains("i") {
                total += label.len() + tick % 7;
            } else {
                total += label.len();
            }
        }
    }
    total
}

for _ in 0..iterations {
    checksum += run();
}

print(checksum);
"#,
    },
];

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

    let vm = Vm::new().with_standard_natives();
    for workload in WORKLOADS {
        let result = run_vela(&vm, workload, params)?;
        print_result(
            "vela",
            env!("CARGO_PKG_VERSION"),
            "internal",
            workload.name,
            result,
            params,
        );
    }

    for runtime in external_runtimes() {
        match runtime.locate() {
            Some(command) => {
                let version = runtime.version(&command);
                println!(
                    "runtime={} version=\"{}\" command={}",
                    runtime.name,
                    sanitize(&version),
                    command
                );
                for workload in WORKLOADS {
                    match runtime.run(&command, workload, params) {
                        Ok(result) => print_result(
                            runtime.name,
                            &version,
                            "process",
                            workload.name,
                            result,
                            params,
                        ),
                        Err(error) => println!(
                            "runtime={} bench={} status=error command={} error=\"{}\"",
                            runtime.name,
                            workload.name,
                            command,
                            sanitize(&error.to_string())
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

    fn run(
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

    fn script_for<'a>(&self, workload: &'a Workload) -> &'a str {
        match self.name {
            "lua5" | "luajit" => workload.lua,
            "node" => workload.node,
            "rhai" => workload.rhai,
            _ => "",
        }
    }

    fn run_process(
        &self,
        command: &str,
        script: &str,
        iterations: usize,
    ) -> Result<u64, Box<dyn Error>> {
        let script = script.replace("{iterations}", &iterations.to_string());
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
            script_mode: ScriptMode::InlineArg,
        },
        ExternalRuntime {
            name: "luajit",
            commands: &["luajit"],
            version_args: &["-v"],
            run_args: &["-e"],
            script_mode: ScriptMode::InlineArg,
        },
        ExternalRuntime {
            name: "node",
            commands: &["node"],
            version_args: &["--version"],
            run_args: &["-e"],
            script_mode: ScriptMode::InlineArg,
        },
        ExternalRuntime {
            name: "rhai",
            commands: &["rhai-run"],
            version_args: &["--version"],
            run_args: &[],
            script_mode: ScriptMode::FileArg,
        },
    ]
}

fn run_vela(
    vm: &Vm,
    workload: &Workload,
    params: BenchParams,
) -> Result<BenchResult, Box<dyn Error>> {
    let registry = vela_stdlib::standard_registry()
        .map_err(|error| format!("standard registry failed: {error}"))?;
    let program = compile_program_source_with_registry(
        SourceId::new(1),
        workload.vela,
        registry.compile_view(),
    )
    .map_err(|error| format!("{error:?}"))?;
    let program = link_program_for_vm(vm, &program)?;

    for _ in 0..params.warmup {
        let checksum = run_vela_iterations(vm, &program, params.iterations)?;
        black_box(checksum);
    }

    let mut samples = Vec::with_capacity(params.repeats);
    let mut checksum = bytes_checksum(b"vela") ^ bytes_checksum(workload.name.as_bytes());
    for _ in 0..params.repeats {
        let started = Instant::now();
        let iteration_checksum = run_vela_iterations(vm, &program, params.iterations)?;
        samples.push(started.elapsed());
        checksum = mix(checksum, iteration_checksum);
        black_box(iteration_checksum);
    }

    Ok(summarize(samples, checksum))
}

fn run_vela_iterations(
    vm: &Vm,
    program: &LinkedProgram,
    iterations: usize,
) -> Result<u64, Box<dyn Error>> {
    let mut checksum = 0_u64;
    for _ in 0..iterations {
        let value = vm.run_linked_program(program, "main", &[])?;
        checksum = checksum.wrapping_add(value_checksum(&value));
    }
    Ok(checksum)
}

fn link_program_for_vm(
    vm: &Vm,
    program: &UnlinkedProgram,
) -> Result<LinkedProgram, Box<dyn Error>> {
    let mut linker = Linker::new();
    for id in vm.native_implementation_ids() {
        linker.add_native_implementation(id);
    }
    linker
        .link_program(program)
        .map_err(|error| format!("{error:?}").into())
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

fn print_result(
    runtime: &str,
    version: &str,
    mode: &str,
    bench: &str,
    result: BenchResult,
    params: BenchParams,
) {
    println!(
        "runtime={} version=\"{}\" bench={} mode={} min_ns={} mean_ns={} median_ns={} p95_ns={} per_iter_mean_ns={} checksum={}",
        runtime,
        sanitize(version),
        bench,
        mode,
        result.min_ns,
        result.mean_ns,
        result.median_ns,
        result.p95_ns,
        result.mean_ns / params.iterations as u128,
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

fn value_checksum(value: &OwnedValue) -> u64 {
    match value {
        OwnedValue::Scalar(vela_common::ScalarValue::I64(value)) => *value as u64,
        OwnedValue::Bool(value) => u64::from(*value),
        OwnedValue::Scalar(vela_common::ScalarValue::F64(value)) => value.to_bits(),
        OwnedValue::String(value) => bytes_checksum(value.as_bytes()),
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
