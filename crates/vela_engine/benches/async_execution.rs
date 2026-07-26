use std::error::Error;
use std::future::{Future, poll_fn};
use std::hint::black_box;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};
use std::time::Instant;

use vela_common::Capability;
use vela_def::FunctionId;
use vela_engine::engine::Engine;
use vela_engine::native::{FunctionAccess, NativeFunctionDesc, TypeHint};
use vela_engine::runtime::{CallArgs, CallOptions, Runtime};
use vela_macros::{ScriptHost, methods};
use vela_vm::owned_value::OwnedValue;

#[path = "async_execution/provider.rs"]
mod provider;

const DEFAULT_ITERATIONS: usize = 10_000;
const QUICK_ITERATIONS: usize = 1_000;
const STABLE_ITERATIONS: usize = 100_000;
const MEMORY_RUNTIME_COUNT: usize = 2_000;
const MEMORY_EXTRA_FRAME_DEPTH: usize = 16;

fn main() -> Result<(), Box<dyn Error>> {
    match std::env::args().nth(1).as_deref() {
        Some("memory-idle") => memory_workload(None),
        Some("memory-suspended") => memory_workload(Some(0)),
        Some("memory-suspended-deep") => memory_workload(Some(MEMORY_EXTRA_FRAME_DEPTH)),
        Some("--quick") => throughput(QUICK_ITERATIONS),
        Some("--stable") => throughput(STABLE_ITERATIONS),
        _ => throughput(DEFAULT_ITERATIONS),
    }
}

fn throughput(iterations: usize) -> Result<(), Box<dyn Error>> {
    println!("vela_engine_async_execution iterations={iterations}");

    let engine = async_engine()?;
    let mut sync = runtime(&engine, "fn main() -> i64 { return 42; }")?;
    let mut ready = runtime(&engine, "async fn main() -> i64 { return 42; }")?;
    let mut pending = runtime(
        &engine,
        "async fn main() -> i64 { return bench::pending_once().await; }",
    )?;
    let reentry_engine = reentry_engine()?;
    let mut reentry = runtime(
        &reentry_engine,
        "fn child() -> i64 { return 42; } \
         async fn main() -> i64 { return bench::reenter().await; }",
    )?;
    let mut rooted_reentry = runtime(
        &reentry_engine,
        "struct Held { value: i64 } \
         fn make_held() { return Held { value: 42 }; } \
         async fn main() -> i64 { return bench::hold_reentry_root().await; }",
    )?;
    let mut deep = runtime(
        &engine,
        "fn countdown(value: i64) -> i64 { \
             if value == 0 { return 42; } \
             return countdown(value - 1); \
         } \
         fn main() -> i64 { return countdown(10_000); }",
    )?;
    let method_engine = Engine::builder()
        .register_type::<Counter>()
        .register_exports(Counter::vela_inherent_exports())
        .capability(Capability::HostRead)
        .capability(Capability::HostWrite)
        .build()?;
    let mut method = runtime(
        &method_engine,
        "async fn main(counter: Counter) -> i64 { return counter.increment().await; }",
    )?;
    let mut shared_method = runtime(
        &method_engine,
        "async fn main(counter: Counter) -> i64 { return counter.read().await; }",
    )?;
    let mut counter = Counter { value: 0 };
    let mut shared_counter = Counter { value: 42 };
    let mut provider = provider::ProviderBench::new()?;

    report("sync_entry", iterations, || sync_call(&mut sync))?;
    report("ready_async_entry", iterations, || async_call(&mut ready))?;
    report("pending_wake_resume", iterations, || {
        async_call(&mut pending)
    })?;
    report("reentry_scalar", iterations, || async_call(&mut reentry))?;
    report("reentry_dynamic_root", iterations, || {
        async_call(&mut rooted_reentry)
    })?;
    report("deep_call_depth_10000", (iterations / 100).max(10), || {
        sync_call(&mut deep)
    })?;
    report("ready_async_mut_lease", iterations, || {
        let output = poll_to_completion(method.call_async(
            "main",
            CallArgs::new().with_host_mut("counter", &mut counter),
            CallOptions::unbounded(),
        ))?;
        owned_i64(&mut method, &output)
    })?;
    report("ready_async_shared_lease", iterations, || {
        let output = poll_to_completion(shared_method.call_async(
            "main",
            CallArgs::new().with_host_mut("counter", &mut shared_counter),
            CallOptions::unbounded(),
        ))?;
        owned_i64(&mut shared_method, &output)
    })?;
    provider.report(iterations)?;
    Ok(())
}

fn memory_workload(extra_frame_depth: Option<usize>) -> Result<(), Box<dyn Error>> {
    let engine = async_engine()?;
    let source = suspended_source(extra_frame_depth.unwrap_or(0));
    let version = engine.compile_hot_reload_initial(&source)?;
    let mut runtimes = (0..MEMORY_RUNTIME_COUNT)
        .map(|_| {
            Runtime::from_hot_reload_version(engine.clone(), version.clone())
                .expect("runtime should initialize")
        })
        .collect::<Vec<_>>();
    let runtime_count = runtimes.len();
    if let Some(extra_frame_depth) = extra_frame_depth {
        let mut futures = runtimes
            .iter_mut()
            .map(|runtime| runtime.call_async("main", CallArgs::new(), CallOptions::unbounded()))
            .collect::<Vec<_>>();
        let mut context = Context::from_waker(Waker::noop());
        for future in &mut futures {
            assert!(matches!(Pin::new(future).poll(&mut context), Poll::Pending));
        }
        println!(
            "shape=suspended runtimes={} extra_frame_depth={} futures={} future_header_bytes={}",
            runtime_count,
            extra_frame_depth,
            futures.len(),
            std::mem::size_of_val(futures.as_slice())
        );
        black_box(&futures);
        hold_memory_sample();
    } else {
        println!("shape=idle runtimes={runtime_count}");
        black_box(&runtimes);
        hold_memory_sample();
    }
    Ok(())
}

fn hold_memory_sample() {
    let Some(milliseconds) = std::env::var("VELA_BENCH_MEMORY_HOLD_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
    else {
        return;
    };
    std::thread::sleep(std::time::Duration::from_millis(milliseconds.min(10_000)));
}

fn suspended_source(extra_frame_depth: usize) -> String {
    let mut source = String::new();
    for depth in 0..extra_frame_depth {
        let callee = if depth == 0 {
            "bench::pending_once()".to_owned()
        } else {
            format!("frame_{}()", depth - 1)
        };
        source.push_str(&format!(
            "async fn frame_{depth}() -> i64 {{ return {callee}.await; }}\n"
        ));
    }
    let entry = if extra_frame_depth == 0 {
        "bench::pending_once()".to_owned()
    } else {
        format!("frame_{}()", extra_frame_depth - 1)
    };
    source.push_str(&format!(
        "async fn main() -> i64 {{ return {entry}.await; }}\n"
    ));
    source
}

fn async_engine() -> Result<Engine, Box<dyn Error>> {
    Ok(Engine::builder()
        .register_async_fn(
            NativeFunctionDesc::new("bench::pending_once", FunctionId::new(0xA550))
                .returns(TypeHint::i64())
                .access(FunctionAccess::public()),
            |_args| {
                Box::pin(async move {
                    let mut first = true;
                    poll_fn(move |context| {
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

fn reentry_engine() -> Result<Engine, Box<dyn Error>> {
    Ok(Engine::builder()
        .register_async_context_fn(
            NativeFunctionDesc::new("bench::reenter", FunctionId::new(0xA551))
                .returns(TypeHint::i64())
                .access(FunctionAccess::public()),
            |_args, context| {
                Box::pin(async move {
                    let child = context.call("child", CallArgs::new())?;
                    black_box(child);
                    Ok(OwnedValue::i64(42))
                })
            },
        )
        .register_async_context_fn(
            NativeFunctionDesc::new("bench::hold_reentry_root", FunctionId::new(0xA552))
                .returns(TypeHint::i64())
                .access(FunctionAccess::public()),
            |_args, context| {
                Box::pin(async move {
                    let held = context.call("make_held", CallArgs::new())?;
                    black_box(&held);
                    drop(held);
                    Ok(OwnedValue::i64(42))
                })
            },
        )
        .build()?)
}

fn runtime(engine: &Engine, source: &str) -> Result<Runtime, Box<dyn Error>> {
    let version = engine.compile_hot_reload_initial(source)?;
    Ok(Runtime::from_hot_reload_version(engine.clone(), version)
        .expect("runtime should initialize"))
}

fn sync_call(runtime: &mut Runtime) -> Result<i64, Box<dyn Error>> {
    let output = runtime.call("main", CallArgs::new(), CallOptions::unbounded())?;
    owned_i64(runtime, &output)
}

fn async_call(runtime: &mut Runtime) -> Result<i64, Box<dyn Error>> {
    let output =
        poll_to_completion(runtime.call_async("main", CallArgs::new(), CallOptions::unbounded()))?;
    owned_i64(runtime, &output)
}

fn owned_i64(
    runtime: &mut Runtime,
    value: &vela_engine::runtime::VelaValue,
) -> Result<i64, Box<dyn Error>> {
    match runtime.value_to_owned(value)? {
        OwnedValue::Scalar(vela_common::ScalarValue::I64(value)) => Ok(value),
        other => Err(format!("expected i64 benchmark result, got {other:?}").into()),
    }
}

pub(crate) fn report(
    name: &str,
    iterations: usize,
    mut operation: impl FnMut() -> Result<i64, Box<dyn Error>>,
) -> Result<(), Box<dyn Error>> {
    for _ in 0..100 {
        black_box(operation()?);
    }
    let sample_count = if iterations >= STABLE_ITERATIONS {
        3
    } else {
        1
    };
    let mut samples = Vec::with_capacity(sample_count);
    let mut checksum = 0_i64;
    for _ in 0..sample_count {
        let started = Instant::now();
        let mut sample_checksum = 0_i64;
        for _ in 0..iterations {
            sample_checksum = sample_checksum.wrapping_add(black_box(operation()?));
        }
        let elapsed = started.elapsed().as_nanos();
        checksum = checksum.wrapping_add(sample_checksum);
        samples.push(elapsed);
    }
    samples.sort_unstable();
    let minimum = samples[0];
    let median = samples[samples.len() / 2];
    let maximum = samples[samples.len() - 1];
    println!(
        "workload={name} samples={sample_count} iterations_per_sample={iterations} \
         total_ns={median} ns_per_call={:.1} min_ns_per_call={:.1} max_ns_per_call={:.1} \
         checksum={checksum}",
        median as f64 / iterations as f64,
        minimum as f64 / iterations as f64,
        maximum as f64 / iterations as f64,
    );
    Ok(())
}

pub(crate) fn poll_to_completion<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
    }
}

#[derive(ScriptHost)]
#[vela(path = "bench::Counter")]
struct Counter {
    #[vela(get)]
    value: i64,
}

#[methods]
impl Counter {
    pub async fn read(&self) -> i64 {
        self.value
    }

    pub async fn increment(&mut self) -> i64 {
        self.value += 1;
        self.value
    }
}
