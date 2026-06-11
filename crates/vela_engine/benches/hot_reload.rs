use std::error::Error;
use std::hint::black_box;
use std::time::{Duration, Instant};

use vela_common::SourceId;
use vela_engine::engine::Engine;
use vela_engine::runtime::{CallOptions, Runtime};
use vela_host::access::HostAccess;
use vela_host::mock::MockStateAdapter;
use vela_hot_reload::version::ProgramVersion;
use vela_vm::owned_value::OwnedValue;

const QUICK_REPEATS: usize = 2;
const QUICK_ITERATIONS: usize = 8;
const QUICK_WARMUP: usize = 2;
const DEFAULT_REPEATS: usize = 7;
const DEFAULT_ITERATIONS: usize = 100;
const DEFAULT_WARMUP: usize = 10;

const INITIAL_SOURCE: &str = r#"
#[event("monster.kill")]
fn on_kill(player, monster) {
    return player + monster + 20;
}

fn helper(value) {
    return value + 1;
}

fn main() {
    return on_kill(1, helper(2));
}
"#;

const UPDATED_SOURCE: &str = r#"
#[event("monster.kill")]
fn on_kill(player, monster) {
    let bonus = 30;
    return player + monster + bonus;
}

fn helper(value) {
    return value + 2;
}

fn main() {
    return on_kill(1, helper(2));
}
"#;

const ABI_REJECT_SOURCE: &str = r#"
#[event("monster.kill")]
fn on_kill(player) {
    return player + 30;
}

fn helper(value) {
    return value + 2;
}

fn main() {
    return on_kill(1);
}
"#;

fn main() -> Result<(), Box<dyn Error>> {
    let params = BenchParams::from_args();
    println!(
        "vela_engine_hot_reload profile={} target={}/{} repeats={} iterations={} warmup={}",
        profile(),
        std::env::consts::OS,
        std::env::consts::ARCH,
        params.repeats,
        params.iterations,
        params.warmup
    );

    let engine = Engine::builder().build()?;
    let initial = engine.compile_hot_reload_initial(SourceId::new(1), INITIAL_SOURCE)?;

    let accepted = run_workload("hot_reload_accept", "compile_apply", params, || {
        run_accepted_update(&engine, &initial)
    })?;
    print_result("hot_reload_accept", "compile_apply", accepted);

    let rejected = run_workload("hot_reload_abi_reject", "compile_reject", params, || {
        run_abi_rejection(&engine, &initial)
    })?;
    print_result("hot_reload_abi_reject", "compile_reject", rejected);

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

fn run_workload(
    name: &str,
    mode: &str,
    params: BenchParams,
    mut run_once: impl FnMut() -> Result<u64, Box<dyn Error>>,
) -> Result<BenchResult, Box<dyn Error>> {
    for _ in 0..params.warmup {
        let checksum = run_once()?;
        black_box(checksum);
    }

    let mut samples = Vec::with_capacity(params.repeats);
    let mut checksum = bytes_checksum(name.as_bytes()) ^ bytes_checksum(mode.as_bytes());
    for _ in 0..params.repeats {
        let started = Instant::now();
        for _ in 0..params.iterations {
            let iteration_checksum = run_once()?;
            checksum = mix(checksum, iteration_checksum);
            black_box(iteration_checksum);
        }
        samples.push(started.elapsed());
    }

    Ok(summarize(samples, checksum))
}

fn run_accepted_update(engine: &Engine, initial: &ProgramVersion) -> Result<u64, Box<dyn Error>> {
    let update = engine.compile_hot_reload_update(initial, SourceId::new(2), UPDATED_SOURCE)?;
    let mut runtime = Runtime::from_hot_reload_version(engine.clone(), initial.clone());
    let report = runtime.apply_hot_update(update)?;
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let value = runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx)?;

    Ok(report_checksum(
        report.accepted,
        report.to_version.map(|id| id.0),
        value,
    ))
}

fn run_abi_rejection(engine: &Engine, initial: &ProgramVersion) -> Result<u64, Box<dyn Error>> {
    let update = engine.compile_hot_reload_update(initial, SourceId::new(3), ABI_REJECT_SOURCE);
    let mut runtime = Runtime::from_hot_reload_version(engine.clone(), initial.clone());
    let report = runtime.apply_hot_update_result_report(update)?;
    let active_version = runtime
        .hot_reload_version()
        .map_or(0, |version| version.id.0);

    Ok(report_checksum(
        report.accepted,
        Some(active_version),
        OwnedValue::i64(report.errors.len() as i64),
    ))
}

fn report_checksum(accepted: bool, version: Option<u64>, value: OwnedValue) -> u64 {
    let accepted = u64::from(accepted);
    let version = version.unwrap_or(u64::MAX);
    mix(mix(accepted, version), value_checksum(&value))
}

fn print_result(name: &str, mode: &str, result: BenchResult) {
    println!(
        "bench={} mode={} min_ns={} mean_ns={} median_ns={} p95_ns={} checksum={}",
        name, mode, result.min_ns, result.mean_ns, result.median_ns, result.p95_ns, result.checksum
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
        OwnedValue::Missing => 0x01,
        OwnedValue::Null => 0x02,
        OwnedValue::Bool(value) => u64::from(*value) ^ 0x03,
        OwnedValue::Scalar(value) => scalar_checksum(*value),
        OwnedValue::String(value) => bytes_checksum(value.as_bytes()),
        OwnedValue::Array(values) | OwnedValue::Set(values) => values
            .iter()
            .fold(0x05, |checksum, value| mix(checksum, value_checksum(value))),
        OwnedValue::Map(values) => values.iter().fold(0x06, |checksum, (key, value)| {
            mix(
                mix(checksum, bytes_checksum(key.as_bytes())),
                value_checksum(value),
            )
        }),
        OwnedValue::Record { type_name, fields } => fields.values().fold(
            mix(0x07, bytes_checksum(type_name.as_bytes())),
            |checksum, value| mix(checksum, value_checksum(value)),
        ),
        OwnedValue::Enum {
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
        OwnedValue::Range(_) => 0x09,
        OwnedValue::Closure(_) | OwnedValue::HostRef(_) | OwnedValue::PathProxy(_) => 0x0a,
        OwnedValue::Iterator(_) => 0x0b,
    }
}

fn scalar_checksum(value: vela_common::ScalarValue) -> u64 {
    match value {
        vela_common::ScalarValue::I8(value) => value as i64 as u64,
        vela_common::ScalarValue::I16(value) => value as i64 as u64,
        vela_common::ScalarValue::I32(value) => value as i64 as u64,
        vela_common::ScalarValue::I64(value) => value as u64,
        vela_common::ScalarValue::U8(value) => u64::from(value),
        vela_common::ScalarValue::U16(value) => u64::from(value),
        vela_common::ScalarValue::U32(value) => u64::from(value),
        vela_common::ScalarValue::U64(value) => value,
        vela_common::ScalarValue::F32(value) => u64::from(value.to_bits()),
        vela_common::ScalarValue::F64(value) => value.to_bits(),
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
