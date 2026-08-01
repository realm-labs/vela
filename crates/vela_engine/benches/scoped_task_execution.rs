use std::error::Error;
use std::hint::black_box;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

use vela_common::{Capability, CapabilitySet, SourceId};
use vela_def::FunctionId;
use vela_engine::engine::Engine;
use vela_engine::native::{FunctionAccess, NativeFunctionDesc, TypeHint};
use vela_engine::runtime::{CallArgs, CallOptions, Runtime};
use vela_engine::service::{Service, ServiceRuntimeBinding, ServiceSourceManifest};
use vela_engine::task::{
    ScopedTask, ScopedTaskCompletion, ScopedTaskHost, ScopedTaskOutcome, TaskAdmissionError,
    TaskPolicy, TaskScope,
};
use vela_hir::source_ingestion::build_single_source;
use vela_macros::{service, service_domain};
use vela_vm::budget::{CollectionLimits, ExecutionLimits};
use vela_vm::owned_value::OwnedValue;

const QUICK_ITERATIONS: usize = 500;
const STABLE_ITERATIONS: usize = 10_000;
const WARMUP_ITERATIONS: usize = 50;

const ORDINARY_SOURCE: &str = r#"
async fn ready_worker(value: i64) -> i64 { return value + 1; }
async fn copy_worker(values: Array) -> i64 { return values.len(); }
async fn pending_worker() -> i64 { return bench_task::pending_once().await; }
fn finish(outcome: Result<i64, task::Error>) {}
fn admit_ready(value: i64) { task::spawn_scoped(ready_worker(value)); }
fn admit_copy(values: Array) { task::spawn_scoped(copy_worker(values)); }
fn admit_pending() { task::spawn_scoped(pending_worker()); }
fn admit_then(value: i64) { task::spawn_scoped_then(ready_worker(value), finish); }
"#;

const SERVICE_SOURCE: &str = r#"
async fn service_worker(value: i64) -> i64 {
    let base = service::base::adjust(value);
    return service::pinned::audit::record(base);
}

#[service_impl(task_bench::calculator)]
impl CalculatorPatch {
    fn adjust(value: i64) -> i64 {
        task::spawn_scoped(service_worker(value));
        return value * 10;
    }
}
"#;

#[service(path = "task_bench::calculator")]
pub trait CalculatorService: Send + Sync {
    fn adjust(&self, value: i64) -> i64;
}

struct RustCalculator;

impl CalculatorService for RustCalculator {
    fn adjust(&self, value: i64) -> i64 {
        value + 1
    }
}

#[service(path = "task_bench::audit")]
pub trait AuditService: Send + Sync {
    fn record(&self, value: i64) -> i64;
}

struct RustAudit;

impl AuditService for RustAudit {
    fn record(&self, value: i64) -> i64 {
        value + 100
    }
}

#[service_domain]
pub struct TaskBenchServices {
    pub calculator: Service<dyn CalculatorService>,
    pub audit: Service<dyn AuditService>,
}

#[derive(Default)]
struct CaptureHost {
    tasks: Mutex<Vec<ScopedTask>>,
}

impl ScopedTaskHost for CaptureHost {
    fn admit(&self, task: ScopedTask) -> Result<(), TaskAdmissionError> {
        self.tasks
            .lock()
            .expect("benchmark task host lock")
            .push(task);
        Ok(())
    }
}

impl CaptureHost {
    fn take(&self) -> ScopedTask {
        self.tasks
            .lock()
            .expect("benchmark task host lock")
            .pop()
            .expect("one benchmark task")
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let iterations = if std::env::args().any(|argument| argument == "--stable") {
        STABLE_ITERATIONS
    } else {
        QUICK_ITERATIONS
    };
    println!(
        "suite=scoped_task_execution iterations={iterations} warmup={WARMUP_ITERATIONS} mode=interpreter"
    );

    let engine = ordinary_engine()?;
    let artifact = engine.link_compiled_program(engine.compile_source(ORDINARY_SOURCE)?)?;
    let mut runtime = Runtime::from_linked_artifact(engine.clone(), artifact)?;
    let host = Arc::new(CaptureHost::default());
    let pooled_scope = TaskScope::new(host.clone(), policy(iterations + WARMUP_ITERATIONS + 8));
    let payload = OwnedValue::array(
        (0_i64..64)
            .map(|value| OwnedValue::array([OwnedValue::i64(value), OwnedValue::i64(value + 1)])),
    );

    report("admission_copy_owned_graph", iterations, || {
        runtime.call(
            "admit_copy",
            CallArgs::from_positional([payload.clone()]),
            CallOptions::unbounded().with_task_scope(pooled_scope.clone()),
        )?;
        drop(host.take());
        Ok(1)
    })?;
    report("fresh_runtime_worker", iterations, || {
        let scope = TaskScope::new(host.clone(), policy(1));
        runtime.call(
            "admit_ready",
            CallArgs::from_positional([OwnedValue::i64(41)]),
            CallOptions::unbounded().with_task_scope(scope),
        )?;
        ready_checksum(host.take())
    })?;
    report("pooled_runtime_worker", iterations, || {
        runtime.call(
            "admit_ready",
            CallArgs::from_positional([OwnedValue::i64(41)]),
            CallOptions::unbounded().with_task_scope(pooled_scope.clone()),
        )?;
        ready_checksum(host.take())
    })?;

    let mut pending = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        runtime.call(
            "admit_pending",
            CallArgs::new(),
            CallOptions::unbounded().with_task_scope(pooled_scope.clone()),
        )?;
        pending.push(host.take());
    }
    report_prepared("pending_worker_first_poll", pending, |mut task| {
        let mut context = Context::from_waker(Waker::noop());
        let pending = matches!(task.poll(&mut context), Poll::Pending);
        drop(task);
        u64::from(pending)
    });

    let (service_root, service_host) = service_fixture(iterations)?;
    report("service_nested_dispatch", iterations, || {
        black_box(service_root.calculator()).adjust(black_box(41));
        ready_checksum(service_host.take())
    })?;

    let mut completions = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        runtime.call(
            "admit_then",
            CallArgs::from_positional([OwnedValue::i64(41)]),
            CallOptions::unbounded().with_task_scope(pooled_scope.clone()),
        )?;
        completions.push(ready_completion(host.take())?);
    }
    report_prepared(
        "continuation_safe_point_delivery",
        completions,
        |completion| {
            u64::from(matches!(
                completion.resume(CallArgs::new(), CallOptions::unbounded()),
                vela_engine::task::TaskContinuationOutcome::Completed
            ))
        },
    );

    let metrics = pooled_scope.metrics();
    println!(
        "task_pool hits={} misses={} returns={} discards={}",
        metrics.runtime_pool_hits,
        metrics.runtime_pool_misses,
        metrics.runtime_pool_returns,
        metrics.runtime_pool_discards
    );
    Ok(())
}

fn ordinary_engine() -> Result<Engine, Box<dyn Error>> {
    Ok(Engine::builder()
        .capability(Capability::TaskSpawn)
        .register_async_fn(
            NativeFunctionDesc::new("bench_task::pending_once", FunctionId::new(0xB37A_0001))
                .returns(TypeHint::i64())
                .access(FunctionAccess::public()),
            |_args| {
                Box::pin(async {
                    let mut first = true;
                    std::future::poll_fn(move |context| {
                        if first {
                            first = false;
                            context.waker().wake_by_ref();
                            Poll::Pending
                        } else {
                            Poll::Ready(Ok(OwnedValue::i64(42)))
                        }
                    })
                    .await
                })
            },
        )
        .build()?)
}

fn service_fixture(
    maximum: usize,
) -> Result<(TaskBenchServicesRoot, Arc<CaptureHost>), Box<dyn Error>> {
    let host = Arc::new(CaptureHost::default());
    let task_scope = TaskScope::new(host.clone(), policy(maximum + WARMUP_ITERATIONS + 8));
    let app = TaskBenchServices::builder(Engine::builder().capabilities(CapabilitySet::all()))
        .task_scope(task_scope.clone())
        .emergency_patch_effect_ceiling(vela_engine::native::EffectSet::task_spawn())
        .calculator(RustCalculator)
        .audit(RustAudit)
        .build()?;
    let (engine, services) = app.into_parts();
    let rust = services.pin();
    let sources = build_single_source(SourceId::new(1), SERVICE_SOURCE)
        .map_err(|error| format!("{error:?}"))?;
    let manifest = ServiceSourceManifest::link(sources.graph(), services.schema())?;
    let artifact = engine.link_compiled_program(engine.compile_source(SERVICE_SOURCE)?)?;
    let update = manifest.bind_artifact(artifact)?;
    let candidate = services.stage_snapshot(
        &rust,
        update,
        ServiceRuntimeBinding::for_engine(engine),
        CallOptions::unbounded().with_task_scope(task_scope),
    )?;
    services.activate_if_current(candidate)?;
    Ok((services.pin(), host))
}

fn policy(maximum: usize) -> TaskPolicy {
    TaskPolicy::new(
        std::num::NonZeroUsize::new(maximum).expect("positive task capacity"),
        std::num::NonZeroUsize::new(maximum).expect("positive completion capacity"),
        ExecutionLimits::new(100_000, 1 << 20, 64).with_collection_limits(CollectionLimits {
            max_array_len: 4_096,
            max_map_entries: 4_096,
            max_set_len: 4_096,
        }),
        std::num::NonZeroU64::new(128).expect("positive host call budget"),
        Duration::from_secs(5),
        CapabilitySet::all(),
    )
    .expect("finite benchmark task policy")
}

fn ready_checksum(mut task: ScopedTask) -> Result<u64, Box<dyn Error>> {
    let mut context = Context::from_waker(Waker::noop());
    match task.poll(&mut context) {
        Poll::Ready(ScopedTaskOutcome::Completed(_)) => Ok(1),
        outcome => Err(format!("ready benchmark task did not complete: {outcome:?}").into()),
    }
}

fn ready_completion(mut task: ScopedTask) -> Result<ScopedTaskCompletion, Box<dyn Error>> {
    let mut context = Context::from_waker(Waker::noop());
    match task.poll_completion(&mut context) {
        Poll::Ready(completion) => Ok(completion),
        Poll::Pending => Err("ready benchmark completion remained pending".into()),
    }
}

fn report(
    name: &str,
    iterations: usize,
    mut operation: impl FnMut() -> Result<u64, Box<dyn Error>>,
) -> Result<(), Box<dyn Error>> {
    for _ in 0..WARMUP_ITERATIONS {
        black_box(operation()?);
    }
    let started = Instant::now();
    let mut checksum = 0_u64;
    for _ in 0..iterations {
        checksum = checksum.rotate_left(5) ^ black_box(operation()?);
    }
    print_result(name, iterations, started.elapsed(), checksum);
    Ok(())
}

fn report_prepared<T>(name: &str, values: Vec<T>, mut operation: impl FnMut(T) -> u64) {
    let iterations = values.len();
    let started = Instant::now();
    let mut checksum = 0_u64;
    for value in values {
        checksum = checksum.rotate_left(5) ^ black_box(operation(value));
    }
    print_result(name, iterations, started.elapsed(), checksum);
}

fn print_result(name: &str, iterations: usize, elapsed: Duration, checksum: u64) {
    println!(
        "task_result name={name} iterations={iterations} total_ns={} ns_per_operation={:.1} checksum={checksum}",
        elapsed.as_nanos(),
        elapsed.as_nanos() as f64 / iterations as f64,
    );
}
