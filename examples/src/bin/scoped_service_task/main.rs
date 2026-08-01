use std::error::Error;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use vela_common::CapabilitySet;
use vela_def::FunctionId;
use vela_engine::engine::Engine;
use vela_engine::native::{EffectSet, FunctionAccess, NativeFunctionDesc, TypeHint};
use vela_engine::runtime::{CallArgs, CallOptions};
use vela_engine::service::{Service, ServicePatch};
use vela_engine::task::TaskContinuationOutcome;
use vela_macros::{service, service_domain};
use vela_vm::owned_value::OwnedValue;

const PATCH: &str = include_str!("scoped_service_task.vela");

#[service(path = "scoped_example::repair")]
pub trait RepairService: Send + Sync {
    fn schedule(&self, value: i64) -> i64;
}

struct RustRepair;

impl RepairService for RustRepair {
    fn schedule(&self, value: i64) -> i64 {
        value + 1
    }
}

#[service(path = "scoped_example::audit")]
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
pub struct ScopedServices {
    pub repair: Service<dyn RepairService>,
    pub audit: Service<dyn AuditService>,
}

struct PendingValue {
    value: OwnedValue,
    yielded: bool,
}

impl std::future::Future for PendingValue {
    type Output = vela_vm::error::VmResult<OwnedValue>;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        if self.yielded {
            Poll::Ready(Ok(self.value.clone()))
        } else {
            self.yielded = true;
            context.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let observations = Arc::new(Mutex::new(Vec::new()));
    let native_observations = observations.clone();
    let adapter = vela_examples::service_tasks::ActorTaskAdapter::new();
    let app = ScopedServices::builder(
        Engine::builder()
            .capabilities(CapabilitySet::all())
            .register_async_fn(
                NativeFunctionDesc::new("example_io::pending", FunctionId::new(0x5C0A_0001))
                    .param("value", TypeHint::i64())
                    .returns(TypeHint::i64())
                    .effects(EffectSet::io_read())
                    .access(FunctionAccess::public()),
                |args| {
                    Box::pin(PendingValue {
                        value: args.first().cloned().unwrap_or(OwnedValue::Unit),
                        yielded: false,
                    })
                },
            )
            .register_native_fn(
                NativeFunctionDesc::new("example_io::observe", FunctionId::new(0x5C0A_0002))
                    .param("value", TypeHint::i64())
                    .param("turn", TypeHint::i64())
                    .returns(TypeHint::unit())
                    .effects(EffectSet::host_write())
                    .access(FunctionAccess::public()),
                move |args| {
                    native_observations
                        .lock()
                        .expect("example observation lock")
                        .push(args.to_vec());
                    Ok(OwnedValue::Unit)
                },
            ),
    )
    .task_scope(adapter.task_scope())
    .emergency_patch_effect_ceiling(vela_examples::service_tasks::emergency_patch_effect_ceiling())
    .repair(RustRepair)
    .audit(RustAudit)
    .build()?;
    let revision = app.patches().revision()?;
    app.patches()
        .apply(ServicePatch::against(&revision).put("repair.vela", PATCH))?;

    let root = app.domain().pin();
    let immediate = root.repair().schedule(5);
    if immediate != 50 {
        return Err(format!("expected immediate patched result 50, got {immediate}").into());
    }

    let continuation = adapter.resume_one_timeout(
        std::time::Duration::from_secs(2),
        CallArgs::new().with_value("turn", 7_i64),
        CallOptions::unbounded(),
    );
    if !matches!(continuation, Some(TaskContinuationOutcome::Completed)) {
        return Err(format!("continuation did not complete: {continuation:?}").into());
    }
    let observed = observations.lock().expect("example observation lock");
    if observed.as_slice() != [vec![OwnedValue::i64(106), OwnedValue::i64(7)]] {
        return Err(format!("unexpected safe-point observation: {observed:?}").into());
    }
    let metrics = adapter.metrics();
    println!(
        "immediate={immediate} continuation=106 turn=7 task_id_count={} pool_hits={} pool_misses={}",
        metrics.admitted, metrics.runtime_pool_hits, metrics.runtime_pool_misses
    );
    drop(observed);
    adapter.shutdown();
    Ok(())
}
