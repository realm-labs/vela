use std::cell::Cell;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

use vela_common::{CallableAsyncness, HostMethodId, HostTypeId, ReceiverCapability, SourceId};
use vela_def::{FieldId, TypeId};
use vela_engine::engine::Engine;
use vela_engine::host_type::HostTypeSpec;
use vela_engine::method::NativeMethodDesc;
use vela_engine::native::{EffectSet, TypeHint};
use vela_engine::permission::{Capability, CapabilitySet};
use vela_engine::runtime::CallOptions;
use vela_engine::schema::ScriptHostSchema;
use vela_engine::service::{Service, ServiceRuntimeBinding, ServiceSourceManifest};
use vela_hir::source_ingestion::build_single_source;
use vela_host::error::{HostError, HostErrorKind, HostResult};
use vela_host::object::ScriptHostObject;
use vela_host::target::HostTargetInstance;
use vela_host::value::HostValue;
use vela_macros::{service, service_domain};
use vela_reflect::registry::{FieldDesc, TypeDesc, TypeKey};

const CONTEXT_TYPE_ID: u128 = 0x51C0_0E01;
const CONTEXT_HOST_TYPE_ID: u64 = 0x51C0;
const BUMP_METHOD_ID: u128 = 0x51C0_0E02;
const BUMP_ASYNC_METHOD_ID: u128 = 0x51C0_0E03;

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
        args: &[HostValue],
    ) -> HostResult<HostValue> {
        if method != HostMethodId::new(BUMP_METHOD_ID) {
            return Err(invalid_method());
        }
        let delta = host_i64(args.first())?;
        *self.value += delta;
        Ok(HostValue::i64(*self.value))
    }

    fn call_async_host_exclusive<'call>(
        &'call mut self,
        method: HostMethodId,
        args: Vec<HostValue>,
    ) -> vela_host::object::HostCallFuture<'call> {
        if method != HostMethodId::new(BUMP_ASYNC_METHOD_ID) {
            return Box::pin(async { Err(invalid_method()) });
        }
        Box::pin(async move {
            HostYieldOnce { pending: true }.await;
            let delta = host_i64(args.first())?;
            *self.value += delta;
            Ok(HostValue::i64(*self.value))
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
    HostTypeSpec::new(context_type_desc())
        .erased_method(bump)
        .erased_async_method(bump_async)
}

#[service(path = "scoped_test::handler")]
pub trait ScopedHandlerService: Send + Sync {
    async fn handle(&self, context: &mut BorrowedContext<'_>, delta: i64) -> i64;
}

struct RustScopedHandler;

impl ScopedHandlerService for RustScopedHandler {
    async fn handle(&self, context: &mut BorrowedContext<'_>, delta: i64) -> i64 {
        *context.value += delta;
        *context.value
    }
}

#[service_domain]
pub struct ScopedServices {
    pub handler: Service<dyn ScopedHandlerService>,
}

const PATCH_SOURCE: &str = r#"
#[service_impl(scoped_test::handler)]
impl HandlerPatch {
    async fn handle(context: BorrowedContext, delta: i64) -> i64 {
        let after_sync = context.bump(delta);
        return after_sync + context.bump_async(delta).await;
    }
}
"#;

#[test]
fn async_service_accepts_send_non_sync_non_static_host_context() {
    let app = ScopedServices::builder(
        Engine::builder()
            .capabilities(CapabilitySet::new().with(Capability::HostWrite))
            .register_host_type(context_type_spec()),
    )
    .handler(RustScopedHandler)
    .build()
    .expect("service application with schema-only Host contract");
    let (engine, services) = app.into_parts();
    let base = services.pin();
    let mut default_value = 3_i64;
    let mut default_context = BorrowedContext {
        value: &mut default_value,
        not_sync: Cell::new(()),
    };
    let default_future = base.handler().handle(&mut default_context, 4);
    assert_send(&default_future);
    assert_eq!(poll_ready(default_future), 7);
    assert_eq!(*default_context.value, 7);

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
            ServiceRuntimeBinding::for_engine(engine),
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
    let future = root.handler().handle(&mut context, 7);
    assert_send(&future);
    assert_eq!(poll_after_one_pending(future), 41);
    assert_eq!(*context.value, 24);
    context.not_sync.get();
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

fn host_i64(value: Option<&HostValue>) -> HostResult<i64> {
    match value {
        Some(HostValue::Scalar(vela_common::ScalarValue::I64(value))) => Ok(*value),
        _ => Err(HostError {
            kind: HostErrorKind::InvalidArgument {
                expected: "one i64 Host method argument",
            },
            source_span: None,
        }),
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

fn poll_ready<T>(mut future: Pin<Box<dyn Future<Output = T> + Send + '_>>) -> T {
    let mut task = Context::from_waker(Waker::noop());
    match future.as_mut().poll(&mut task) {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("Rust default should complete without suspension"),
    }
}

fn assert_send<T: Send>(_: &T) {}
