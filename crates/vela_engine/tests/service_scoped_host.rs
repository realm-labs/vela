use std::cell::Cell;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

use vela_common::{CallableAsyncness, HostMethodId, HostTypeId, ReceiverCapability, SourceId};
use vela_def::{FieldId, TypeId};
use vela_engine::engine::Engine;
use vela_engine::host_call::{decode_host_call_arg, encode_host_call_return};
use vela_engine::host_type::HostTypeSpec;
use vela_engine::interop::VelaValueBoundary;
use vela_engine::method::NativeMethodDesc;
use vela_engine::native::{EffectSet, TypeHint};
use vela_engine::permission::{Capability, CapabilitySet};
use vela_engine::runtime::CallOptions;
use vela_engine::schema::ScriptHostSchema;
use vela_engine::service::{Service, ServiceRuntimeBinding, ServiceSourceManifest};
use vela_hir::source_ingestion::build_single_source;
use vela_host::call_value::HostCallValue;
use vela_host::error::{HostError, HostErrorKind, HostResult};
use vela_host::object::ScriptHostObject;
use vela_host::target::HostTargetInstance;
use vela_host::value::HostValue;
use vela_macros::{Value, service, service_domain};
use vela_reflect::registry::{FieldDesc, TypeDesc, TypeKey};

const CONTEXT_TYPE_ID: u128 = 0x51C0_0E01;
const CONTEXT_HOST_TYPE_ID: u64 = 0x51C0;
const BUMP_METHOD_ID: u128 = 0x51C0_0E02;
const BUMP_ASYNC_METHOD_ID: u128 = 0x51C0_0E03;
const PROCESS_METHOD_ID: u128 = 0x51C0_0E04;
const PROCESS_ASYNC_METHOD_ID: u128 = 0x51C0_0E05;

#[derive(Clone, Debug, Eq, PartialEq, Value)]
#[vela(path = "scoped_test::Decision")]
pub enum Decision {
    Accepted { code: i64 },
    Deferred,
}

#[derive(Clone, Debug, Eq, PartialEq, Value)]
#[vela(path = "scoped_test::Payload")]
pub struct Payload {
    pub amount: i64,
    pub values: Vec<i64>,
    pub decision: Decision,
}

#[derive(Clone, Debug, Eq, PartialEq, Value)]
#[vela(path = "scoped_test::Reply")]
pub struct Reply {
    pub total: i64,
    pub values: Vec<i64>,
    pub decision: Decision,
}

pub struct BorrowedContext<'ctx> {
    value: &'ctx mut i64,
    not_sync: Cell<()>,
}

impl ScriptHostSchema for BorrowedContext<'_> {
    fn script_host_type_desc() -> TypeDesc {
        context_type_desc()
    }
}

impl ScriptHostObject for BorrowedContext<'_> {
    fn host_type_id(&self) -> HostTypeId {
        HostTypeId::new(CONTEXT_HOST_TYPE_ID)
    }

    fn read_resolved_host(
        &self,
        _access: vela_host::resolved::ResolvedHostAccess,
        _target: HostTargetInstance<'_>,
    ) -> HostResult<HostValue> {
        Ok(HostValue::i64(*self.value))
    }

    fn write_resolved_host(
        &mut self,
        _access: vela_host::resolved::ResolvedHostAccess,
        _target: HostTargetInstance<'_>,
        value: HostValue,
    ) -> HostResult<()> {
        let HostValue::Scalar(vela_common::ScalarValue::I64(value)) = value else {
            return Err(HostError {
                kind: HostErrorKind::InvalidArgument {
                    expected: "i64 BorrowedContext value",
                },
                source_span: None,
            });
        };
        *self.value = value;
        Ok(())
    }

    fn call_resolved_host(
        &mut self,
        _access: vela_host::resolved::ResolvedHostAccess,
        _target: HostTargetInstance<'_>,
        method: HostMethodId,
        args: &[HostCallValue],
    ) -> HostResult<HostCallValue> {
        if method == HostMethodId::new(PROCESS_METHOD_ID) {
            let payload: Payload = decode_host_call_arg(required_payload(args.first())?)?;
            let adjustment = rust_payload_adjustment(&payload);
            *self.value += adjustment;
            return encode_host_call_return(reply(*self.value, adjustment));
        }
        if method != HostMethodId::new(BUMP_METHOD_ID) {
            return Err(invalid_method());
        }
        let delta = host_call_i64(args.first())?;
        *self.value += delta;
        Ok(HostCallValue::i64(*self.value))
    }

    fn call_async_host_exclusive<'call>(
        &'call mut self,
        method: HostMethodId,
        args: Vec<HostCallValue>,
    ) -> vela_host::object::HostCallFuture<'call> {
        if method == HostMethodId::new(PROCESS_ASYNC_METHOD_ID) {
            return Box::pin(async move {
                HostYieldOnce { pending: true }.await;
                let payload: Payload = decode_host_call_arg(required_payload(args.first())?)?;
                let adjustment = rust_payload_adjustment(&payload);
                *self.value += adjustment;
                encode_host_call_return(reply(*self.value, adjustment))
            });
        }
        if method != HostMethodId::new(BUMP_ASYNC_METHOD_ID) {
            return Box::pin(async { Err(invalid_method()) });
        }
        Box::pin(async move {
            HostYieldOnce { pending: true }.await;
            let delta = host_call_i64(args.first())?;
            *self.value += delta;
            Ok(HostCallValue::i64(*self.value))
        })
    }
}

fn context_type_desc() -> TypeDesc {
    TypeDesc::new(TypeKey::new(
        TypeId::new(CONTEXT_TYPE_ID),
        "scoped_test::BorrowedContext",
    ))
    .host_type(HostTypeId::new(CONTEXT_HOST_TYPE_ID))
    .field(
        FieldDesc::new(FieldId::new(1), "value")
            .type_hint("i64")
            .writable(true),
    )
}

fn context_type_spec() -> HostTypeSpec {
    let owner = context_type_desc().key;
    let bump = NativeMethodDesc::new(owner.clone(), HostMethodId::new(BUMP_METHOD_ID), "bump")
        .receiver(ReceiverCapability::Exclusive)
        .effects(EffectSet::host_write())
        .param("delta", TypeHint::i64())
        .returns(TypeHint::i64());
    let bump_async =
        NativeMethodDesc::new(owner, HostMethodId::new(BUMP_ASYNC_METHOD_ID), "bump_async")
            .receiver(ReceiverCapability::Exclusive)
            .effects(EffectSet::host_write())
            .asyncness(CallableAsyncness::Async)
            .param("delta", TypeHint::i64())
            .returns(TypeHint::i64());
    let process = NativeMethodDesc::new(
        context_type_desc().key,
        HostMethodId::new(PROCESS_METHOD_ID),
        "process",
    )
    .receiver(ReceiverCapability::Exclusive)
    .effects(EffectSet::host_write())
    .param("payload", Payload::vela_type_hint())
    .returns(Reply::vela_type_hint());
    let process_async = NativeMethodDesc::new(
        context_type_desc().key,
        HostMethodId::new(PROCESS_ASYNC_METHOD_ID),
        "process_async",
    )
    .receiver(ReceiverCapability::Exclusive)
    .effects(EffectSet::host_write())
    .asyncness(CallableAsyncness::Async)
    .param("payload", Payload::vela_type_hint())
    .returns(Reply::vela_type_hint());
    HostTypeSpec::new(context_type_desc())
        .erased_method(bump)
        .erased_async_method(bump_async)
        .erased_method(process)
        .erased_async_method(process_async)
}

#[service(path = "scoped_test::handler")]
pub trait ScopedHandlerService: Send + Sync {
    fn handle_sync(&self, context: &mut BorrowedContext<'_>, delta: i64) -> i64;

    async fn handle(&self, context: &mut BorrowedContext<'_>, payload: Payload, delta: i64) -> i64;

    fn call_audit_sync(&self, context: &mut BorrowedContext<'_>, delta: i64) -> i64;

    async fn call_audit_async(&self, context: &mut BorrowedContext<'_>, delta: i64) -> i64;
}

struct RustScopedHandler;

impl ScopedHandlerService for RustScopedHandler {
    fn handle_sync(&self, context: &mut BorrowedContext<'_>, delta: i64) -> i64 {
        *context.value += delta;
        *context.value
    }

    async fn handle(&self, context: &mut BorrowedContext<'_>, payload: Payload, delta: i64) -> i64 {
        *context.value += delta + rust_payload_adjustment(&payload);
        *context.value
    }

    fn call_audit_sync(&self, context: &mut BorrowedContext<'_>, delta: i64) -> i64 {
        *context.value += delta * 100;
        *context.value
    }

    async fn call_audit_async(&self, context: &mut BorrowedContext<'_>, delta: i64) -> i64 {
        *context.value += delta * 100;
        *context.value
    }
}

#[service(path = "scoped_test::audit")]
pub trait ScopedAuditService: Send + Sync {
    fn record(&self, context: &mut BorrowedContext<'_>, delta: i64) -> i64;

    async fn record_async(&self, context: &mut BorrowedContext<'_>, delta: i64) -> i64;
}

struct RustScopedAudit;

impl ScopedAuditService for RustScopedAudit {
    fn record(&self, context: &mut BorrowedContext<'_>, delta: i64) -> i64 {
        *context.value += delta;
        *context.value
    }

    async fn record_async(&self, context: &mut BorrowedContext<'_>, delta: i64) -> i64 {
        HostYieldOnce { pending: true }.await;
        assert_ne!(delta, -1, "intentional scoped audit panic");
        *context.value += delta;
        *context.value
    }
}

#[service_domain]
pub struct ScopedServices {
    pub handler: Service<dyn ScopedHandlerService>,
    pub audit: Service<dyn ScopedAuditService>,
}

const PATCH_SOURCE: &str = r#"
#[service_impl(scoped_test::handler)]
impl HandlerPatch {
    fn handle_sync(context: BorrowedContext, delta: i64) -> i64 {
        context.bump(delta);
        return service::base::handle_sync(context, delta) + 1;
    }

    async fn handle(context: BorrowedContext, payload: Payload, delta: i64) -> i64 {
        let base_value = service::base::handle(context, payload, delta).await;
        let after_sync = context.process(payload);
        let after_async = context.process_async(payload).await;
        return base_value
            + after_sync.total
            + after_sync.values.len()
            + after_async.total
            + after_async.values.len()
            + delta;
    }

    fn call_audit_sync(context: BorrowedContext, delta: i64) -> i64 {
        return service::pinned::audit::record(context, delta);
    }

    async fn call_audit_async(context: BorrowedContext, delta: i64) -> i64 {
        return service::pinned::audit::record_async(context, delta).await;
    }
}
"#;

const AUDIT_PATCH_SOURCE: &str = r#"
#[service_impl(scoped_test::audit)]
impl AuditPatch {
    fn record(context: BorrowedContext, delta: i64) -> i64 {
        return service::base::record(context, delta) + 10;
    }

    async fn record_async(context: BorrowedContext, delta: i64) -> i64 {
        let recorded = service::base::record_async(context, delta).await;
        return recorded + 20;
    }
}
"#;

#[test]
fn async_service_accepts_send_non_sync_non_static_host_context() {
    let app = ScopedServices::builder(
        Engine::builder()
            .capabilities(CapabilitySet::new().with(Capability::HostWrite))
            .install_generated_type::<Reply>()
            .register_host_type(context_type_spec()),
    )
    .task_scope(crate::support::dropping_task_scope())
    .emergency_patch_effect_ceiling(crate::support::emergency_patch_effect_ceiling())
    .handler(RustScopedHandler)
    .audit(RustScopedAudit)
    .build()
    .expect("service application with schema-only Host contract");
    let (engine, services) = app.into_parts();
    let base = services.pin();
    let mut default_value = 3_i64;
    let mut default_context = BorrowedContext {
        value: &mut default_value,
        not_sync: Cell::new(()),
    };
    let default_future = base.handler().handle(&mut default_context, payload(), 4);
    assert_send(&default_future);
    assert_eq!(poll_ready(default_future), 24);
    assert_eq!(*default_context.value, 24);

    let sources = build_single_source(SourceId::new(0x51C0), PATCH_SOURCE).expect("service source");
    let manifest =
        ServiceSourceManifest::link(sources.graph(), services.schema()).expect("service schema");
    let artifact = engine
        .link_compiled_program(engine.compile_source(PATCH_SOURCE).expect("compiled patch"))
        .expect("linked patch");
    let update = manifest
        .bind_artifact(artifact)
        .expect("artifact-bound update");
    let candidate = services
        .stage_snapshot(
            &base,
            update,
            ServiceRuntimeBinding::for_engine(engine.clone()),
            CallOptions::new(100_000, 1024 * 1024, 64),
        )
        .expect("staged scoped Host service");
    services
        .activate_if_current(candidate)
        .expect("activated scoped Host service");

    let mut value = 10_i64;
    let mut context = BorrowedContext {
        value: &mut value,
        not_sync: Cell::new(()),
    };
    let root = services.pin();
    let mut sync_value = 1_i64;
    let mut sync_context = BorrowedContext {
        value: &mut sync_value,
        not_sync: Cell::new(()),
    };
    assert_eq!(root.handler().handle_sync(&mut sync_context, 3), 8);
    assert_eq!(*sync_context.value, 7);

    let mut rust_audit_value = 100_i64;
    let mut rust_audit_context = BorrowedContext {
        value: &mut rust_audit_value,
        not_sync: Cell::new(()),
    };
    assert_eq!(
        root.handler().call_audit_sync(&mut rust_audit_context, 3),
        103
    );
    assert_eq!(
        poll_after_one_pending(root.handler().call_audit_async(&mut rust_audit_context, 4)),
        107
    );

    let mut cancelled_value = 500_i64;
    let mut cancelled_context = BorrowedContext {
        value: &mut cancelled_value,
        not_sync: Cell::new(()),
    };
    poll_pending_then_drop(root.handler().call_audit_async(&mut cancelled_context, 5));
    assert_eq!(
        root.handler().call_audit_sync(&mut cancelled_context, 1),
        501
    );

    let mut panic_value = 600_i64;
    let mut panic_context = BorrowedContext {
        value: &mut panic_value,
        not_sync: Cell::new(()),
    };
    let mut panic_future = root.handler().call_audit_async(&mut panic_context, -1);
    let mut panic_task = Context::from_waker(Waker::noop());
    assert!(matches!(
        panic_future.as_mut().poll(&mut panic_task),
        Poll::Pending
    ));
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = panic_future.as_mut().poll(&mut panic_task);
        }))
        .is_err()
    );
    drop(panic_future);
    assert_eq!(root.handler().call_audit_sync(&mut panic_context, 1), 601);

    let future = root.handler().handle(&mut context, payload(), 7);
    assert_send(&future);
    assert_eq!(poll_after_one_pending(future), 164);
    assert_eq!(*context.value, 68);
    context.not_sync.get();

    let full_source = format!("{PATCH_SOURCE}\n{AUDIT_PATCH_SOURCE}");
    let full_sources = build_single_source(SourceId::new(0x51C1), &full_source)
        .expect("complete cross-Service source");
    let full_manifest = ServiceSourceManifest::link(full_sources.graph(), services.schema())
        .expect("complete cross-Service schema");
    let full_artifact = engine
        .link_compiled_program(
            engine
                .compile_source(&full_source)
                .expect("compiled complete cross-Service patch"),
        )
        .expect("linked complete cross-Service patch");
    let full_update = full_manifest
        .bind_artifact(full_artifact)
        .expect("artifact-bound complete cross-Service update");
    let full_candidate = services
        .stage_snapshot(
            &root,
            full_update,
            ServiceRuntimeBinding::for_engine(engine.clone()),
            CallOptions::new(100_000, 1024 * 1024, 64),
        )
        .expect("staged complete cross-Service patch");
    services
        .activate_if_current(full_candidate)
        .expect("activated complete cross-Service patch");

    let mut old_value = 200_i64;
    let mut old_context = BorrowedContext {
        value: &mut old_value,
        not_sync: Cell::new(()),
    };
    assert_eq!(root.handler().call_audit_sync(&mut old_context, 3), 203);

    let full_root = services.pin();
    let mut patched_value = 300_i64;
    let mut patched_context = BorrowedContext {
        value: &mut patched_value,
        not_sync: Cell::new(()),
    };
    assert_eq!(
        full_root.handler().call_audit_sync(&mut patched_context, 3),
        313
    );
    assert_eq!(*patched_context.value, 303);

    let mut patched_async_value = 400_i64;
    let mut patched_async_context = BorrowedContext {
        value: &mut patched_async_value,
        not_sync: Cell::new(()),
    };
    assert_eq!(
        poll_after_one_pending(
            full_root
                .handler()
                .call_audit_async(&mut patched_async_context, 4)
        ),
        424
    );
    assert_eq!(*patched_async_context.value, 404);
}

struct HostYieldOnce {
    pending: bool,
}

impl Future for HostYieldOnce {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        if self.pending {
            self.pending = false;
            context.waker().wake_by_ref();
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }
}

fn host_call_i64(value: Option<&HostCallValue>) -> HostResult<i64> {
    match value {
        Some(HostCallValue::Scalar(vela_common::ScalarValue::I64(value))) => Ok(*value),
        _ => Err(HostError {
            kind: HostErrorKind::InvalidArgument {
                expected: "one i64 Host method argument",
            },
            source_span: None,
        }),
    }
}

fn payload() -> Payload {
    Payload {
        amount: 5,
        values: vec![2, 3],
        decision: Decision::Accepted { code: 7 },
    }
}

fn rust_payload_adjustment(payload: &Payload) -> i64 {
    let decision = match payload.decision {
        Decision::Accepted { code } => code,
        Decision::Deferred => 0,
    };
    payload.amount + payload.values.iter().sum::<i64>() + decision
}

fn reply(total: i64, adjustment: i64) -> Reply {
    Reply {
        total,
        values: vec![adjustment, total],
        decision: Decision::Accepted { code: total },
    }
}

fn required_payload(value: Option<&HostCallValue>) -> HostResult<&HostCallValue> {
    value.ok_or_else(invalid_host_method_value)
}

fn invalid_host_method_value() -> HostError {
    HostError {
        kind: HostErrorKind::InvalidArgument {
            expected: "scoped_test::Payload Host method value",
        },
        source_span: None,
    }
}

fn invalid_method() -> HostError {
    HostError {
        kind: HostErrorKind::InvalidArgument {
            expected: "registered BorrowedContext method",
        },
        source_span: None,
    }
}

fn poll_after_one_pending<T>(mut future: Pin<Box<dyn Future<Output = T> + Send + '_>>) -> T {
    let mut task = Context::from_waker(Waker::noop());
    assert!(matches!(future.as_mut().poll(&mut task), Poll::Pending));
    match future.as_mut().poll(&mut task) {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("future should complete after one pending poll"),
    }
}

fn poll_pending_then_drop<T>(mut future: Pin<Box<dyn Future<Output = T> + Send + '_>>) {
    let mut task = Context::from_waker(Waker::noop());
    assert!(matches!(future.as_mut().poll(&mut task), Poll::Pending));
}

fn poll_ready<T>(mut future: Pin<Box<dyn Future<Output = T> + Send + '_>>) -> T {
    let mut task = Context::from_waker(Waker::noop());
    match future.as_mut().poll(&mut task) {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("Rust default should complete without suspension"),
    }
}

fn assert_send<T: Send>(_: &T) {}
