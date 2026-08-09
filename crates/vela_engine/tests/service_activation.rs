use std::sync::Arc;

use vela_common::SourceId;
use vela_engine::engine::Engine;
use vela_engine::permission::{Capability, CapabilitySet};
use vela_engine::runtime::CallOptions;
#[cfg(feature = "artifact-codec")]
use vela_engine::service::PatchRevision;
use vela_engine::service::{
    LinkedServiceSourceManifest, PatchEdit, PatchSources, Service, ServiceMethodSelection,
    ServiceMethodUpdate, ServicePatch, ServiceRuntimeBinding, ServiceSourceManifest,
    ServiceUpdateBundle,
};
use vela_hir::source_ingestion::build_single_source;
use vela_macros::{ScriptHost, service, service_domain};

#[derive(ScriptHost)]
#[vela(path = "test::RequestContext")]
pub struct RequestContext {
    #[vela(get, set)]
    pub counter: i64,
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

#[service_domain(context = RequestContext)]
pub struct TestServices {
    pub calculator: Service<dyn CalculatorService>,
    pub audit: Service<dyn AuditService>,
}

fn service_app() -> TestServicesApp {
    TestServices::builder(
        Engine::builder()
            .capabilities(
                CapabilitySet::new()
                    .with(Capability::HostWrite)
                    .with(Capability::TaskSpawn),
            )
            .install_generated_type::<RequestContext>(),
    )
    .task_scope(crate::support::dropping_task_scope())
    .emergency_patch_effect_ceiling(crate::support::emergency_patch_effect_ceiling())
    .calculator(RustCalculatorService)
    .audit(RustAuditService)
    .build()
    .expect("service domain")
}

#[test]
fn application_patch_facade_edits_multifile_workspace_and_activates_snapshot() {
    let app = service_app();
    let old = app.domain().pin();
    let revision = app.patches().revision().expect("initial revision");
    let staged = app
        .patches()
        .stage(
            ServicePatch::against(&revision)
                .put(
                    "calculator.vela",
                    r#"
#[service_impl(test::calculator)]
impl CalculatorHotfix {
    fn adjust(context, value) {
        return value + 40;
    }
}
"#,
                )
                .put(
                    "audit.vela",
                    r#"
#[service_impl(test::audit)]
impl AuditHotfix {
    fn record(context, value) {
        return value + 5;
    }
}
"#,
                ),
        )
        .expect("stage source workspace");
    assert_ne!(staged.generation_id(), old.generation_id());
    staged.activate().expect("activate staged source");
    let active_revision = app.patches().revision().expect("active revision");
    assert_eq!(active_revision.sources().len(), 2);
    assert!(active_revision.sources().contains("calculator.vela"));
    assert!(active_revision.sources().contains("audit.vela"));
    assert!(matches!(
        app.patches().stage(ServicePatch::against(&revision).put(
            "calculator.vela",
            "fn stale_edit_must_not_compile() { return 0; }",
        ),),
        Err(vela_engine::service::ServicePatchError::Workspace(
            vela_engine::service::ServicePatchWorkspaceError::StaleServiceGeneration { .. }
        ))
    ));

    let mut context = RequestContext { counter: 0 };
    app.with_request(&mut context, |root, request_context| {
        assert_eq!(root.calculator().adjust(request_context, 2), 42);
        assert_eq!(root.audit().record(request_context, 2), 7);
    });

    let removal_rollback = app
        .patches()
        .apply(PatchEdit::remove("audit.vela"))
        .expect("remove one source");
    let active_revision = app.patches().revision().expect("edited revision");
    assert_eq!(active_revision.sources().len(), 1);
    app.with_request(&mut context, |root, request_context| {
        assert_eq!(root.audit().record(request_context, 2), 4);
    });
    app.patches()
        .rollback(removal_rollback)
        .expect("rollback source removal");
    assert_eq!(
        app.patches()
            .revision()
            .expect("restored revision")
            .sources()
            .len(),
        2
    );

    let replacement = PatchSources::from_files([("calculator.vela", "fn helper() { return 1; }")])
        .expect("replacement sources");
    app.patches()
        .apply(ServicePatch::replace(replacement))
        .expect("complete replacement");
    assert_eq!(
        app.patches()
            .revision()
            .expect("replacement revision")
            .sources()
            .len(),
        1
    );
}

#[test]
fn deployment_bundle_build_load_dry_run_and_exact_base_diagnostics() {
    let (engine, services) = service_app().into_parts();
    let initial = services.pin();
    let source = r#"
#[service_impl(test::calculator)]
impl CalculatorHotfix {
    fn adjust(context, value) {
        let total = 0;
        for item in 0..3 { total += item + 1 - 1; }
        return value + 20 + total - total;
    }
}
"#;
    let sources = build_single_source(SourceId::new(90), source).expect("valid source");
    let manifest =
        ServiceSourceManifest::link(sources.graph(), services.schema()).expect("service manifest");
    let artifact = engine
        .link_compiled_program(engine.compile_source(source).expect("compiled source"))
        .expect("linked artifact");
    assert!(artifact_has_selected_scalar_loop(&artifact));
    let second_artifact = engine
        .link_compiled_program(engine.compile_source(source).expect("recompiled source"))
        .expect("relinked artifact");
    assert_ne!(artifact.generation(), second_artifact.generation());
    assert_eq!(artifact.checksum(), second_artifact.checksum());

    let update = manifest
        .bind_artifact(Arc::clone(&artifact))
        .expect("artifact-bound update");
    let bundle =
        ServiceUpdateBundle::snapshot(services.schema(), Arc::clone(&artifact), update.clone())
            .expect("Snapshot bundle");
    let metadata = bundle.metadata().clone();
    assert_eq!(metadata.artifact_checksum(), artifact.checksum());
    assert_eq!(metadata.update_count(), 1);
    let loaded =
        ServiceUpdateBundle::load(metadata, services.schema(), Arc::clone(&artifact), update)
            .expect("loaded bundle");
    let report = services.dry_run_bundle(&initial, &loaded);
    assert!(report.accepted());
    let summary = report.outcome().as_ref().expect("selection summary");
    assert_eq!(summary.method_count(), 3);
    assert_eq!(summary.vela_count(), 1);
    assert_eq!(summary.rust_default_count(), 2);
    assert_eq!(services.pin().generation_id(), initial.generation_id());

    let candidate = services
        .stage_bundle(
            &initial,
            loaded,
            ServiceRuntimeBinding::for_engine(engine.clone()),
            call_options(),
        )
        .expect("staged bundle");
    services
        .activate_if_current(candidate)
        .expect("activated bundle");
    let active = services.pin();
    assert_eq!(active.artifact_checksum(), Some(artifact.checksum()));

    let delta_update = LinkedServiceSourceManifest::from_updates(
        active
            .selections()
            .expect("active selections")
            .iter()
            .filter_map(|(key, selection)| match selection {
                ServiceMethodSelection::RustDefault => None,
                ServiceMethodSelection::Vela(target) => Some(ServiceMethodUpdate::vela(
                    key.service_id,
                    key.method_id,
                    services
                        .schema()
                        .services()
                        .iter()
                        .find(|service| service.id() == key.service_id)
                        .expect("service schema")
                        .abi_fingerprint(),
                    target.clone(),
                )),
            }),
    )
    .expect("Delta update");
    let wrong_base = vela_bytecode::ArtifactChecksum::new([0x5a; 32]);
    let stale_bundle = ServiceUpdateBundle::delta(
        services.schema(),
        active.generation_id(),
        wrong_base,
        artifact,
        delta_update,
    )
    .expect("structurally valid Delta bundle");
    let stale_report = services.dry_run_bundle(&active, &stale_bundle);
    assert!(!stale_report.accepted());
    assert!(matches!(
        stale_report.outcome(),
        Err(vela_engine::service::ServiceStagingError::Deployment(
            vela_engine::service::ServiceBundleError::BaseArtifactChecksumMismatch { .. }
        ))
    ));
    assert_eq!(services.pin().generation_id(), active.generation_id());
}

#[test]
fn snapshot_activates_one_vela_method_keeps_adjacent_rust_and_rolls_back() {
    let (engine, services) = service_app().into_parts();
    let old = services.pin();
    let mut context = RequestContext { counter: 0 };

    assert_eq!(old.calculator().adjust(&mut context, 5), 6);
    assert_eq!(context.counter, 1);

    let source = r#"
#[service_impl(test::calculator)]
impl CalculatorHotfix {
    fn adjust(context, value) {
        let total = 0;
        for item in 0..3 { total += item + 1 - 1; }
        return value + 10 + total - total;
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
    assert!(artifact_has_selected_scalar_loop(&artifact));
    let update = manifest
        .bind_artifact(Arc::clone(&artifact))
        .expect("artifact-bound update");
    let candidate = services
        .stage_snapshot(
            &old,
            update.clone(),
            ServiceRuntimeBinding::for_engine(engine.clone()),
            call_options(),
        )
        .expect("snapshot candidate");
    let installed = candidate.generation_id();
    let rollback = services
        .activate_if_current(candidate)
        .expect("snapshot activation");
    let active = services.pin();

    assert_eq!(active.generation_id(), installed);
    assert!(Arc::ptr_eq(
        active.artifact().expect("active Snapshot artifact"),
        &artifact
    ));
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
    assert!(restored.artifact().is_none());
    assert!(artifact_has_selected_scalar_loop(&artifact));
    assert_eq!(restored.calculator().adjust(&mut context, 5), 6);
    assert_eq!(context.counter, 13);

    let snapshot = services
        .stage_snapshot(
            &restored,
            update,
            ServiceRuntimeBinding::for_engine(engine.clone()),
            call_options(),
        )
        .expect("second snapshot candidate");
    services
        .activate_if_current(snapshot)
        .expect("second snapshot activation");
    let vela_base = services.pin();
    assert!(Arc::ptr_eq(
        vela_base.artifact().expect("reactivated Snapshot artifact"),
        &artifact
    ));

    let delta_source = r#"
#[service_impl(test::calculator)]
impl CalculatorHotfix {
    fn adjust(context, value) {
        let total = 0;
        for item in 0..3 { total += item + 1 - 1; }
        return value + 10 + total - total;
    }

    fn adjacent(context, value) {
        let total = 0;
        for item in 0..4 { total += item + 1 - 1; }
        return value * 3 + total - total;
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
    assert!(artifact_has_selected_scalar_loop(&delta_artifact));
    let delta_artifact_generation = delta_artifact.generation();
    let delta_artifact_owner = Arc::clone(&delta_artifact);
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
            ServiceRuntimeBinding::for_engine(engine.clone()),
            call_options(),
        )
        .expect("exact-base Delta");
    services
        .activate_if_current(delta_candidate)
        .expect("Delta activation");
    let delta_root = services.pin();
    assert!(Arc::ptr_eq(
        delta_root.artifact().expect("folded Delta artifact"),
        &delta_artifact_owner
    ));

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
            ServiceRuntimeBinding::for_engine(engine.clone()),
            call_options(),
        )
        .expect("RustDefault candidate");
    let stale_candidate = services
        .stage_delta(
            &delta_root,
            rust_default,
            ServiceRuntimeBinding::for_engine(engine.clone()),
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
            ServiceRuntimeBinding::for_engine(engine.clone()),
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
    let (engine, services) = service_app().into_parts();
    let base = services.pin();
    let mut context = RequestContext { counter: 0 };
    let snapshot_source = r#"
#[service_impl(test::calculator)]
impl CalculatorHotfix {
    fn adjust(context, value) {
        let total = 0;
        for item in 0..3 { total += item + 1 - 1; }
        let original = service::base::adjust(context, value);
        return service::pinned::audit::record(context, original) + total - total;
    }
}
"#;
    let snapshot_update = linked_update(&engine, services.schema(), 11, snapshot_source);
    assert!(artifact_has_selected_scalar_loop(
        snapshot_update.artifact().expect("Snapshot artifact")
    ));
    let snapshot = services
        .stage_snapshot(
            &base,
            snapshot_update,
            ServiceRuntimeBinding::for_engine(engine.clone()),
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
        let total = 0;
        for item in 0..3 { total += item + 1 - 1; }
        let original = service::base::adjust(context, value);
        return service::pinned::audit::record(context, original) + total - total;
    }
}

#[service_impl(test::audit)]
impl AuditHotfix {
    fn record(context, value) {
        let total = 0;
        for item in 0..4 { total += item + 1 - 1; }
        return value + 7 + total - total;
    }
}
"#;
    let linked_delta = linked_update(&engine, services.schema(), 12, delta_source);
    let delta_artifact = Arc::clone(linked_delta.artifact().expect("Delta artifact"));
    assert!(artifact_has_selected_scalar_loop(&delta_artifact));
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
            ServiceRuntimeBinding::for_engine(engine.clone()),
            call_options(),
        )
        .expect("exact-base cross-service Delta");
    services
        .activate_if_current(delta)
        .expect("activate cross-service Delta");
    let second = services.pin();
    assert!(Arc::ptr_eq(
        second.artifact().expect("cross-service Delta artifact"),
        &delta_artifact
    ));

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

fn artifact_has_selected_scalar_loop(artifact: &vela_bytecode::LinkedArtifact) -> bool {
    artifact.program().functions().any(|(_, code)| {
        code.scalar_blocks
            .iter()
            .any(|plan| plan.range_loop.is_some())
    })
}

#[cfg(feature = "artifact-codec")]
#[test]
fn portable_service_bundle_round_trips_binds_and_executes_without_source_compilation() {
    let (engine, services) = service_app().into_parts();
    let base = services.pin();
    let source = r#"
async fn portable_worker(value: i64) -> i64 {
    let total = 0;
    for item in 0..3 { total += item + 1 - 1; }
    return value + 1 + total - total;
}

#[service_impl(test::calculator)]
impl CalculatorPortableHotfix {
    fn adjust(context: RequestContext, value: i64) -> i64 {
        task::spawn_scoped(portable_worker(value));
        context.counter += 5;
        if value > 4 {
            return value + 31;
        }
        return value + 30;
    }
}
"#;
    let audit_source = r#"
#[service_impl(test::audit)]
impl AuditPortableHotfix {
    fn record(context: RequestContext, value: i64) -> i64 {
        context.counter += 7;
        return value + 50;
    }
}
"#;
    let revision = PatchRevision::empty();
    let patch = ServicePatch::against(&revision)
        .put("calculator.vela", source)
        .put("audit.vela", audit_source);
    let revision = revision.apply(patch).expect("offline multi-file revision");
    let host_schema_hash = 0x1234_5678_9abc_def0;
    let portable = engine
        .compile_portable_service_patch(services.schema(), &revision, host_schema_hash)
        .expect("portable service bundle");
    let artifact_checksum = portable.artifact_checksum();
    let first = portable.encode().expect("first encoding");
    let second = portable.encode().expect("second encoding");
    assert_eq!(first, second);
    for old_version in [1_u32, 2, 3, 4] {
        let mut old = first.clone();
        old[8..12].copy_from_slice(&old_version.to_le_bytes());
        assert!(matches!(
            vela_engine::service::PortableServiceUpdateBundle::decode(&old),
            Err(
                vela_engine::service::PortableServiceBundleError::UnsupportedFormat {
                    expected: 5,
                    actual,
                }
            ) if actual == old_version
        ));
    }
    assert_eq!(services.pin().generation_id(), base.generation_id());
    let checksum = portable.checksum();
    let mut corrupted = first.clone();
    *corrupted.last_mut().expect("payload byte") ^= 0x40;
    assert!(matches!(
        vela_engine::service::PortableServiceUpdateBundle::decode(&corrupted),
        Err(vela_engine::service::PortableServiceBundleError::ChecksumMismatch)
    ));

    let decoded = vela_engine::service::PortableServiceUpdateBundle::decode(&first)
        .expect("decode service bundle");
    assert_eq!(decoded.checksum(), checksum);
    assert_eq!(
        decoded.artifact_checksum().as_bytes(),
        artifact_checksum.as_bytes()
    );
    assert_eq!(
        decoded.mode(),
        vela_engine::service::ServiceUpdateMode::Snapshot
    );
    assert_eq!(decoded.diagnostics().len(), 2);
    assert_eq!(decoded.diagnostics()[0].path(), "audit.vela");
    assert_eq!(decoded.diagnostics()[1].path(), "calculator.vela");
    assert!(matches!(
        decoded
            .clone()
            .load(&engine, services.schema(), host_schema_hash ^ 1),
        Err(vela_engine::service::PortableServiceBundleError::HostSchemaHashMismatch { .. })
    ));
    let loaded = decoded
        .load(&engine, services.schema(), host_schema_hash)
        .expect("bind portable service bundle");
    let [task_target] = loaded.artifact().task_targets() else {
        panic!("portable Service artifact should preserve one task target");
    };
    assert!(
        loaded.artifact().program().functions().any(|(_, code)| code
            .scalar_blocks
            .iter()
            .any(|plan| plan.range_loop.is_some())),
        "portable Service artifact should preserve selected physical coverage"
    );
    assert!(
        loaded
            .artifact()
            .required_features()
            .contains(vela_bytecode::ArtifactFeatureSet::host_scoped_tasks())
    );
    assert_eq!(
        task_target.operation,
        vela_bytecode::ArtifactTaskOperation::SpawnScoped
    );
    assert_eq!(task_target.worker_debug_name, "calculator::portable_worker");
    assert_eq!(
        task_target.service_requirement,
        vela_bytecode::ArtifactTaskServiceRequirement::InheritOriginatingGeneration
    );
    let report = services.dry_run_bundle(&base, &loaded);
    assert!(report.accepted());
    let candidate = services
        .stage_bundle(
            &base,
            loaded,
            ServiceRuntimeBinding::for_engine(engine.clone()),
            call_options(),
        )
        .expect("stage portable bundle");
    let linked_artifact_checksum = candidate
        .artifact_checksum()
        .expect("portable candidate should expose its linked artifact checksum");
    services
        .activate_if_current(candidate)
        .expect("activate portable bundle");
    assert_eq!(
        services.pin().artifact_checksum(),
        Some(linked_artifact_checksum)
    );

    let mut context = RequestContext { counter: 0 };
    assert_eq!(services.pin().calculator().adjust(&mut context, 2), 32);
    assert_eq!(context.counter, 5);
    assert_eq!(services.pin().audit().record(&mut context, 2), 52);
    assert_eq!(context.counter, 12);

    let active = services.pin();
    let delta_source = r#"
#[service_impl(test::calculator)]
impl CalculatorPortableDelta {
    fn adjust(context: RequestContext, value: i64) -> i64 {
        let total = 0;
        for item in 0..4 { total += item + 1 - 1; }
        return value + 40 + total - total;
    }
}
"#;
    let delta_sources =
        build_single_source(SourceId::new(93), delta_source).expect("valid Delta source");
    let delta_manifest = ServiceSourceManifest::link(delta_sources.graph(), services.schema())
        .expect("Delta service manifest");
    let delta_complete_source = format!("{audit_source}\n{delta_source}");
    let delta_program = vela_bytecode::PortableProgramArtifact::from_compiled(
        engine
            .compile_source(&delta_complete_source)
            .expect("offline Delta compile"),
    )
    .expect("portable Delta bytecode");
    let stale = vela_engine::service::PortableServiceUpdateBundle::delta(
        services.schema(),
        active.generation_id(),
        vela_bytecode::ArtifactChecksum::new([0x5a; 32]),
        delta_program.clone(),
        &delta_manifest,
        host_schema_hash,
        [],
    )
    .expect("structurally valid stale Delta");
    let stale = vela_engine::service::PortableServiceUpdateBundle::decode(
        &stale.encode().expect("encode stale Delta"),
    )
    .expect("decode stale Delta")
    .load(&engine, services.schema(), host_schema_hash)
    .expect("load stale Delta");
    assert!(!services.dry_run_bundle(&active, &stale).accepted());

    let delta = vela_engine::service::PortableServiceUpdateBundle::delta(
        services.schema(),
        active.generation_id(),
        active
            .artifact_checksum()
            .expect("portable Snapshot has an artifact"),
        delta_program,
        &delta_manifest,
        host_schema_hash,
        [vela_engine::service::PortableDiagnosticSource::new(
            "delta.vela",
            delta_source,
        )],
    )
    .expect("portable exact-base Delta");
    let delta = vela_engine::service::PortableServiceUpdateBundle::decode(
        &delta.encode().expect("encode exact-base Delta"),
    )
    .expect("decode exact-base Delta")
    .load(&engine, services.schema(), host_schema_hash)
    .expect("load exact-base Delta");
    assert!(artifact_has_selected_scalar_loop(delta.artifact()));
    assert!(services.dry_run_bundle(&active, &delta).accepted());
    let candidate = services
        .stage_bundle(
            &active,
            delta,
            ServiceRuntimeBinding::for_engine(engine.clone()),
            call_options(),
        )
        .expect("stage exact-base Delta");
    services
        .activate_if_current(candidate)
        .expect("activate exact-base Delta");
    assert_eq!(services.pin().calculator().adjust(&mut context, 2), 42);
}

#[cfg(feature = "artifact-codec")]
#[test]
fn portable_service_bundle_rejects_untyped_host_parameters_before_deployment() {
    let (engine, services) = service_app().into_parts();
    let source = r#"
#[service_impl(test::calculator)]
impl UntypedPortableHotfix {
    fn adjust(context, value) {
        context.counter += 5;
        return value + 30;
    }
}
"#;
    let sources = build_single_source(SourceId::new(92), source).expect("valid source");
    let manifest =
        ServiceSourceManifest::link(sources.graph(), services.schema()).expect("service manifest");
    let portable_program = vela_bytecode::PortableProgramArtifact::from_compiled(
        engine.compile_source(source).expect("offline compile"),
    )
    .expect("portable bytecode");

    let error = vela_engine::service::PortableServiceUpdateBundle::snapshot(
        services.schema(),
        portable_program,
        &manifest,
        7,
        [],
    )
    .expect_err("untyped Host parameters are not interpreter-safe");
    assert!(matches!(
        error,
        vela_engine::service::PortableServiceBundleError::UntypedHostParameter {
            ref parameter,
            ..
        } if parameter == "context"
    ));
}

fn call_options() -> CallOptions {
    CallOptions::new(100_000, 1024 * 1024, 64)
        .with_task_scope(crate::support::dropping_task_scope())
}
