use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use vela_common::CapabilitySet;
use vela_def::FunctionId;
use vela_engine::engine::Engine;
use vela_engine::native::{EffectSet, FunctionAccess, NativeFunctionDesc, TypeHint};
use vela_engine::runtime::{CallArgs, CallOptions};
use vela_engine::service::{Service, ServicePatch};
use vela_engine::task::{
    ScopedTask, ScopedTaskCompletion, ScopedTaskHost, ScopedTaskOutcome, TaskAdmissionError,
    TaskContinuationOutcome, TaskGeneration, TaskScope,
};
use vela_macros::{service, service_domain};
use vela_vm::owned_value::OwnedValue;
use vela_vm::value::Value;

const OLD_SOURCE: &str = r#"
async fn repair(value: i64) -> i64 {
    let fetched = test_io::pending(value).await;
    let default = service::base::grant(fetched);
    return service::pinned::audit::record(default);
}

fn admit_repair(value: i64) {
    task::spawn_scoped_then(repair(value), finish_repair);
}

fn finish_repair(outcome: Result<i64, task::Error>, turn: i64) {
    let value = outcome.unwrap_or(-1);
    test_io::observe(service::pinned::audit::record(value), turn);
}

#[service_impl(detached_test::inventory)]
impl InventoryPatch {
    fn grant(value: i64) -> i64 {
        admit_repair(value);
        return value * 10;
    }
}
"#;

const NEW_SOURCE: &str = r#"
async fn repair(value: i64) -> i64 {
    let fetched = test_io::pending(value).await;
    let default = service::base::grant(fetched);
    return service::pinned::audit::record(default);
}

fn admit_repair(value: i64) {
    task::spawn_scoped_then(repair(value), finish_repair);
}

fn finish_repair(outcome: Result<i64, task::Error>, turn: i64) {
    let value = outcome.unwrap_or(-1);
    test_io::observe(service::pinned::audit::record(value), turn);
}

#[service_impl(detached_test::inventory)]
impl InventoryPatch {
    fn grant(value: i64) -> i64 {
        admit_repair(value);
        return value * 20;
    }
}

#[service_impl(detached_test::audit)]
impl AuditPatch {
    fn record(value: i64) -> i64 {
        return value + 1000;
    }
}
"#;

#[service(path = "detached_test::inventory")]
pub trait InventoryService: Send + Sync {
    fn grant(&self, value: i64) -> i64;
}

struct RustInventory;

impl InventoryService for RustInventory {
    fn grant(&self, value: i64) -> i64 {
        value + 1
    }
}

#[service(path = "detached_test::audit")]
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
pub struct DetachedServices {
    pub inventory: Service<dyn InventoryService>,
    pub audit: Service<dyn AuditService>,
}

#[derive(Default)]
struct RecordingTaskHost {
    tasks: Mutex<Vec<ScopedTask>>,
}

impl ScopedTaskHost for RecordingTaskHost {
    fn admit(&self, task: ScopedTask) -> Result<(), TaskAdmissionError> {
        self.tasks.lock().expect("task host lock").push(task);
        Ok(())
    }
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

fn app(
    host: Arc<RecordingTaskHost>,
    observations: Arc<Mutex<Vec<Vec<OwnedValue>>>>,
) -> DetachedServicesApp {
    DetachedServices::builder(
        Engine::builder()
            .capabilities(CapabilitySet::all())
            .register_async_fn(
                NativeFunctionDesc::new("test_io::pending", FunctionId::new(0xD37A_0001))
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
                NativeFunctionDesc::new("test_io::observe", FunctionId::new(0xD37A_0002))
                    .param("value", TypeHint::i64())
                    .param("turn", TypeHint::i64())
                    .returns(TypeHint::unit())
                    .effects(EffectSet::host_write())
                    .access(FunctionAccess::public()),
                move |args| {
                    observations
                        .lock()
                        .expect("continuation observation lock")
                        .push(args.to_vec());
                    Ok(OwnedValue::Unit)
                },
            ),
    )
    .task_scope(TaskScope::new(host, crate::support::task_policy()))
    .emergency_patch_effect_ceiling(crate::support::emergency_patch_effect_ceiling())
    .inventory(RustInventory)
    .audit(RustAudit)
    .build()
    .expect("detached Service application")
}

fn take_task(host: &RecordingTaskHost) -> ScopedTask {
    host.tasks.lock().expect("task host lock").remove(0)
}

fn complete(task: &mut ScopedTask) -> ScopedTaskCompletion {
    let mut context = Context::from_waker(std::task::Waker::noop());
    let Poll::Ready(completion) = task.poll_completion(&mut context) else {
        panic!("detached worker should complete on its second poll");
    };
    completion
}

fn completed_i64(completion: &ScopedTaskCompletion) -> i64 {
    let ScopedTaskOutcome::Completed(image) = completion.outcome() else {
        panic!("detached worker should succeed: {:?}", completion.outcome());
    };
    let mut heap = vela_vm::heap::ScriptHeap::new();
    let mut budget = vela_vm::budget::ExecutionBudget::new(1000, 64 * 1024, 16);
    let roots = image
        .import_into(&mut heap, &mut budget)
        .expect("host imports detached outcome");
    match roots.as_slice() {
        [Value::I64(value)] => *value,
        values => panic!("unexpected detached outcome: {values:?}"),
    }
}

#[test]
fn detached_service_worker_pins_origin_generation_across_reload() {
    let host = Arc::new(RecordingTaskHost::default());
    let observations = Arc::new(Mutex::new(Vec::new()));
    let app = app(Arc::clone(&host), Arc::clone(&observations));
    let revision = app.patches().revision().expect("initial patch revision");
    app.patches()
        .apply(ServicePatch::against(&revision).put("repair.vela", OLD_SOURCE))
        .expect("activate old detached patch");
    let old = app.domain().pin();
    assert_eq!(old.inventory().grant(5), 50);
    let mut old_task = take_task(&host);
    assert_eq!(
        old_task.metadata().generation,
        TaskGeneration::Service {
            executable: old.artifact().expect("old artifact").generation(),
            service_set: old.service_set_id(),
            service_generation: old.generation_id(),
        }
    );
    let mut context = Context::from_waker(std::task::Waker::noop());
    assert!(old_task.poll(&mut context).is_pending());

    let revision = app.patches().revision().expect("old patch revision");
    app.patches()
        .apply(ServicePatch::against(&revision).put("repair.vela", NEW_SOURCE))
        .expect("activate new detached patch");
    let new = app.domain().pin();
    assert_ne!(new.generation_id(), old.generation_id());

    let old_completion = complete(&mut old_task);
    assert_eq!(completed_i64(&old_completion), 106);
    drop(old_task);
    assert!(matches!(
        old_completion.resume(
            CallArgs::new().with_value("turn", 7_i64),
            CallOptions::unbounded(),
        ),
        TaskContinuationOutcome::Completed
    ));
    assert_eq!(new.inventory().grant(5), 100);
    let mut new_task = take_task(&host);
    assert!(new_task.poll(&mut context).is_pending());
    let new_completion = complete(&mut new_task);
    assert_eq!(completed_i64(&new_completion), 1006);
    drop(new_task);
    assert!(matches!(
        new_completion.resume(
            CallArgs::new().with_value("turn", 8_i64),
            CallOptions::unbounded(),
        ),
        TaskContinuationOutcome::Completed
    ));
    assert_eq!(
        observations
            .lock()
            .expect("continuation observation lock")
            .as_slice(),
        [
            vec![OwnedValue::i64(206), OwnedValue::i64(7)],
            vec![OwnedValue::i64(2006), OwnedValue::i64(8)],
        ]
    );
}
