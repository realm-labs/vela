use std::sync::Arc;

use vela_common::SourceId;
use vela_engine::engine::Engine;
use vela_engine::permission::{Capability, CapabilitySet};
use vela_engine::runtime::{CallOptions, Runtime, RuntimeBuildError};
use vela_engine::service::{
    LinkedServiceSourceManifest, ServiceMethodSelection, ServiceMethodUpdate,
    ServiceRuntimeAuthority, ServiceRuntimeBinding, ServiceRuntimeSlot, ServiceSourceManifest,
};
use vela_hir::source_ingestion::build_single_source;
use vela_macros::{ScriptHost, service, service_set};

#[derive(ScriptHost)]
#[script(path = "test::RequestContext")]
pub struct RequestContext {
    pub counter: i64,
    #[script(skip)]
    runtime: ServiceRuntimeSlot,
}

#[vela_macros::script_methods]
impl RequestContext {}

impl ServiceRuntimeAuthority for RequestContext {
    fn take_service_runtime(
        &mut self,
        artifact: &Arc<vela_bytecode::LinkedArtifact>,
    ) -> Result<Runtime, RuntimeBuildError> {
        self.runtime.take(artifact)
    }

    fn restore_service_runtime(
        &mut self,
        artifact: &Arc<vela_bytecode::LinkedArtifact>,
        runtime: Runtime,
    ) {
        self.runtime.restore(artifact, runtime);
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

#[service(path = "test::audit")]
pub trait AuditService: Send + Sync {
    fn record(&self, context: &mut RequestContext, value: i64) -> i64;
}

pub struct RustAuditService;

impl AuditService for RustAuditService {
    fn record(&self, context: &mut RequestContext, value: i64) -> i64 {
        context.counter += 100;
        value * 2
    }
}

#[service_set(context = RequestContext)]
pub struct TestServices {
    #[vela::default(RustCalculatorService)]
    pub calculator: dyn CalculatorService,
    #[vela::default(RustAuditService)]
    pub audit: dyn AuditService,
}

#[test]
fn snapshot_activates_one_vela_method_keeps_adjacent_rust_and_rolls_back() {
    let engine = TestServices::register_types(
        Engine::builder()
            .capabilities(CapabilitySet::new().with(Capability::HostWrite))
            .register_rust_type::<RequestContext>(RequestContext::vela_type_binding()),
    )
    .build()
    .expect("service engine");
    let services = TestServices::new(&engine.type_bindings()).expect("service set");
    let old = services.pin();
    let mut context = RequestContext {
        counter: 0,
        runtime: ServiceRuntimeSlot::new(engine.clone()),
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
            update.clone(),
            ServiceRuntimeBinding::for_context::<RequestContext>(),
            call_options(),
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

    let snapshot = services
        .stage_snapshot(
            &restored,
            update,
            ServiceRuntimeBinding::for_context::<RequestContext>(),
            call_options(),
        )
        .expect("second snapshot candidate");
    services
        .activate_if_current(snapshot)
        .expect("second snapshot activation");
    let vela_base = services.pin();

    let delta_source = r#"
#[service_impl(test::calculator)]
impl CalculatorHotfix {
    fn adjust(context, value) {
        return value + 10;
    }

    fn adjacent(context, value) {
        return value * 3;
    }
}
"#;
    let delta_sources =
        build_single_source(SourceId::new(2), delta_source).expect("valid Delta source");
    let delta_manifest = ServiceSourceManifest::link(delta_sources.graph(), services.schema())
        .expect("Delta schema");
    let delta_compiled = engine
        .compile_source(delta_source)
        .expect("compiled Delta source");
    let delta_artifact = engine
        .link_compiled_program(delta_compiled)
        .expect("linked Delta artifact");
    let delta_artifact_generation = delta_artifact.generation();
    let linked_delta = delta_manifest
        .bind_artifact(delta_artifact)
        .expect("artifact-bound Delta");
    let calculator = services
        .schema()
        .services()
        .iter()
        .find(|service| service.path() == "test::calculator")
        .expect("calculator schema");
    let adjacent = calculator
        .methods()
        .iter()
        .find(|method| method.path.ends_with("::adjacent"))
        .expect("adjacent schema");
    let sparse_delta = LinkedServiceSourceManifest::from_updates(
        linked_delta
            .into_updates()
            .into_iter()
            .filter(|update| update.key().method_id == adjacent.id),
    )
    .expect("one-artifact sparse Delta");
    let delta_candidate = services
        .stage_delta(
            &vela_base,
            sparse_delta,
            ServiceRuntimeBinding::for_context::<RequestContext>(),
            call_options(),
        )
        .expect("exact-base Delta");
    services
        .activate_if_current(delta_candidate)
        .expect("Delta activation");
    let delta_root = services.pin();

    context.counter = 0;
    assert_eq!(delta_root.calculator().adjust(&mut context, 5), 15);
    assert_eq!(delta_root.calculator().adjacent(&mut context, 5), 15);
    assert_eq!(context.counter, 0);
    let vela_targets = delta_root
        .selections()
        .expect("Delta selections")
        .iter()
        .filter_map(|(_, selection)| match selection {
            ServiceMethodSelection::RustDefault => None,
            ServiceMethodSelection::Vela(target) => Some(target),
        })
        .collect::<Vec<_>>();
    assert_eq!(vela_targets.len(), 2);
    assert!(
        vela_targets
            .iter()
            .all(|target| { target.artifact().generation() == delta_artifact_generation })
    );

    let rust_default =
        LinkedServiceSourceManifest::from_updates([ServiceMethodUpdate::rust_default(
            calculator.id(),
            adjacent.id,
            calculator.abi_fingerprint(),
        )])
        .expect("explicit RustDefault Delta");
    let default_candidate = services
        .stage_delta(
            &delta_root,
            rust_default.clone(),
            ServiceRuntimeBinding::for_context::<RequestContext>(),
            call_options(),
        )
        .expect("RustDefault candidate");
    let stale_candidate = services
        .stage_delta(
            &delta_root,
            rust_default,
            ServiceRuntimeBinding::for_context::<RequestContext>(),
            call_options(),
        )
        .expect("concurrent stale candidate");
    services
        .activate_if_current(default_candidate)
        .expect("RustDefault activation");
    assert!(matches!(
        services.activate_if_current(stale_candidate),
        Err(vela_engine::service::ServicePublicationError::StaleBaseGeneration { .. })
    ));
    let default_root = services.pin();
    context.counter = 0;
    assert_eq!(default_root.calculator().adjust(&mut context, 5), 15);
    assert_eq!(default_root.calculator().adjacent(&mut context, 5), 10);
    assert_eq!(context.counter, 10);
    assert_eq!(
        default_root
            .selections()
            .expect("default selections")
            .get(calculator.id(), adjacent.id),
        Some(&ServiceMethodSelection::RustDefault)
    );

    let trap_source = r#"
#[service_impl(test::calculator)]
impl CalculatorTrap {
    fn adjust(context, value) {
        return value / 0;
    }
}
"#;
    let trap_sources =
        build_single_source(SourceId::new(3), trap_source).expect("valid trap source");
    let trap_manifest =
        ServiceSourceManifest::link(trap_sources.graph(), services.schema()).expect("trap schema");
    let trap_compiled = engine.compile_source(trap_source).expect("compiled trap");
    let trap_artifact = engine
        .link_compiled_program(trap_compiled)
        .expect("linked trap");
    let trap_update = trap_manifest
        .bind_artifact(trap_artifact)
        .expect("artifact-bound trap");
    let trap_candidate = services
        .stage_snapshot(
            &default_root,
            trap_update,
            ServiceRuntimeBinding::for_context::<RequestContext>(),
            call_options(),
        )
        .expect("trap snapshot");
    services
        .activate_if_current(trap_candidate)
        .expect("trap activation");
    let trap_root = services.pin();
    context.counter = 0;
    let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        trap_root.calculator().adjust(&mut context, 5)
    }));
    assert!(failure.is_err());
    assert_eq!(
        context.counter, 0,
        "a Vela failure must not retry the Rust default"
    );
}

#[test]
fn lexical_base_and_pinned_cross_service_calls_keep_one_generation() {
    let engine = TestServices::register_types(
        Engine::builder()
            .capabilities(CapabilitySet::new().with(Capability::HostWrite))
            .register_rust_type::<RequestContext>(RequestContext::vela_type_binding()),
    )
    .build()
    .expect("service engine");
    let services = TestServices::new(&engine.type_bindings()).expect("service set");
    let base = services.pin();
    let mut context = RequestContext {
        counter: 0,
        runtime: ServiceRuntimeSlot::new(engine.clone()),
    };
    let snapshot_source = r#"
#[service_impl(test::calculator)]
impl CalculatorHotfix {
    fn adjust(context, value) {
        let original = base.adjust(context, value);
        return services.audit.record(context, original);
    }
}
"#;
    let snapshot_update = linked_update(&engine, services.schema(), 11, snapshot_source);
    let snapshot = services
        .stage_snapshot(
            &base,
            snapshot_update,
            ServiceRuntimeBinding::for_context::<RequestContext>(),
            call_options(),
        )
        .expect("lexical snapshot");
    services
        .activate_if_current(snapshot)
        .expect("activate lexical snapshot");
    let first = services.pin();

    assert_eq!(first.calculator().adjust(&mut context, 5), 12);
    assert_eq!(
        context.counter, 101,
        "base and the pinned Rust service must see one host object"
    );

    let delta_source = r#"
#[service_impl(test::calculator)]
impl CalculatorHotfix {
    fn adjust(context, value) {
        let original = base.adjust(context, value);
        return services.audit.record(context, original);
    }
}

#[service_impl(test::audit)]
impl AuditHotfix {
    fn record(context, value) {
        return value + 7;
    }
}
"#;
    let linked_delta = linked_update(&engine, services.schema(), 12, delta_source);
    let audit = services
        .schema()
        .service("audit")
        .expect("audit service schema");
    let sparse_delta = LinkedServiceSourceManifest::from_updates(
        linked_delta
            .into_updates()
            .into_iter()
            .filter(|update| update.key().service_id == audit.id()),
    )
    .expect("audit-only Delta");
    let delta = services
        .stage_delta(
            &first,
            sparse_delta,
            ServiceRuntimeBinding::for_context::<RequestContext>(),
            call_options(),
        )
        .expect("exact-base cross-service Delta");
    services
        .activate_if_current(delta)
        .expect("activate cross-service Delta");
    let second = services.pin();

    context.counter = 0;
    assert_eq!(first.calculator().adjust(&mut context, 5), 12);
    assert_eq!(
        context.counter, 101,
        "old root retains the Rust audit selection"
    );
    context.counter = 0;
    assert_eq!(second.calculator().adjust(&mut context, 5), 13);
    assert_eq!(
        context.counter, 1,
        "new root inherits calculator Vela and pins the patched audit service"
    );
}

fn linked_update(
    engine: &Engine,
    schema: &vela_engine::service::ServiceSetSchema,
    source_id: u32,
    source: &str,
) -> LinkedServiceSourceManifest {
    let sources =
        build_single_source(SourceId::new(source_id), source).expect("valid service source");
    let manifest =
        ServiceSourceManifest::link(sources.graph(), schema).expect("schema-linked update");
    let compiled = engine
        .compile_source(source)
        .expect("compiled service update");
    let artifact = engine
        .link_compiled_program(compiled)
        .expect("linked service update");
    manifest
        .bind_artifact(artifact)
        .expect("artifact-bound service update")
}

fn call_options() -> CallOptions {
    CallOptions::new(100_000, 1024 * 1024, 64)
}
