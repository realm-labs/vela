use std::alloc::System;
use std::collections::BTreeMap;
use std::error::Error;
use std::hint::black_box;
use std::time::Instant;

use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};
use vela_common::{HostObjectId, SourceId};
use vela_engine::args::FromScriptArg;
use vela_engine::context::NativeCallContext;
use vela_engine::engine::Engine;
use vela_engine::interop::{
    HostLeaseParameterPlan, HostParamLeaseRequest, PreparedHostLeasePlan,
    preflight_host_parameter_leases,
};
use vela_engine::permission::Capability;
use vela_engine::runtime::{CallArgs, CallOptions, Runtime};
use vela_engine::service::{Service, ServiceRuntimeBinding, ServiceSourceManifest};
use vela_hir::source_ingestion::build_single_source;
use vela_host::lease::HostLeaseKind;
use vela_host::path::HostRef;
use vela_macros::{ScriptHost, ScriptReflect, export, methods, service, service_domain};
use vela_vm::error::VmResult;
use vela_vm::owned_value::OwnedValue;

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

const QUICK_ITERATIONS: usize = 1_000;
const STABLE_ITERATIONS: usize = 100_000;
const WARMUP_ITERATIONS: usize = 100;

const SOURCE: &str = r#"
fn static_field_read_write(host: BoundaryHost) {
    host.value += 1;
    return host.value;
}

fn registered_method_call(host: BoundaryHost) {
    return host.increment(1);
}

fn nested_reborrow(host: BoundaryHost) {
    return bench::nested_reborrow(host);
}

fn nested_reborrow_child(host: BoundaryHost) {
    host.value += 1;
    return host.value;
}

fn borrowed_return_release(actor: BoundaryHost) {
    let child = bench::borrowed_child(actor);
    host::release(child);
    return actor.touch();
}

fn host_backed_bulk_collection(host: BoundaryHost) {
    return host.sum_values();
}
"#;

const SERVICE_SOURCE: &str = r#"
#[service_impl(bench::boundary_default)]
impl BoundaryPatch {
    fn apply(host) {
        return service::base::apply(host);
    }
}
"#;

#[derive(Debug, ScriptHost, ScriptReflect)]
#[vela(path = "bench::BoundaryChild")]
pub struct BoundaryChild {
    #[vela(get, set)]
    value: i64,
}

#[methods(path = "bench::BoundaryChild")]
impl BoundaryChild {
    pub fn increment(&mut self, amount: i64) -> i64 {
        self.value += amount;
        self.value
    }
}

#[derive(ScriptHost, ScriptReflect)]
#[vela(path = "bench::BoundaryHost")]
pub struct BoundaryHost {
    #[vela(get, set)]
    value: i64,
    #[vela(skip)]
    child: BoundaryChild,
    #[vela(skip)]
    values: BTreeMap<i64, i64>,
    #[vela(skip)]
    touches: i64,
}

#[methods(path = "bench::BoundaryHost")]
impl BoundaryHost {
    pub fn increment(&mut self, amount: i64) -> i64 {
        self.value += amount;
        self.value
    }

    pub fn child_mut(&mut self) -> &mut BoundaryChild {
        &mut self.child
    }

    pub fn touch(&mut self) -> i64 {
        self.touches += 1;
        self.touches
    }

    pub fn sum_values(&self) -> i64 {
        self.values.values().copied().sum()
    }
}

#[export(path = "bench::nested_reborrow")]
pub fn nested_reborrow(
    context: &mut NativeCallContext<'_, '_>,
    host: &mut BoundaryHost,
) -> VmResult<i64> {
    let mut args = CallArgs::new();
    args.push_positional_host_mut(host);
    let _ = context.call("nested_reborrow_child", args)?;
    Ok(host.value)
}

#[export(path = "bench::borrowed_child")]
pub fn borrowed_child(host: &mut BoundaryHost) -> &mut BoundaryChild {
    &mut host.child
}

#[export(path = "bench::shared_pair")]
pub fn shared_pair(first: &BoundaryHost, second: &BoundaryHost) -> i64 {
    first.value + second.value
}

#[export(path = "bench::exclusive_pair")]
pub fn exclusive_pair(first: &mut BoundaryHost, second: &mut BoundaryHost) -> i64 {
    first.value += 1;
    second.value += 1;
    first.value + second.value
}

#[service(path = "bench::boundary_default")]
pub trait BoundaryDefaultService: Send + Sync {
    fn apply(&self, host: &mut BoundaryHost) -> i64;
}

struct RustBoundaryDefaultService;

impl BoundaryDefaultService for RustBoundaryDefaultService {
    fn apply(&self, host: &mut BoundaryHost) -> i64 {
        host.value += 1;
        host.value
    }
}

#[service_domain(context = BoundaryHost)]
pub struct BoundaryServices {
    pub boundary: Service<dyn BoundaryDefaultService>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let iterations = if std::env::args().any(|argument| argument == "--stable") {
        STABLE_ITERATIONS
    } else {
        QUICK_ITERATIONS
    };
    println!("suite=service_boundary_baseline iterations={iterations} warmup={WARMUP_ITERATIONS}");

    let mut runtime = boundary_runtime()?;
    let app = BoundaryServices::builder(
        Engine::builder()
            .capability(Capability::HostWrite)
            .register_type::<BoundaryHost>(),
    )
    .boundary(RustBoundaryDefaultService)
    .build()?;
    let (service_engine, services) = app.into_parts();
    let mut host = BoundaryHost {
        value: 1,
        child: BoundaryChild { value: 2 },
        values: BTreeMap::from([(1, 3), (2, 5), (3, 8), (4, 13)]),
        touches: 0,
    };

    let default_service = RustBoundaryDefaultService;
    report("direct_rust_concrete", iterations, || {
        Ok(default_service.apply(black_box(&mut host)) as u64)
    })?;
    let default_service: &dyn BoundaryDefaultService = &default_service;
    report("direct_rust_trait_dispatch", iterations, || {
        Ok(black_box(default_service).apply(black_box(&mut host)) as u64)
    })?;
    let rust_root = services.pin();
    report("generated_rust_default", iterations, || {
        Ok(black_box(rust_root.boundary()).apply(black_box(&mut host)) as u64)
    })?;
    let service_sources = build_single_source(SourceId::new(1), SERVICE_SOURCE)
        .map_err(|error| format!("{error:?}"))?;
    let manifest = ServiceSourceManifest::link(service_sources.graph(), services.schema())?;
    let artifact =
        service_engine.link_compiled_program(service_engine.compile_source(SERVICE_SOURCE)?)?;
    let update = manifest.bind_artifact(artifact)?;
    let candidate = services.stage_snapshot(
        &rust_root,
        update,
        ServiceRuntimeBinding::for_engine(service_engine.clone()),
        CallOptions::unbounded(),
    )?;
    services.activate_if_current(candidate)?;
    let vela_root = services.pin();
    report("generated_active_vela", iterations, || {
        Ok(black_box(vela_root.boundary()).apply(black_box(&mut host)) as u64)
    })?;

    let root = HostRef::new(BoundaryHost::vela_host_type_id(), HostObjectId::new(1), 1);
    report("host_ref_alias_copy", iterations, || {
        Ok(host_ref_checksum(black_box(root)))
    })?;

    report("static_field_read_write", iterations, || {
        call_host(&mut runtime, "static_field_read_write", &mut host)
    })?;
    report("registered_method_call", iterations, || {
        call_host(&mut runtime, "registered_method_call", &mut host)
    })?;

    let shared_requests = preflight_requests(
        vela_callable_contract_shared_pair(),
        [root, root],
        HostLeaseKind::Shared,
    )?;
    report("shared_argument_preflight", iterations, || {
        let requests = preflight_host_parameter_leases(black_box(&shared_requests))?;
        Ok(request_checksum(&requests))
    })?;

    let exclusive_requests = preflight_requests(
        vela_callable_contract_exclusive_pair(),
        [
            root,
            HostRef::new(BoundaryHost::vela_host_type_id(), HostObjectId::new(2), 1),
        ],
        HostLeaseKind::Exclusive,
    )?;
    report("exclusive_argument_preflight", iterations, || {
        let requests = preflight_host_parameter_leases(black_box(&exclusive_requests))?;
        Ok(request_checksum(&requests))
    })?;

    let shared_args = [OwnedValue::HostRef(root), OwnedValue::HostRef(root)];
    let shared_plan = PreparedHostLeasePlan::new(
        vela_callable_contract_shared_pair(),
        2,
        [
            HostLeaseParameterPlan::argument(
                0,
                0,
                BoundaryHost::vela_host_type_id(),
                HostLeaseKind::Shared,
            ),
            HostLeaseParameterPlan::argument(
                1,
                1,
                BoundaryHost::vela_host_type_id(),
                HostLeaseKind::Shared,
            ),
        ],
    );
    report("prepared_shared_argument_preflight", iterations, || {
        let requests = shared_plan.prepare(black_box(&shared_args))?;
        Ok(request_checksum(&requests))
    })?;

    let exclusive_args = [
        OwnedValue::HostRef(root),
        OwnedValue::HostRef(HostRef::new(
            BoundaryHost::vela_host_type_id(),
            HostObjectId::new(2),
            1,
        )),
    ];
    let exclusive_plan = PreparedHostLeasePlan::new(
        vela_callable_contract_exclusive_pair(),
        2,
        [
            HostLeaseParameterPlan::argument(
                0,
                0,
                BoundaryHost::vela_host_type_id(),
                HostLeaseKind::Exclusive,
            ),
            HostLeaseParameterPlan::argument(
                1,
                1,
                BoundaryHost::vela_host_type_id(),
                HostLeaseKind::Exclusive,
            ),
        ],
    );
    report("prepared_exclusive_argument_preflight", iterations, || {
        let requests = exclusive_plan.prepare(black_box(&exclusive_args))?;
        Ok(request_checksum(&requests))
    })?;

    report("nested_same_session_reborrow", iterations, || {
        call_host(&mut runtime, "nested_reborrow", &mut host)
    })?;
    report("borrowed_return_release", iterations, || {
        call_host(&mut runtime, "borrowed_return_release", &mut host)
    })?;
    report("host_backed_bulk_collection", iterations, || {
        call_host(&mut runtime, "host_backed_bulk_collection", &mut host)
    })?;

    Ok(())
}

fn boundary_runtime() -> Result<Runtime, Box<dyn Error>> {
    let engine = Engine::builder()
        .capability(Capability::HostRead)
        .capability(Capability::HostWrite)
        .register_type::<BoundaryChild>()
        .register_type::<BoundaryHost>()
        .register_exports(BoundaryChild::vela_inherent_exports())
        .register_exports(BoundaryHost::vela_inherent_exports())
        .register_exports(vela_export_bundle_nested_reborrow())
        .register_exports(vela_export_bundle_borrowed_child())
        .register_exports(vela_export_bundle_shared_pair())
        .register_exports(vela_export_bundle_exclusive_pair())
        .build()?;
    let program = engine.compile_source(SOURCE)?;
    Ok(Runtime::new(engine, program)?)
}

fn call_host(
    runtime: &mut Runtime,
    function: &str,
    host: &mut BoundaryHost,
) -> Result<u64, Box<dyn Error>> {
    let mut args = CallArgs::new();
    args.push_positional_host_mut(host);
    let value = runtime.call(function, args, CallOptions::unbounded())?;
    let value = i64::from_script_arg(&runtime.value_to_owned(&value)?)?;
    Ok(value as u64)
}

fn preflight_requests(
    contract: vela_engine::interop::CallableContract,
    roots: [HostRef; 2],
    mode: HostLeaseKind,
) -> Result<Vec<HostParamLeaseRequest>, Box<dyn Error>> {
    roots
        .into_iter()
        .enumerate()
        .map(|(index, root)| {
            HostParamLeaseRequest::from_argument(
                &contract,
                index,
                index,
                BoundaryHost::vela_host_type_id(),
                mode,
                &OwnedValue::HostRef(root),
            )
            .map_err(Into::into)
        })
        .collect()
}

fn report(
    name: &str,
    iterations: usize,
    mut operation: impl FnMut() -> Result<u64, Box<dyn Error>>,
) -> Result<(), Box<dyn Error>> {
    for _ in 0..WARMUP_ITERATIONS {
        black_box(operation()?);
    }

    let region = Region::new(GLOBAL);
    let started = Instant::now();
    let mut checksum = 0_u64;
    for _ in 0..iterations {
        checksum = checksum.rotate_left(7) ^ black_box(operation()?);
    }
    let elapsed = started.elapsed();
    let allocation = region.change();
    let calls_per_second = iterations as f64 / elapsed.as_secs_f64();
    println!(
        "boundary_result name={name} ns_per_call={} calls_per_second={calls_per_second:.3} allocation_count={} allocations_per_call={:.3} allocated_bytes={} allocated_bytes_per_call={:.3} deallocated_bytes={} checksum={checksum}",
        elapsed.as_nanos() / iterations as u128,
        allocation.allocations,
        allocation.allocations as f64 / iterations as f64,
        allocation.bytes_allocated,
        allocation.bytes_allocated as f64 / iterations as f64,
        allocation.bytes_deallocated,
    );
    Ok(())
}

fn host_ref_checksum(root: HostRef) -> u64 {
    root.type_id.get() ^ root.object_id.get() ^ u64::from(root.generation)
}

fn request_checksum(requests: &[(HostRef, HostLeaseKind)]) -> u64 {
    requests.iter().fold(0_u64, |checksum, (root, mode)| {
        checksum
            ^ root.object_id.get()
            ^ match mode {
                HostLeaseKind::Shared => 1,
                HostLeaseKind::Exclusive => 2,
            }
    })
}
