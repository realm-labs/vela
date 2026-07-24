use std::sync::{Arc, Mutex};

use vela_common::SourceId;
use vela_engine::engine::Engine;
use vela_engine::runtime::{CallOptions, Runtime, RuntimeBuildError};
use vela_engine::service::{
    ServiceRuntimeAuthority, ServiceRuntimeBinding, ServiceRuntimeSlot, ServiceSourceManifest,
};
use vela_hir::source_ingestion::build_single_source;
use vela_macros::{ScriptHost, service, service_set};

#[derive(ScriptHost)]
#[script(path = "test::RequestContext")]
pub struct RequestContext {
    pub counter: i64,
    #[script(skip)]
    runtime: Mutex<ServiceRuntimeSlot>,
}

#[vela_macros::script_methods]
impl RequestContext {}

impl ServiceRuntimeAuthority for RequestContext {
    fn take_service_runtime(
        &mut self,
        artifact: &Arc<vela_bytecode::LinkedArtifact>,
    ) -> Result<Runtime, RuntimeBuildError> {
        self.runtime
            .get_mut()
            .expect("exclusive request context cannot poison its Runtime slot")
            .take(artifact)
    }

    fn restore_service_runtime(
        &mut self,
        artifact: &Arc<vela_bytecode::LinkedArtifact>,
        runtime: Runtime,
    ) {
        self.runtime
            .get_mut()
            .expect("exclusive request context cannot poison its Runtime slot")
            .restore(artifact, runtime);
    }
}

struct WrongContext;

impl ServiceRuntimeAuthority for WrongContext {
    fn take_service_runtime(
        &mut self,
        _artifact: &Arc<vela_bytecode::LinkedArtifact>,
    ) -> Result<Runtime, RuntimeBuildError> {
        unreachable!("a mismatched context must fail during staging")
    }

    fn restore_service_runtime(
        &mut self,
        _artifact: &Arc<vela_bytecode::LinkedArtifact>,
        _runtime: Runtime,
    ) {
        unreachable!("a mismatched context must fail during staging")
    }
}

#[service(path = "test::calculator")]
pub trait CalculatorService: Send + Sync {
    fn adjust(&self, context: &mut RequestContext, value: i64) -> i64;
    fn adjacent(&self, context: &mut RequestContext, value: i64) -> i64;
}

pub struct RustCalculatorService;

impl CalculatorService for RustCalculatorService {
    fn adjust(&self, context: &mut RequestContext, value: i64) -> i64 {
        context.counter += 1;
        value + 1
    }

    fn adjacent(&self, context: &mut RequestContext, value: i64) -> i64 {
        context.counter += 10;
        value * 2
    }
}

#[service_set(context = RequestContext)]
pub struct TestServices {
    #[vela::default(RustCalculatorService)]
    pub calculator: dyn CalculatorService,
}

#[test]
fn snapshot_activates_one_vela_method_keeps_adjacent_rust_and_rolls_back() {
    let engine = TestServices::register_types(
        Engine::builder().register_rust_type::<RequestContext>(RequestContext::vela_type_binding()),
    )
    .build()
    .expect("service engine");
    let services = TestServices::new(&engine.type_bindings()).expect("service set");
    let old = services.pin();
    let mut context = RequestContext {
        counter: 0,
        runtime: Mutex::new(ServiceRuntimeSlot::new(engine.clone())),
    };

    assert_eq!(old.calculator().adjust(&mut context, 5), 6);
    assert_eq!(context.counter, 1);

    let source = r#"
#[service_impl(test::calculator)]
impl CalculatorHotfix {
    fn adjust(context, value) {
        return value + 10;
    }
}
"#;
    let sources = build_single_source(SourceId::new(1), source).expect("valid service source");
    let manifest =
        ServiceSourceManifest::link(sources.graph(), services.schema()).expect("linked schema");
    let compiled = engine
        .compile_source(source)
        .expect("compiled service source");
    let artifact = engine
        .link_compiled_program(compiled)
        .expect("linked service artifact");
    let update = manifest
        .bind_artifact(artifact)
        .expect("artifact-bound update");
    let Err(error) = services.stage_snapshot(
        &old,
        update.clone(),
        ServiceRuntimeBinding::for_context::<WrongContext>(),
        CallOptions::new(100_000, 1024 * 1024, 64),
    ) else {
        panic!("foreign Runtime context must fail");
    };
    assert!(matches!(
        error,
        vela_engine::service::ServiceStagingError::ContextTypeMismatch { .. }
    ));
    assert_eq!(services.pin().generation_id(), old.generation_id());
    let candidate = services
        .stage_snapshot(
            &old,
            update,
            ServiceRuntimeBinding::for_context::<RequestContext>(),
            CallOptions::new(100_000, 1024 * 1024, 64),
        )
        .expect("snapshot candidate");
    let installed = candidate.generation_id();
    let rollback = services
        .activate_if_current(candidate)
        .expect("snapshot activation");
    let active = services.pin();

    assert_eq!(active.generation_id(), installed);
    assert_eq!(old.calculator().adjust(&mut context, 5), 6);
    assert_eq!(active.calculator().adjust(&mut context, 5), 15);
    assert_eq!(active.calculator().adjacent(&mut context, 5), 10);
    assert_eq!(context.counter, 12);
    assert!(active.selections().is_some());

    services
        .rollback_if_current(rollback)
        .expect("conditional rollback");
    let restored = services.pin();
    assert_eq!(restored.generation_id(), old.generation_id());
    assert_eq!(restored.calculator().adjust(&mut context, 5), 6);
    assert_eq!(context.counter, 13);
}
