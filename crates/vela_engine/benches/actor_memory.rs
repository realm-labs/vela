use std::alloc::System;
use std::error::Error;
use std::future::Future;
use std::hint::black_box;
use std::pin::Pin;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Barrier};
use std::task::{Context, Poll, Waker};
use std::thread;
use std::time::{Duration, Instant};

use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};
use vela_common::StateSlot;
use vela_engine::binding::VmResult;
use vela_engine::dispatch::{
    DispatchAuthority, DispatchController, DispatchInvocation, DispatchRoot,
};
use vela_engine::engine::Engine;
use vela_engine::runtime::{Runtime, SharedImage, SharedRuntime};
use vela_hot_reload::version::ProgramVersion;
use vela_macros::{ScriptHost, ScriptReflect, export, replaceable};
use vela_vm::{
    DynamicMethodInlineCacheEntry, HostInlineCacheEntry, MethodInlineCacheEntry,
    NativeInlineCacheEntry, RecordFieldInlineCacheEntry,
};

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

const LARGE_FUNCTION_COUNT: usize = 256;
const STABLE_HOT_CALLS_PER_WORKER: usize = 5_000;
const QUICK_HOT_CALLS_PER_WORKER: usize = 200;
const STABLE_COLD_ACTORS_PER_WORKER: usize = 128;
const QUICK_COLD_ACTORS_PER_WORKER: usize = 16;
const DEFAULT_RSS_CEILING_MIB: u64 = 1_536;
const DEFAULT_CHILD_TIME_CEILING_SECS: u64 = 90;

static PENDING_RELEASED: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy)]
enum Sampling {
    Quick,
    Stable,
}

impl Sampling {
    fn from_args(args: &[String]) -> Self {
        if args.iter().any(|arg| arg == "--quick") {
            Self::Quick
        } else {
            Self::Stable
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Quick => "quick",
            Self::Stable => "stable",
        }
    }

    fn runtime_counts(self) -> &'static [usize] {
        match self {
            Self::Quick => &[1, 100],
            Self::Stable => &[1, 100, 10_000],
        }
    }

    fn hot_calls_per_worker(self) -> usize {
        match self {
            Self::Quick => QUICK_HOT_CALLS_PER_WORKER,
            Self::Stable => STABLE_HOT_CALLS_PER_WORKER,
        }
    }

    fn cold_actors_per_worker(self) -> usize {
        match self {
            Self::Quick => QUICK_COLD_ACTORS_PER_WORKER,
            Self::Stable => STABLE_COLD_ACTORS_PER_WORKER,
        }
    }
}

#[derive(Clone, Copy)]
enum ArtifactShape {
    Small,
    Large,
}

impl ArtifactShape {
    fn label(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Large => "large",
        }
    }

    fn parse(value: &str) -> Result<Self, Box<dyn Error>> {
        match value {
            "small" => Ok(Self::Small),
            "large" => Ok(Self::Large),
            _ => Err(format!("unknown artifact shape `{value}`").into()),
        }
    }
}

#[derive(ScriptHost, ScriptReflect)]
#[script(path = "bench::ActorTurn")]
struct ActorTurn {
    #[script(skip)]
    root: DispatchRoot,
    #[script(skip)]
    runtime: SharedRuntime,
}

impl ActorTurn {
    fn new(root: DispatchRoot, image: SharedImage) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            root,
            runtime: SharedRuntime::from_shared_image(image)?,
        })
    }
}

impl DispatchAuthority for ActorTurn {
    fn vela_dispatch_root(&self) -> &DispatchRoot {
        &self.root
    }

    fn vela_dispatch_invocation(&mut self) -> VmResult<DispatchInvocation<'_>> {
        let Self { root, runtime } = self;
        root.invocation(runtime)
    }
}

#[replaceable(path = "host::bench::actor_turn", authority = "turn", index = 0)]
pub async fn actor_turn(turn: &mut ActorTurn, value: i64) -> VmResult<i64> {
    let _ = turn;
    Ok(value + 1)
}

#[export(path = "bench::wait_for_release")]
pub async fn wait_for_release(value: i64) -> VmResult<i64> {
    std::future::poll_fn(|_| {
        if PENDING_RELEASED.load(Ordering::Acquire) {
            Poll::Ready(Ok(value))
        } else {
            Poll::Pending
        }
    })
    .await
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let sampling = Sampling::from_args(&args);
    match args.first().map(String::as_str) {
        Some("memory") => memory_suite(sampling),
        Some("allocations") => allocation_calibration(sampling),
        Some("_memory-child") => memory_child(&args),
        Some("all") | None => {
            memory_suite(sampling)?;
            allocation_calibration(sampling)
        }
        Some(mode) => Err(format!("unknown actor-runtime benchmark mode `{mode}`").into()),
    }
}

fn memory_suite(sampling: Sampling) -> Result<(), Box<dyn Error>> {
    let executable = std::env::current_exe()?;
    let rss_ceiling_bytes =
        env_u64("VELA_ACTOR_RSS_CEILING_MIB", DEFAULT_RSS_CEILING_MIB).saturating_mul(1024 * 1024);
    let time_ceiling = Duration::from_secs(env_u64(
        "VELA_ACTOR_TIME_CEILING_SECS",
        DEFAULT_CHILD_TIME_CEILING_SECS,
    ));
    println!(
        "suite=actor_memory sampling={} rss_ceiling_bytes={} time_ceiling_ms={}",
        sampling.label(),
        rss_ceiling_bytes,
        time_ceiling.as_millis()
    );
    print_entry_sizes();
    for shape in [ArtifactShape::Small, ArtifactShape::Large] {
        for &runtime_count in sampling.runtime_counts() {
            let mut child = Command::new(&executable)
                .args(["_memory-child", shape.label(), &runtime_count.to_string()])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?;
            let started = Instant::now();
            let mut peak_rss_bytes = 0_u64;
            let mut capacity_failure = None;
            loop {
                if let Some(status) = child.try_wait()? {
                    let output = child.wait_with_output()?;
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    peak_rss_bytes = peak_rss_bytes
                        .max(output_metric(&stdout, "rss_after_bytes").unwrap_or(peak_rss_bytes));
                    print!("{stdout}");
                    if !status.success() {
                        println!(
                            "memory_result shape={} runtimes={} status=child_error exit={status} peak_rss_bytes={} stderr={:?}",
                            shape.label(),
                            runtime_count,
                            peak_rss_bytes,
                            stderr.trim()
                        );
                    } else {
                        println!(
                            "memory_result shape={} runtimes={} status=ok peak_rss_bytes={}",
                            shape.label(),
                            runtime_count,
                            peak_rss_bytes
                        );
                    }
                    break;
                }
                peak_rss_bytes = peak_rss_bytes.max(process_rss_bytes(child.id()).unwrap_or(0));
                if peak_rss_bytes > rss_ceiling_bytes {
                    capacity_failure = Some("rss_ceiling");
                } else if started.elapsed() > time_ceiling {
                    capacity_failure = Some("time_ceiling");
                }
                if let Some(reason) = capacity_failure {
                    child.kill()?;
                    let output = child.wait_with_output()?;
                    println!(
                        "memory_result shape={} runtimes={} status=capacity_failure reason={} peak_rss_bytes={} elapsed_ms={} stderr={:?}",
                        shape.label(),
                        runtime_count,
                        reason,
                        peak_rss_bytes,
                        started.elapsed().as_millis(),
                        String::from_utf8_lossy(&output.stderr).trim()
                    );
                    break;
                }
                thread::sleep(Duration::from_millis(5));
            }
        }
    }
    Ok(())
}

fn print_entry_sizes() {
    println!(
        "entry_sizes state_read_bytes={} host_access_bytes={} record_field_bytes={} method_dispatch_bytes={} dynamic_method_bytes={} native_call_bytes={} profile_counter_bytes={}",
        std::mem::size_of::<Option<StateSlot>>(),
        std::mem::size_of::<Option<HostInlineCacheEntry>>(),
        std::mem::size_of::<Option<RecordFieldInlineCacheEntry>>(),
        std::mem::size_of::<Option<MethodInlineCacheEntry>>(),
        std::mem::size_of::<Option<DynamicMethodInlineCacheEntry>>(),
        std::mem::size_of::<Option<NativeInlineCacheEntry>>(),
        std::mem::size_of::<u64>()
    );
}

fn memory_child(args: &[String]) -> Result<(), Box<dyn Error>> {
    let shape = ArtifactShape::parse(args.get(1).ok_or("missing artifact shape")?)?;
    let runtime_count = args
        .get(2)
        .ok_or("missing Runtime count")?
        .parse::<usize>()?;
    let engine = Engine::builder().build()?;
    let version = engine.compile_hot_reload_initial(&memory_source(shape))?;
    let cache_sites = version.linked_artifact().cache_layout().len();
    let instruction_count = instruction_count(&version);
    let state_schema_count = version.linked_program().states().len();
    let actor_state_payload_bytes = state_schema_count.saturating_mul(std::mem::size_of::<i64>());
    let rss_before = process_rss_bytes(std::process::id()).unwrap_or(0);
    let allocation_region = Region::new(GLOBAL);
    let started = Instant::now();
    let runtimes = (0..runtime_count)
        .map(|_| Runtime::from_hot_reload_version(engine.clone(), version.clone()))
        .collect::<Result<Vec<_>, _>>()?;
    let construction = started.elapsed();
    let allocation = allocation_region.change();
    let rss_after = process_rss_bytes(std::process::id()).unwrap_or(rss_before);
    println!(
        "memory_child shape={} runtimes={} construction_ns={} retained_rss_bytes={} rss_before_bytes={} rss_after_bytes={} allocation_count={} allocated_bytes={} deallocated_bytes={} cache_sites={} instruction_count={} state_schema_count={} actor_state_payload_bytes={}",
        shape.label(),
        runtimes.len(),
        construction.as_nanos(),
        rss_after.saturating_sub(rss_before),
        rss_before,
        rss_after,
        allocation.allocations,
        allocation.bytes_allocated,
        allocation.bytes_deallocated,
        cache_sites,
        instruction_count,
        state_schema_count,
        actor_state_payload_bytes.saturating_mul(runtime_count)
    );
    black_box(&runtimes);
    Ok(())
}

fn allocation_calibration(sampling: Sampling) -> Result<(), Box<dyn Error>> {
    let fixture = Arc::new(ConcurrencyFixture::new()?);
    println!(
        "suite=actor_allocation_calibration sampling={} allocator=stats_alloc hot_calls={} cold_actors={}",
        sampling.label(),
        sampling.hot_calls_per_worker(),
        sampling.cold_actors_per_worker()
    );
    concurrent_hot(&fixture, 1, sampling.hot_calls_per_worker())?;
    concurrent_cold(&fixture, 1, sampling.cold_actors_per_worker())
}

struct ConcurrencyFixture {
    root: DispatchRoot,
    image: SharedImage,
}

impl ConcurrencyFixture {
    fn new() -> Result<Self, Box<dyn Error>> {
        let slots = vec![vela_replaceable_slot_actor_turn()];
        let engine = Engine::builder()
            .register_host_type::<ActorTurn>()
            .register_exports(vela_export_bundle_wait_for_release())
            .register_replaceable_slots(slots.clone())
            .build()?;
        let program = engine.compile_source(
            r#"
state calls: i64 = 0;

#[override(host::bench::actor_turn)]
pub async fn patched(value: i64) -> i64 {
    if value == 0 {
        return bench::wait_for_release(41).await;
    }
    calls += 1;
    return calls + value;
}
"#,
        )?;
        let image = vela_engine::runtime::RuntimeImage::new_compiled(engine, program).into_shared();
        let staging = SharedRuntime::from_shared_image(image.clone())?;
        let controller = DispatchController::new(slots)?;
        let candidate = controller.stage_current(&staging)?;
        controller.activate(candidate)?;
        Ok(Self {
            root: DispatchRoot::pin(&controller),
            image,
        })
    }

    fn actor(&self) -> Result<ActorTurn, Box<dyn Error>> {
        ActorTurn::new(self.root.clone(), self.image.clone())
    }
}

fn concurrent_hot(
    fixture: &ConcurrencyFixture,
    workers: usize,
    calls_per_worker: usize,
) -> Result<(), Box<dyn Error>> {
    let actors = (0..workers)
        .map(|_| fixture.actor())
        .collect::<Result<Vec<_>, _>>()?;
    run_concurrent("cache_hot", actors, calls_per_worker, true)
}

fn concurrent_cold(
    fixture: &ConcurrencyFixture,
    workers: usize,
    actors_per_worker: usize,
) -> Result<(), Box<dyn Error>> {
    let actors = (0..workers)
        .map(|_| {
            (0..actors_per_worker)
                .map(|_| fixture.actor())
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()?;
    let barrier = Arc::new(Barrier::new(workers));
    let allocation_region = Region::new(GLOBAL);
    let wall_started = Instant::now();
    let results = thread::scope(|scope| {
        let handles = actors
            .into_iter()
            .map(|worker_actors| {
                let barrier = Arc::clone(&barrier);
                scope.spawn(move || -> Result<WorkerResult, String> {
                    let mut samples = Vec::with_capacity(worker_actors.len());
                    let mut checksum = 0_i64;
                    barrier.wait();
                    for mut actor in worker_actors {
                        let started = Instant::now();
                        let value = poll_to_completion(actor_turn(&mut actor, 41))
                            .map_err(|error| error.to_string())?;
                        samples.push(started.elapsed());
                        checksum = checksum.wrapping_add(value);
                    }
                    Ok(WorkerResult { samples, checksum })
                })
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .map_err(|_| "cache-cold benchmark worker panicked".to_owned())?
            })
            .collect::<Result<Vec<_>, String>>()
    })
    .map_err(|error| -> Box<dyn Error> { error.into() })?;
    report_concurrency(
        "cache_cold",
        workers,
        wall_started.elapsed(),
        results,
        allocation_region.change(),
    );
    Ok(())
}

fn run_concurrent(
    mode: &str,
    mut actors: Vec<ActorTurn>,
    calls_per_worker: usize,
    warm: bool,
) -> Result<(), Box<dyn Error>> {
    if warm {
        for actor in &mut actors {
            black_box(poll_to_completion(actor_turn(actor, 41))?);
        }
    }
    let workers = actors.len();
    let barrier = Arc::new(Barrier::new(workers));
    let allocation_region = Region::new(GLOBAL);
    let wall_started = Instant::now();
    let results = thread::scope(|scope| {
        let handles = actors
            .into_iter()
            .map(|mut actor| {
                let barrier = Arc::clone(&barrier);
                scope.spawn(move || -> Result<WorkerResult, String> {
                    let mut samples = Vec::with_capacity(calls_per_worker);
                    let mut checksum = 0_i64;
                    barrier.wait();
                    for _ in 0..calls_per_worker {
                        let started = Instant::now();
                        let value = poll_to_completion(actor_turn(&mut actor, 41))
                            .map_err(|error| error.to_string())?;
                        samples.push(started.elapsed());
                        checksum = checksum.wrapping_add(value);
                    }
                    Ok(WorkerResult { samples, checksum })
                })
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .map_err(|_| "cache-hot benchmark worker panicked".to_owned())?
            })
            .collect::<Result<Vec<_>, String>>()
    })
    .map_err(|error| -> Box<dyn Error> { error.into() })?;
    report_concurrency(
        mode,
        workers,
        wall_started.elapsed(),
        results,
        allocation_region.change(),
    );
    Ok(())
}

struct WorkerResult {
    samples: Vec<Duration>,
    checksum: i64,
}

fn report_concurrency(
    mode: &str,
    workers: usize,
    elapsed: Duration,
    results: Vec<WorkerResult>,
    allocation: stats_alloc::Stats,
) {
    let checksum = results
        .iter()
        .fold(0_i64, |sum, result| sum.wrapping_add(result.checksum));
    let mut samples = results
        .into_iter()
        .flat_map(|result| result.samples)
        .collect::<Vec<_>>();
    samples.sort_unstable();
    let calls = samples.len();
    println!(
        "concurrency_result mode={} workers={} calls={} throughput_per_sec={:.3} p50_ns={} p95_ns={} p99_ns={} allocation_count={} allocated_bytes={} deallocated_bytes={} lock_wait_ns=0 lock_wait_source=structural_no_shared_runtime_or_cache_lock checksum={}",
        mode,
        workers,
        calls,
        calls as f64 / elapsed.as_secs_f64(),
        percentile_ns(&samples, 50),
        percentile_ns(&samples, 95),
        percentile_ns(&samples, 99),
        allocation.allocations,
        allocation.bytes_allocated,
        allocation.bytes_deallocated,
        checksum
    );
}

fn poll_to_completion<T>(future: impl Future<Output = VmResult<T>>) -> VmResult<T> {
    let mut future = std::pin::pin!(future);
    poll_pinned_to_completion(future.as_mut())
}

fn poll_pinned_to_completion<T>(
    mut future: Pin<&mut impl Future<Output = VmResult<T>>>,
) -> VmResult<T> {
    let mut task = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(result) = future.as_mut().poll(&mut task) {
            return result;
        }
        thread::yield_now();
    }
}

fn percentile_ns(samples: &[Duration], percentile: usize) -> u128 {
    if samples.is_empty() {
        return 0;
    }
    let index = ((samples.len() - 1) * percentile).div_ceil(100);
    samples[index].as_nanos()
}

fn instruction_count(version: &ProgramVersion) -> usize {
    version
        .linked_program()
        .functions()
        .map(|(_, code)| code.instructions.len())
        .sum()
}

fn memory_source(shape: ArtifactShape) -> String {
    match shape {
        ArtifactShape::Small => {
            "state counter: i64 = 0; pub fn main(value: i64) -> i64 { counter += value; return counter; }".to_owned()
        }
        ArtifactShape::Large => {
            let mut source = "state counter: i64 = 0;\n".to_owned();
            for index in 0..LARGE_FUNCTION_COUNT {
                source.push_str(&format!(
                    "fn function_{index}(value: i64) -> i64 {{ counter += value; return counter + {index}; }}\n"
                ));
            }
            source.push_str("pub fn main(value: i64) -> i64 { return function_0(value); }\n");
            source
        }
    }
}

fn process_rss_bytes(pid: u32) -> Option<u64> {
    let output = Command::new("ps")
        .args(["-o", "rss=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    let kib = String::from_utf8(output.stdout)
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()?;
    Some(kib.saturating_mul(1024))
}

fn output_metric(output: &str, name: &str) -> Option<u64> {
    output
        .split_ascii_whitespace()
        .find_map(|field| field.strip_prefix(&format!("{name}=")))?
        .parse()
        .ok()
}

fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}
