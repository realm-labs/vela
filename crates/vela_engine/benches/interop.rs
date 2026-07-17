use std::error::Error;
use std::hint::black_box;
use std::sync::Arc;
use std::time::Instant;

use parking_lot::Mutex;
use vela_bytecode::{RustBindingCallableIdentity, RustBindingSchema};
use vela_common::Capability;
use vela_engine::binding::{
    BindingAuthority, BindingCallable, BindingCallableIdentitySpec, BindingCallableSpec,
    BindingSchemaSpec, RootBinding, VmResult,
};
use vela_engine::context::NativeCallContext;
use vela_engine::dispatch::{DispatchAuthority, DispatchController, DispatchRoot};
use vela_engine::engine::Engine;
use vela_engine::runtime::{CallArgs, CallOptions, Runtime};
use vela_macros::{ScriptHost, ScriptReflect, export, methods, replaceable};

const DEFAULT_ITERATIONS: usize = 10_000;
const QUICK_ITERATIONS: usize = 1_000;
const STABLE_ITERATIONS: usize = 100_000;

const SOURCE: &str = r#"
pub fn generated_target(value: i64) -> i64 { return value + 1; }
fn child(value: i64) -> i64 { return value + 1; }
fn scalar_entry(value: i64) -> i64 { return bench::scalar(value); }
fn shared_entry(player: Player) -> i64 { return bench::read_player(player); }
fn exclusive_entry(player: Player) -> i64 { return bench::write_player(player); }
fn round_trip_entry(value: i64) -> i64 { return bench::round_trip(value); }
"#;

const DISPATCH_SOURCE: &str = r#"
#[override(host::bench::replaceable_scalar)]
fn replaceable_patch(context: DispatchContext, value: i64) -> i64 {
    return context.marker + value + 1;
}
"#;

#[derive(ScriptHost, ScriptReflect)]
#[script(path = "bench::Player")]
pub struct Player {
    #[script(get, set)]
    value: i64,
}

#[methods]
impl Player {
    pub fn current(&self) -> i64 {
        self.value
    }
}

#[derive(ScriptHost, ScriptReflect)]
#[script(path = "bench::DispatchContext")]
pub struct DispatchContext {
    #[script(get)]
    marker: i64,
    #[script(skip)]
    root: DispatchRoot,
}

impl DispatchAuthority for DispatchContext {
    fn vela_dispatch_root(&self) -> &DispatchRoot {
        &self.root
    }
}

#[methods]
impl DispatchContext {
    pub fn marker(&self) -> i64 {
        self.marker
    }
}

#[export(path = "bench::scalar")]
pub fn scalar(value: i64) -> i64 {
    value + 1
}

#[export(path = "bench::read_player")]
pub fn read_player(player: &Player) -> i64 {
    player.value
}

#[export(path = "bench::write_player")]
pub fn write_player(player: &mut Player) -> i64 {
    player.value += 1;
    player.value
}

#[export(path = "bench::round_trip")]
pub fn round_trip(context: &mut NativeCallContext<'_, '_>, value: i64) -> VmResult<i64> {
    let child = context.call("child", CallArgs::new().with(value))?;
    black_box(child);
    Ok(value + 1)
}

#[replaceable(
    path = "host::bench::replaceable_scalar",
    authority = "context",
    index = 0
)]
pub fn replaceable_scalar(context: &DispatchContext, value: i64) -> VmResult<i64> {
    Ok(context.marker + value)
}

#[replaceable(
    path = "host::bench::replaceable_other",
    authority = "context",
    index = 1
)]
pub fn replaceable_other(context: &DispatchContext, value: i64) -> VmResult<i64> {
    Ok(context.marker + value)
}

fn main() -> Result<(), Box<dyn Error>> {
    let iterations = match std::env::args().nth(1).as_deref() {
        Some("--quick") => QUICK_ITERATIONS,
        Some("--stable") => STABLE_ITERATIONS,
        _ => DEFAULT_ITERATIONS,
    };
    println!("vela_engine_interop iterations={iterations}");

    let engine = Engine::builder()
        .register_host_type::<Player>()
        .register_host_type::<DispatchContext>()
        .register_exports(vela_export_bundle_scalar())
        .register_exports(vela_export_bundle_read_player())
        .register_exports(vela_export_bundle_write_player())
        .register_exports(vela_export_bundle_round_trip())
        .capability(Capability::HostRead)
        .capability(Capability::HostWrite)
        .build()?;
    let program = engine.compile_source(SOURCE)?;
    let generated_schema = generated_schema(program.binding_schema(), "generated_target")?;

    let mut scalar_runtime = Runtime::new(engine.clone(), engine.compile_source(SOURCE)?)?;
    let mut shared_runtime = Runtime::new(engine.clone(), engine.compile_source(SOURCE)?)?;
    let mut exclusive_runtime = Runtime::new(engine.clone(), engine.compile_source(SOURCE)?)?;
    let mut round_trip_runtime = Runtime::new(engine.clone(), engine.compile_source(SOURCE)?)?;
    let mut generated_runtime = Runtime::new(engine, program)?;
    let mut generated = RootBinding::bind(&mut generated_runtime, generated_schema)?;
    let generated_target = BindingCallable::new(generated_schema, 0);
    let mut shared_player = Player { value: 41 };
    let mut exclusive_player = Player { value: 0 };
    let fallback_controller = DispatchController::new(vec![
        vela_replaceable_slot_replaceable_scalar(),
        vela_replaceable_slot_replaceable_other(),
    ])?;
    let fallback_context = DispatchContext {
        marker: 1,
        root: DispatchRoot::pin(&fallback_controller),
    };
    let dispatch_engine = Engine::builder()
        .register_host_type::<DispatchContext>()
        .capability(Capability::HostRead)
        .build()?;
    let dispatch_program = dispatch_engine.compile_source(DISPATCH_SOURCE)?;
    let dispatch_runtime = Arc::new(Mutex::new(Runtime::new(dispatch_engine, dispatch_program)?));
    let dispatch_controller = DispatchController::new(vec![
        vela_replaceable_slot_replaceable_scalar(),
        vela_replaceable_slot_replaceable_other(),
    ])?;
    let dispatch_candidate = dispatch_controller.stage_current(&dispatch_runtime)?;
    dispatch_controller
        .activate(dispatch_candidate)
        .expect("activate dispatch candidate");
    let active_context = DispatchContext {
        marker: 1,
        root: DispatchRoot::pin(&dispatch_controller),
    };

    report("direct_rust_scalar", iterations, || Ok(scalar(41)))?;
    report("vela_to_rust_scalar", iterations, || {
        call_i64(
            &mut scalar_runtime,
            "scalar_entry",
            CallArgs::new().with(41_i64),
        )
    })?;
    report("vela_to_rust_shared_host", iterations, || {
        call_i64(
            &mut shared_runtime,
            "shared_entry",
            CallArgs::new().with_host_mut("player", &mut shared_player),
        )
    })?;
    report("vela_to_rust_exclusive_host", iterations, || {
        call_i64(
            &mut exclusive_runtime,
            "exclusive_entry",
            CallArgs::new().with_host_mut("player", &mut exclusive_player),
        )
    })?;
    report("rust_to_vela_generated_root", iterations, || {
        Ok(generated.call::<i64, _>(&generated_target, (41_i64,))?)
    })?;
    report("vela_rust_vela_round_trip", iterations, || {
        call_i64(
            &mut round_trip_runtime,
            "round_trip_entry",
            CallArgs::new().with(41_i64),
        )
    })?;
    report("replaceable_empty_slot_fallback", iterations, || {
        Ok(replaceable_scalar(&fallback_context, 41)?)
    })?;
    report("replaceable_local_override_hit", iterations, || {
        Ok(replaceable_scalar(&active_context, 41)?)
    })?;
    report(
        "replaceable_partial_stage_activate_first_call",
        iterations,
        || {
            let candidate = dispatch_controller.stage_current(&dispatch_runtime)?;
            dispatch_controller
                .activate(candidate)
                .expect("activate dispatch candidate");
            let context = DispatchContext {
                marker: 1,
                root: DispatchRoot::pin(&dispatch_controller),
            };
            Ok(replaceable_scalar(&context, 41)?)
        },
    )?;
    Ok(())
}

fn generated_schema(
    schema: &RustBindingSchema,
    path: &str,
) -> Result<&'static BindingSchemaSpec, Box<dyn Error>> {
    let callable = schema
        .callables()
        .find(|callable| callable.public_path == path)
        .ok_or("generated benchmark target is missing")?;
    let identity = match callable.identity {
        RustBindingCallableIdentity::Function(function) => {
            BindingCallableIdentitySpec::Function(function.get())
        }
        RustBindingCallableIdentity::Method { owner, method } => {
            BindingCallableIdentitySpec::Method {
                owner: owner.get(),
                method: method.get(),
            }
        }
    };
    let public_path = Box::leak(callable.public_path.clone().into_boxed_str());
    let callables = Box::leak(Box::new([BindingCallableSpec {
        public_path,
        identity,
        executable: callable.executable.get(),
        contract_fingerprint: callable.contract_fingerprint,
        effect_bits: callable.effects.bits(),
        source: callable.source,
    }]));
    Ok(Box::leak(Box::new(BindingSchemaSpec {
        version: schema.version(),
        checksum: schema.checksum(),
        types: &[],
        callables,
    })))
}

fn call_i64(
    runtime: &mut Runtime,
    target: &str,
    args: CallArgs<'_>,
) -> Result<i64, Box<dyn Error>> {
    let value = runtime.call(target, args, CallOptions::unbounded())?;
    let owned = runtime.value_to_owned(&value)?;
    Ok(<i64 as vela_engine::args::FromScriptArg>::from_script_arg(
        &owned,
    )?)
}

fn report(
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
        samples.push(started.elapsed().as_nanos());
        checksum = checksum.wrapping_add(sample_checksum);
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
