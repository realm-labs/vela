use std::error::Error;
use std::hint::black_box;
use std::time::{Duration, Instant};

use vela_bytecode::compiler::compile_function_source;
use vela_common::SourceId;
use vela_vm::Vm;
use vela_vm::budget::ExecutionBudget;
use vela_vm::value::Value;

#[path = "baseline/workloads.rs"]
mod workloads;

use workloads::{ExecutionMode, WORKLOADS, Workload};

const QUICK_REPEATS: usize = 2;
const QUICK_ITERATIONS: usize = 8;
const QUICK_WARMUP: usize = 2;
const DEFAULT_REPEATS: usize = 7;
const DEFAULT_ITERATIONS: usize = 100;
const DEFAULT_WARMUP: usize = 10;

fn main() -> Result<(), Box<dyn Error>> {
    let params = BenchParams::from_args();
    println!(
        "vela_vm_baseline profile={} target={}/{} repeats={} iterations={} warmup={}",
        profile(),
        std::env::consts::OS,
        std::env::consts::ARCH,
        params.repeats,
        params.iterations,
        params.warmup
    );

    for workload in WORKLOADS {
        let result = run_workload(workload, params)?;
        println!(
            "bench={} mode={} min_ns={} mean_ns={} median_ns={} p95_ns={} checksum={}",
            workload.name,
            workload.mode.as_str(),
            result.min_ns,
            result.mean_ns,
            result.median_ns,
            result.p95_ns,
            result.checksum
        );
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

fn run_workload(workload: &Workload, params: BenchParams) -> Result<BenchResult, Box<dyn Error>> {
    let code = compile_function_source(SourceId::new(1), workload.source, "main")
        .map_err(|error| format!("{error:?}"))?;
    let vm = Vm::new().with_standard_natives();

    for _ in 0..params.warmup {
        let value = run_once(workload.mode, &vm, &code)?;
        black_box(value);
    }

    let mut samples = Vec::with_capacity(params.repeats);
    let mut checksum = 0;
    for _ in 0..params.repeats {
        let started = Instant::now();
        for _ in 0..params.iterations {
            let value = run_once(workload.mode, &vm, &code)?;
            checksum = mix(checksum, value_checksum(&value));
            black_box(value);
        }
        samples.push(started.elapsed());
    }

    Ok(summarize(samples, checksum))
}

fn run_once(
    mode: ExecutionMode,
    vm: &Vm,
    code: &vela_bytecode::CodeObject,
) -> Result<Value, Box<dyn Error>> {
    match mode {
        ExecutionMode::Inline => Ok(vm.run(code)?),
        ExecutionMode::ManagedHeap => {
            let mut budget = ExecutionBudget::unbounded();
            Ok(vm.run_with_managed_heap_and_budget(code, &mut budget)?)
        }
    }
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
        Value::Missing => 0x01,
        Value::Null => 0x02,
        Value::Bool(value) => u64::from(*value) ^ 0x03,
        Value::Int(value) => *value as u64,
        Value::Float(value) => value.to_bits(),
        Value::String(value) => bytes_checksum(value.as_bytes()),
        Value::Array(values) | Value::Set(values) => values
            .iter()
            .fold(0x05, |checksum, value| mix(checksum, value_checksum(value))),
        Value::Map(values) => values.iter().fold(0x06, |checksum, (key, value)| {
            mix(
                mix(checksum, bytes_checksum(key.as_bytes())),
                value_checksum(value),
            )
        }),
        Value::Record { type_name, fields } => fields.values().fold(
            mix(0x07, bytes_checksum(type_name.as_bytes())),
            |checksum, value| mix(checksum, value_checksum(value)),
        ),
        Value::Enum {
            enum_name,
            variant,
            fields,
        } => fields.values().fold(
            mix(
                mix(0x08, bytes_checksum(enum_name.as_bytes())),
                bytes_checksum(variant.as_bytes()),
            ),
            |checksum, value| mix(checksum, value_checksum(value)),
        ),
        Value::Range(_) => 0x09,
        Value::Closure(_) | Value::HeapRef(_) | Value::HostRef(_) | Value::PathProxy(_) => 0x0a,
        Value::Iterator(_) => 0x0b,
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

fn profile() -> &'static str {
    if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    }
}

impl ExecutionMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Inline => "inline",
            Self::ManagedHeap => "managed_heap",
        }
    }
}
