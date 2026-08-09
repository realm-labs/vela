use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

use vela_common::SourceId;
use vela_engine::engine::Engine;
use vela_engine::permission::{Capability, CapabilitySet};
use vela_engine::runtime::CallOptions;
use vela_engine::service::{Service, ServiceRuntimeBinding, ServiceSourceManifest};
use vela_hir::source_ingestion::build_single_source;
use vela_macros::{ScriptHost, Value, service, service_domain};

const ASYNC_PATCH_SOURCE: &str = r#"
#[service_impl(async_test::calculator)]
impl CalculatorPatch {
    async fn apply(context: RequestContext, input: Input) -> i64 {
        context.counter += 10;
        let adjusted = service::base::apply(context, input).await;
        let total = 0;
        for item in 0..3 { total += item + 1 - 1; }
        return adjusted + 20 + total - total;
    }
}
"#;

#[derive(Debug, Value)]
#[vela(path = "async_test::Input")]
pub struct Input {
    pub value: i64,
}

#[derive(ScriptHost)]
#[vela(path = "async_test::RequestContext")]
pub struct RequestContext {
    #[vela(get, set)]
    pub counter: i64,
}

#[service(path = "async_test::calculator")]
pub trait AsyncCalculatorService: Send + Sync {
    async fn apply(&self, context: &mut RequestContext, input: &Input) -> i64;
}

pub struct RustAsyncCalculatorService;

impl AsyncCalculatorService for RustAsyncCalculatorService {
    async fn apply(&self, context: &mut RequestContext, input: &Input) -> i64 {
        context.counter += 1;
        YieldOnce::new().await;
        if input.value == -1 {
            panic!("async service fixture panic");
        }
        context.counter += 1;
        input.value + 1
    }
}

struct YieldOnce {
    yielded: bool,
}

impl YieldOnce {
    const fn new() -> Self {
        Self { yielded: false }
    }
}

impl Future for YieldOnce {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        if self.yielded {
            Poll::Ready(())
        } else {
            self.yielded = true;
            context.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

#[service_domain(context = RequestContext)]
pub struct AsyncServices {
    pub calculator: Service<dyn AsyncCalculatorService>,
}

fn service_app() -> AsyncServicesApp {
    AsyncServices::builder(
        Engine::builder()
            .capabilities(CapabilitySet::new().with(Capability::HostWrite))
            .install_generated_type::<RequestContext>(),
    )
    .task_scope(crate::support::dropping_task_scope())
    .emergency_patch_effect_ceiling(crate::support::emergency_patch_effect_ceiling())
    .calculator(RustAsyncCalculatorService)
    .build()
    .expect("async service domain")
}

#[test]
fn async_service_root_selects_rust_or_vela_through_one_send_adapter() {
    let (engine, services) = service_app().into_parts();
    let rust_root = services.pin();
    let mut context = RequestContext { counter: 0 };

    let rust_input = Input { value: 5 };
    let rust_future = rust_root.calculator().apply(&mut context, &rust_input);
    assert_send(&rust_future);
    assert_eq!(poll_after_one_pending(rust_future), 6);
    assert_eq!(context.counter, 2);

    let sources =
        build_single_source(SourceId::new(61), ASYNC_PATCH_SOURCE).expect("valid async source");
    let manifest =
        ServiceSourceManifest::link(sources.graph(), services.schema()).expect("async schema");
    let compiled = engine
        .compile_source(ASYNC_PATCH_SOURCE)
        .expect("compiled async service source");
    let artifact = engine
        .link_compiled_program(compiled)
        .expect("linked async service artifact");
    assert!(artifact_has_selected_scalar_loop(&artifact));
    let update = manifest
        .bind_artifact(Arc::clone(&artifact))
        .expect("artifact-bound async update");
    let candidate = services
        .stage_snapshot(
            &rust_root,
            update,
            ServiceRuntimeBinding::for_engine(engine.clone()),
            CallOptions::new(100_000, 1024 * 1024, 64),
        )
        .expect("async snapshot");
    services
        .activate_if_current(candidate)
        .expect("activate async snapshot");
    let vela_root = services.pin();
    assert!(Arc::ptr_eq(
        vela_root.artifact().expect("async Vela artifact"),
        &artifact
    ));

    let vela_input = Input { value: 5 };
    let vela_future = vela_root.calculator().apply(&mut context, &vela_input);
    assert_send(&vela_future);
    assert_eq!(poll_after_one_pending(vela_future), 26);
    assert_eq!(context.counter, 14);
}

#[test]
fn application_runs_one_async_request_against_one_pinned_generation() {
    let app = service_app();
    let mut context = RequestContext { counter: 0 };
    let input = Input { value: 5 };

    let future = app.with_request_async(&mut context, async |root, request_context| {
        root.calculator().apply(request_context, &input).await
    });
    assert_send(&future);
    assert_eq!(poll_after_one_pending(Box::pin(future)), 6);
    assert_eq!(context.counter, 2);
}

#[test]
fn pending_actors_are_isolated_and_drop_or_unwind_restores_runtime_and_leases() {
    let (engine, services, root) = active_fixture();
    let mut first = RequestContext { counter: 0 };
    let mut second = RequestContext { counter: 0 };
    let mut third = RequestContext { counter: 0 };
    let mut finishing = RequestContext { counter: 0 };
    let first_input = Input { value: 5 };
    let finishing_input = Input { value: 4 };
    let mut first_future = root.calculator().apply(&mut first, &first_input);
    let mut finishing_future = root.calculator().apply(&mut finishing, &finishing_input);
    assert_send(&first_future);
    let mut task = Context::from_waker(Waker::noop());

    assert!(matches!(
        first_future.as_mut().poll(&mut task),
        Poll::Pending
    ));
    assert!(matches!(
        finishing_future.as_mut().poll(&mut task),
        Poll::Pending
    ));
    let rust_snapshot =
        vela_engine::service::LinkedServiceSourceManifest::from_updates(std::iter::empty::<
            vela_engine::service::ServiceMethodUpdate<
                vela_engine::service::LinkedVelaServiceMethod,
            >,
        >())
        .expect("Rust-default snapshot");
    let replacement = services
        .stage_snapshot(
            &root,
            rust_snapshot,
            ServiceRuntimeBinding::for_engine(engine.clone()),
            CallOptions::new(100_000, 1024 * 1024, 64),
        )
        .expect("stage Rust-default snapshot while old actor is pending");
    services
        .activate_if_current(replacement)
        .expect("activate Rust replacement");
    let new_root = services.pin();

    assert_eq!(
        poll_after_one_pending(root.calculator().apply(&mut second, &Input { value: 6 }),),
        27
    );
    assert_eq!(second.counter, 12);
    assert_eq!(
        poll_after_one_pending(new_root.calculator().apply(&mut third, &Input { value: 6 }),),
        7
    );
    assert_eq!(third.counter, 2);
    assert_ne!(root.generation_id(), new_root.generation_id());
    assert!(matches!(
        finishing_future.as_mut().poll(&mut task),
        Poll::Ready(25)
    ));
    drop(finishing_future);
    assert_eq!(
        finishing.counter, 12,
        "an in-flight old root must finish through its pinned Vela generation"
    );

    drop(first_future);
    assert_eq!(first.counter, 11, "completed effects are not rolled back");
    assert_eq!(
        poll_after_one_pending(root.calculator().apply(&mut first, &Input { value: 7 }),),
        28,
        "a cancelled call releases the exclusive context lease"
    );
    assert_eq!(first.counter, 23);

    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let input = Input { value: -1 };
        let mut future = root.calculator().apply(&mut first, &input);
        let mut task = Context::from_waker(Waker::noop());
        assert!(matches!(future.as_mut().poll(&mut task), Poll::Pending));
        let _ = future.as_mut().poll(&mut task);
    }));
    assert!(panic.is_err());
    assert_eq!(first.counter, 34);
    assert_eq!(
        poll_after_one_pending(root.calculator().apply(&mut first, &Input { value: 8 }),),
        29
    );
    assert_eq!(first.counter, 46);
}

fn active_fixture() -> (Engine, AsyncServices, AsyncServicesRoot) {
    let (engine, services) = service_app().into_parts();
    let base = services.pin();
    let sources =
        build_single_source(SourceId::new(62), ASYNC_PATCH_SOURCE).expect("valid async source");
    let manifest =
        ServiceSourceManifest::link(sources.graph(), services.schema()).expect("async schema");
    let compiled = engine
        .compile_source(ASYNC_PATCH_SOURCE)
        .expect("compiled async service source");
    let artifact = engine
        .link_compiled_program(compiled)
        .expect("linked async service artifact");
    assert!(artifact_has_selected_scalar_loop(&artifact));
    let update = manifest
        .bind_artifact(Arc::clone(&artifact))
        .expect("artifact-bound async update");
    let candidate = services
        .stage_snapshot(
            &base,
            update,
            ServiceRuntimeBinding::for_engine(engine.clone()),
            CallOptions::new(100_000, 1024 * 1024, 64),
        )
        .expect("async snapshot");
    services
        .activate_if_current(candidate)
        .expect("activate async snapshot");
    let root = services.pin();
    assert!(Arc::ptr_eq(
        root.artifact().expect("active async artifact"),
        &artifact
    ));
    (engine, services, root)
}

fn artifact_has_selected_scalar_loop(artifact: &vela_bytecode::LinkedArtifact) -> bool {
    artifact.program().functions().any(|(_, code)| {
        code.scalar_blocks
            .iter()
            .any(|plan| plan.range_loop.is_some())
    })
}

fn poll_after_one_pending<T>(mut future: impl Future<Output = T> + Unpin) -> T {
    let mut context = Context::from_waker(Waker::noop());
    assert!(matches!(
        Pin::new(&mut future).poll(&mut context),
        Poll::Pending
    ));
    match Pin::new(&mut future).poll(&mut context) {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("fixture future should complete on its second poll"),
    }
}

fn assert_send<T: Send>(_: &T) {}
