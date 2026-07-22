use std::error::Error;
use std::future::Future;
use std::hint::black_box;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Barrier};
use std::task::{Context, Poll, Waker};
use std::thread;
use std::time::{Duration, Instant};

use vela_engine::args::FromScriptArg;
use vela_engine::binding::VmResult;
use vela_engine::engine::Engine;
use vela_engine::runtime::{
    CallArgs, CallOptions, Runtime, RuntimeImage, SharedImage, SharedRuntime,
};
use vela_macros::export;

const STABLE_HOT_CALLS_PER_WORKER: usize = 5_000;
const QUICK_HOT_CALLS_PER_WORKER: usize = 200;
const STABLE_COLD_ACTORS_PER_WORKER: usize = 128;
const QUICK_COLD_ACTORS_PER_WORKER: usize = 16;
const STABLE_CONTENTION_CALLS_PER_WORKER: usize = 2_000;
const QUICK_CONTENTION_CALLS_PER_WORKER: usize = 100;

static PENDING_RELEASED: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy)]
enum Sampling {
    Quick,
    Stable,
}

impl Sampling {
    fn from_args() -> Self {
        if std::env::args().any(|arg| arg == "--quick") {
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

    fn contention_calls_per_worker(self) -> usize {
        match self {
            Self::Quick => QUICK_CONTENTION_CALLS_PER_WORKER,
            Self::Stable => STABLE_CONTENTION_CALLS_PER_WORKER,
        }
    }
}

struct ActorTurn {
    runtime: SharedRuntime,
}

impl ActorTurn {
    fn new(image: SharedImage) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            runtime: SharedRuntime::from_shared_image(image)?,
        })
    }
}

async fn actor_turn(turn: &mut ActorTurn, value: i64) -> VmResult<i64> {
    let result = turn
        .runtime
        .call_async(
            "actor_turn",
            CallArgs::new().with_value("value", value),
            CallOptions::unbounded(),
        )
        .await?;
    i64::from_script_arg(&turn.runtime.value_to_owned(&result)?)
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
    let sampling = Sampling::from_args();
    let fixture = Arc::new(ConcurrencyFixture::new()?);
    let available_workers = thread::available_parallelism()?.get();
    let mut worker_counts = vec![1, 2, available_workers];
    worker_counts.sort_unstable();
    worker_counts.dedup();
    println!(
        "suite=actor_concurrency sampling={} allocator=system available_workers={} hot_calls_per_worker={} cold_actors_per_worker={} contention_calls_per_worker={}",
        sampling.label(),
        available_workers,
        sampling.hot_calls_per_worker(),
        sampling.cold_actors_per_worker(),
        sampling.contention_calls_per_worker()
    );
    pending_actor_overlap(&fixture)?;
    for workers in worker_counts {
        concurrent_hot(&fixture, workers, sampling.hot_calls_per_worker())?;
        concurrent_cold(&fixture, workers, sampling.cold_actors_per_worker())?;
        dynamic_cache_contention(workers, sampling.contention_calls_per_worker())?;
    }
    Ok(())
}

struct ConcurrencyFixture {
    image: SharedImage,
}

impl ConcurrencyFixture {
    fn new() -> Result<Self, Box<dyn Error>> {
        let engine = Engine::builder()
            .register_exports(vela_export_bundle_wait_for_release())
            .build()?;
        let program = engine.compile_source(
            r#"
state calls: i64 = 0;

pub async fn actor_turn(value: i64) -> i64 {
    if value == 0 {
        return bench::wait_for_release(41).await;
    }
    calls += 1;
    return calls + value;
}
"#,
        )?;
        let image = vela_engine::runtime::RuntimeImage::new_compiled(engine, program).into_shared();
        Ok(Self { image })
    }

    fn actor(&self) -> Result<ActorTurn, Box<dyn Error>> {
        ActorTurn::new(self.image.clone())
    }
}

fn pending_actor_overlap(fixture: &ConcurrencyFixture) -> Result<(), Box<dyn Error>> {
    PENDING_RELEASED.store(false, Ordering::Release);
    let mut pending_actor = fixture.actor()?;
    let mut independent_actor = fixture.actor()?;
    let mut pending = Box::pin(actor_turn(&mut pending_actor, 0));
    let mut task = Context::from_waker(Waker::noop());
    if !matches!(pending.as_mut().poll(&mut task), Poll::Pending) {
        return Err("pending Actor unexpectedly completed before release".into());
    }
    let started = Instant::now();
    let independent = poll_to_completion(actor_turn(&mut independent_actor, 41))?;
    let independent_latency = started.elapsed();
    PENDING_RELEASED.store(true, Ordering::Release);
    let pending_result = poll_pinned_to_completion(pending.as_mut())?;
    println!(
        "concurrency_result mode=pending_overlap workers=2 calls=2 throughput_per_sec={:.3} p50_ns={} p95_ns={} p99_ns={} allocation_count_source=actor_memory_calibration lock_wait_ns=0 lock_wait_source=workload_has_no_mutable_cache_site independent_result={} pending_result={} overlapped=true",
        2.0 / independent_latency.as_secs_f64(),
        independent_latency.as_nanos(),
        independent_latency.as_nanos(),
        independent_latency.as_nanos(),
        independent,
        pending_result
    );
    Ok(())
}

fn concurrent_hot(
    fixture: &ConcurrencyFixture,
    workers: usize,
    calls_per_worker: usize,
) -> Result<(), Box<dyn Error>> {
    let mut actors = (0..workers)
        .map(|_| fixture.actor())
        .collect::<Result<Vec<_>, _>>()?;
    for actor in &mut actors {
        black_box(poll_to_completion(actor_turn(actor, 41))?);
    }
    let barrier = Arc::new(Barrier::new(workers));
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
        join_workers(handles, "cache-hot")
    })
    .map_err(|error| -> Box<dyn Error> { error.into() })?;
    report_concurrency("cache_hot", workers, wall_started.elapsed(), results);
    Ok(())
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
        join_workers(handles, "cache-cold")
    })
    .map_err(|error| -> Box<dyn Error> { error.into() })?;
    report_concurrency("cache_cold", workers, wall_started.elapsed(), results);
    Ok(())
}

fn join_workers(
    handles: Vec<thread::ScopedJoinHandle<'_, Result<WorkerResult, String>>>,
    mode: &str,
) -> Result<Vec<WorkerResult>, String> {
    handles
        .into_iter()
        .map(|handle| {
            handle
                .join()
                .map_err(|_| format!("{mode} benchmark worker panicked"))?
        })
        .collect()
}

struct WorkerResult {
    samples: Vec<Duration>,
    checksum: i64,
}

fn report_concurrency(mode: &str, workers: usize, elapsed: Duration, results: Vec<WorkerResult>) {
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
        "concurrency_result mode={} workers={} calls={} throughput_per_sec={:.3} p50_ns={} p95_ns={} p99_ns={} allocation_count_source=actor_memory_calibration lock_wait_ns=0 lock_wait_source=workload_has_no_mutable_cache_site checksum={}",
        mode,
        workers,
        calls,
        calls as f64 / elapsed.as_secs_f64(),
        percentile_ns(&samples, 50),
        percentile_ns(&samples, 95),
        percentile_ns(&samples, 99),
        checksum
    );
}

const DYNAMIC_CONTENTION_SOURCE: &str = r#"
struct Label {
    text: String,
}

impl Label {
    fn starts_with(self, prefix: String) -> bool {
        return self.text.starts_with(prefix);
    }
}

fn matches_prefix(value) {
    return value.starts_with("q");
}

fn string_worker() {
    let total = 0;
    for tick in 0..32 {
        if matches_prefix("quest") {
            total += 1;
        }
        total += tick - tick;
    }
    return total;
}

fn label_worker() {
    let total = 0;
    for tick in 0..32 {
        if matches_prefix(Label { text: "quick" }) {
            total += 1;
        }
        total += tick - tick;
    }
    return total;
}
"#;

fn dynamic_cache_contention(workers: usize, calls_per_worker: usize) -> Result<(), Box<dyn Error>> {
    let engine = Engine::builder().build()?;
    let program = engine.compile_source(DYNAMIC_CONTENTION_SOURCE)?;
    let artifact = engine.link_compiled_program(program)?;
    let shared_image =
        RuntimeImage::from_linked_artifact(engine, Arc::clone(&artifact)).into_shared();
    let shared = (0..workers)
        .map(|_| SharedRuntime::from_shared_image(shared_image.clone()))
        .collect::<Result<Vec<_>, _>>()?;
    let isolated = (0..workers)
        .map(|_| {
            Runtime::from_linked_artifact(
                Engine::builder()
                    .build()
                    .expect("isolated Engine should build"),
                Arc::clone(&artifact),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    let shared_result = run_dynamic_workers(shared, calls_per_worker)?;
    let isolated_result = run_dynamic_workers(isolated, calls_per_worker)?;
    let delta =
        (shared_result.throughput_per_sec / isolated_result.throughput_per_sec - 1.0) * 100.0;
    println!(
        "contention_result family=dynamic_method workers={} calls={} shared_throughput_per_sec={:.3} isolated_throughput_per_sec={:.3} shared_vs_isolated_pct={:.3} shared_p95_ns={} isolated_p95_ns={} shared_checksum={} isolated_checksum={} lock_wait_ns=unmeasured contention_signal=shared_vs_isolated_execution_data",
        workers,
        workers * calls_per_worker,
        shared_result.throughput_per_sec,
        isolated_result.throughput_per_sec,
        delta,
        shared_result.p95_ns,
        isolated_result.p95_ns,
        shared_result.checksum,
        isolated_result.checksum,
    );
    Ok(())
}

fn run_dynamic_workers<I>(
    mut runtimes: Vec<vela_engine::runtime::RuntimeImpl<I>>,
    calls_per_worker: usize,
) -> Result<ContentionResult, Box<dyn Error>>
where
    I: vela_engine::runtime::RuntimeImageStorage + Send,
{
    for (worker, runtime) in runtimes.iter_mut().enumerate() {
        black_box(call_dynamic(runtime, dynamic_entry(worker))?);
    }
    let barrier = Arc::new(Barrier::new(runtimes.len()));
    let wall_started = Instant::now();
    let results = thread::scope(|scope| {
        let handles = runtimes
            .into_iter()
            .enumerate()
            .map(|(worker, mut runtime)| {
                let barrier = Arc::clone(&barrier);
                scope.spawn(move || -> Result<WorkerResult, String> {
                    let mut samples = Vec::with_capacity(calls_per_worker);
                    let mut checksum = 0_i64;
                    barrier.wait();
                    for _ in 0..calls_per_worker {
                        let started = Instant::now();
                        let value = call_dynamic(&mut runtime, dynamic_entry(worker))
                            .map_err(|error| error.to_string())?;
                        samples.push(started.elapsed());
                        checksum = checksum.wrapping_add(value);
                    }
                    Ok(WorkerResult { samples, checksum })
                })
            })
            .collect::<Vec<_>>();
        join_workers(handles, "dynamic-method-contention")
    })
    .map_err(|error| -> Box<dyn Error> { error.into() })?;
    let elapsed = wall_started.elapsed();
    let checksum = results
        .iter()
        .fold(0_i64, |sum, result| sum.wrapping_add(result.checksum));
    let mut samples = results
        .into_iter()
        .flat_map(|result| result.samples)
        .collect::<Vec<_>>();
    samples.sort_unstable();
    Ok(ContentionResult {
        throughput_per_sec: samples.len() as f64 / elapsed.as_secs_f64(),
        p95_ns: percentile_ns(&samples, 95),
        checksum,
    })
}

fn dynamic_entry(worker: usize) -> &'static str {
    if worker.is_multiple_of(2) {
        "string_worker"
    } else {
        "label_worker"
    }
}

fn call_dynamic<I>(
    runtime: &mut vela_engine::runtime::RuntimeImpl<I>,
    target: &str,
) -> Result<i64, Box<dyn Error>>
where
    I: vela_engine::runtime::RuntimeImageStorage,
{
    let value = runtime.call(target, CallArgs::new(), CallOptions::unbounded())?;
    let owned = runtime.value_to_owned(&value)?;
    Ok(<i64 as vela_engine::args::FromScriptArg>::from_script_arg(
        &owned,
    )?)
}

struct ContentionResult {
    throughput_per_sec: f64,
    p95_ns: u128,
    checksum: i64,
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
