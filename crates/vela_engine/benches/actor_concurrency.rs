use std::error::Error;
use std::future::Future;
use std::hint::black_box;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Barrier};
use std::task::{Context, Poll, Waker};
use std::thread;
use std::time::{Duration, Instant};

use vela_engine::binding::VmResult;
use vela_engine::dispatch::{
    DispatchAuthority, DispatchController, DispatchInvocation, DispatchRoot,
};
use vela_engine::engine::Engine;
use vela_engine::runtime::{SharedImage, SharedRuntime};
use vela_macros::{ScriptHost, ScriptReflect, export, replaceable};

const STABLE_HOT_CALLS_PER_WORKER: usize = 5_000;
const QUICK_HOT_CALLS_PER_WORKER: usize = 200;
const STABLE_COLD_ACTORS_PER_WORKER: usize = 128;
const QUICK_COLD_ACTORS_PER_WORKER: usize = 16;

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
    let sampling = Sampling::from_args();
    let fixture = Arc::new(ConcurrencyFixture::new()?);
    let available_workers = thread::available_parallelism()?.get();
    let mut worker_counts = vec![1, 2, available_workers];
    worker_counts.sort_unstable();
    worker_counts.dedup();
    println!(
        "suite=actor_concurrency sampling={} allocator=system available_workers={} hot_calls_per_worker={} cold_actors_per_worker={}",
        sampling.label(),
        available_workers,
        sampling.hot_calls_per_worker(),
        sampling.cold_actors_per_worker()
    );
    pending_actor_overlap(&fixture)?;
    for workers in worker_counts {
        concurrent_hot(&fixture, workers, sampling.hot_calls_per_worker())?;
        concurrent_cold(&fixture, workers, sampling.cold_actors_per_worker())?;
    }
    Ok(())
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
        "concurrency_result mode=pending_overlap workers=2 calls=2 throughput_per_sec={:.3} p50_ns={} p95_ns={} p99_ns={} allocation_count_source=actor_memory_calibration lock_wait_ns=0 lock_wait_source=structural_no_shared_runtime_or_cache_lock independent_result={} pending_result={} overlapped=true",
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
        "concurrency_result mode={} workers={} calls={} throughput_per_sec={:.3} p50_ns={} p95_ns={} p99_ns={} allocation_count_source=actor_memory_calibration lock_wait_ns=0 lock_wait_source=structural_no_shared_runtime_or_cache_lock checksum={}",
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
