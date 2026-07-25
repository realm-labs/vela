use std::sync::Arc;

use vela_common::SourceId;
use vela_engine::engine::Engine;
use vela_engine::permission::{Capability, CapabilitySet};
use vela_engine::runtime::{CallOptions, Runtime, RuntimeBuildError};
use vela_engine::service::{
    LinkedServiceSourceManifest, ServiceMethodSelection, ServiceRuntimeAuthority,
    ServiceRuntimeBinding, ServiceRuntimeSlot, ServiceSourceManifest,
};
use vela_hir::source_ingestion::build_single_source;
use vela_macros::{ScriptHost, Value, service, service_set};

#[derive(Clone, Debug, Eq, PartialEq, Value)]
#[script(path = "interop::PatchCommand")]
pub struct PatchCommand {
    pub delta: i64,
    pub label: String,
}

#[derive(ScriptHost)]
#[script(path = "interop::RequestContext")]
pub struct RequestContext {
    #[script(skip)]
    runtime: ServiceRuntimeSlot,
    #[script(skip)]
    expected_values_address: usize,
    #[script(skip)]
    rust_inventory_calls: usize,
    #[script(skip)]
    rust_audit_calls: usize,
    #[script(skip)]
    rust_combine_calls: usize,
    #[script(skip)]
    observed_labels: Vec<String>,
    #[script(skip)]
    borrowed_values: Vec<i64>,
    #[script(skip)]
    borrowed_return_calls: usize,
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

#[service(path = "interop::inventory")]
pub trait InventoryService: Send + Sync {
    fn apply(
        &self,
        context: &mut RequestContext,
        values: &mut Vec<i64>,
        command: PatchCommand,
    ) -> i64;

    fn conflict(&self, context: &mut RequestContext, values: &mut Vec<i64>) -> i64;

    fn values<'borrow>(&self, context: &'borrow mut RequestContext) -> &'borrow mut Vec<i64>;

    fn borrowed_chain(&self, context: &mut RequestContext) -> i64;
}

pub struct RustInventoryService;

impl InventoryService for RustInventoryService {
    fn apply(
        &self,
        context: &mut RequestContext,
        values: &mut Vec<i64>,
        command: PatchCommand,
    ) -> i64 {
        assert_eq!(
            values as *mut Vec<i64> as usize,
            context.expected_values_address
        );
        context.rust_inventory_calls += 1;
        context.observed_labels.push(command.label);
        values.push(command.delta);
        values.iter().sum()
    }

    fn conflict(&self, _context: &mut RequestContext, _values: &mut Vec<i64>) -> i64 {
        unreachable!("the Vela conflict patch replaces this method")
    }

    fn values<'borrow>(&self, context: &'borrow mut RequestContext) -> &'borrow mut Vec<i64> {
        context.borrowed_return_calls += 1;
        &mut context.borrowed_values
    }

    fn borrowed_chain(&self, _context: &mut RequestContext) -> i64 {
        unreachable!("the Vela borrowed-return patch replaces this method")
    }
}

#[service(path = "interop::audit")]
pub trait AuditService: Send + Sync {
    fn record(
        &self,
        context: &mut RequestContext,
        values: &mut Vec<i64>,
        command: PatchCommand,
    ) -> i64;

    fn combine(
        &self,
        context: &mut RequestContext,
        left: &mut Vec<i64>,
        right: &mut Vec<i64>,
    ) -> i64;

    fn bump(&self, values: &mut Vec<i64>) -> i64;
}

pub struct RustAuditService;

impl AuditService for RustAuditService {
    fn record(
        &self,
        context: &mut RequestContext,
        values: &mut Vec<i64>,
        command: PatchCommand,
    ) -> i64 {
        assert_eq!(
            values as *mut Vec<i64> as usize,
            context.expected_values_address
        );
        context.rust_audit_calls += 1;
        context.observed_labels.push(command.label);
        values.push(command.delta);
        i64::try_from(values.len()).expect("fixture length fits i64")
    }

    fn combine(
        &self,
        context: &mut RequestContext,
        _left: &mut Vec<i64>,
        _right: &mut Vec<i64>,
    ) -> i64 {
        context.rust_combine_calls += 1;
        99
    }

    fn bump(&self, values: &mut Vec<i64>) -> i64 {
        values.push(6);
        values.iter().sum()
    }
}

#[service_set(context = RequestContext)]
pub struct InteropServices {
    #[vela::default(RustInventoryService)]
    pub inventory: dyn InventoryService,
    #[vela::default(RustAuditService)]
    pub audit: dyn AuditService,
}

#[test]
fn mixed_service_chain_preserves_custom_values_collection_identity_and_alias_preflight() {
    let engine = InteropServices::register_types(
        Engine::builder()
            .capabilities(CapabilitySet::new().with(Capability::HostWrite))
            .register_rust_type::<RequestContext>(RequestContext::vela_type_binding()),
    )
    .build()
    .expect("interop service engine");
    let services = InteropServices::new(&engine.type_bindings()).expect("interop service set");
    let initial = services.pin();
    let source = r#"
#[service_impl(interop::inventory)]
impl InventoryPatch {
    fn apply(context, values, command) {
        let base_command = interop::PatchCommand {
            delta: command.delta + 1,
            label: "inventory-base",
        };
        let base_sum = base.apply(context, values, base_command);
        let base_write = values[values.len() - 1];
        values.push(base_write + 2);
        let audit_command = interop::PatchCommand {
            delta: base_write + 4,
            label: "audit-rust",
        };
        let audit_len = services.audit.record(context, values, audit_command);
        return base_sum + base_write + audit_len + values[values.len() - 1];
    }

    fn conflict(context, values) {
        return services.audit.combine(context, values, values);
    }

    fn borrowed_chain(context) {
        let values = services.inventory.values(context);
        values.push(4);
        let sum = services.audit.bump(values);
        return sum + values.len();
    }

    fn values(context) {
        return base.values(context);
    }
}
"#;
    let snapshot = stage_snapshot(&engine, &services, &initial, source, SourceId::new(41));
    services
        .activate_if_current(snapshot)
        .expect("activate inventory Vela snapshot");
    let inventory_patch = services.pin();

    let mut values = vec![1_i64];
    let mut context = context(&engine, &mut values);
    assert_eq!(
        inventory_patch.inventory().apply(
            &mut context,
            &mut values,
            PatchCommand {
                delta: 2,
                label: "ignored-input".to_owned(),
            },
        ),
        18
    );
    assert_eq!(values, [1, 3, 5, 7]);
    assert_eq!(context.rust_inventory_calls, 1);
    assert_eq!(context.rust_audit_calls, 1);
    assert_eq!(
        context.observed_labels,
        ["inventory-base".to_owned(), "audit-rust".to_owned()]
    );
    assert_eq!(inventory_patch.inventory().borrowed_chain(&mut context), 15);
    assert_eq!(context.borrowed_values, [2, 4, 6]);
    assert_eq!(context.borrowed_return_calls, 1);
    context.borrowed_values.push(8);

    let delta_source = r#"
#[service_impl(interop::audit)]
impl AuditPatch {
    fn record(context, values, command) {
        values.push(command.delta + 10);
        let rust_len = base.record(context, values, command);
        return rust_len + values.len();
    }
}
"#;
    let delta = stage_delta(
        &engine,
        &services,
        &inventory_patch,
        source,
        delta_source,
        SourceId::new(42),
    );
    services
        .activate_if_current(delta)
        .expect("activate exact-base audit Delta");
    let complete_patch = services.pin();
    assert_eq!(
        complete_patch
            .selections()
            .expect("Vela generation has selections")
            .iter()
            .filter(|(_, selection)| matches!(selection, ServiceMethodSelection::Vela(_)))
            .count(),
        5
    );

    values.clear();
    values.push(1);
    context.expected_values_address = &mut values as *mut Vec<i64> as usize;
    context.rust_inventory_calls = 0;
    context.rust_audit_calls = 0;
    context.observed_labels.clear();
    assert_eq!(
        complete_patch.inventory().apply(
            &mut context,
            &mut values,
            PatchCommand {
                delta: 2,
                label: "ignored-input".to_owned(),
            },
        ),
        24
    );
    assert_eq!(values, [1, 3, 5, 17, 7]);
    assert_eq!(context.rust_inventory_calls, 1);
    assert_eq!(context.rust_audit_calls, 1);
    assert_eq!(
        context.observed_labels,
        ["inventory-base".to_owned(), "audit-rust".to_owned()]
    );

    let before = values.clone();
    let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        complete_patch
            .inventory()
            .conflict(&mut context, &mut values)
    }));
    assert!(failure.is_err());
    assert_eq!(values, before);
    assert_eq!(
        context.rust_combine_calls, 0,
        "duplicate exclusive aliases must fail before authored Rust executes"
    );

    values.clear();
    values.push(1);
    context.expected_values_address = &mut values as *mut Vec<i64> as usize;
    context.rust_inventory_calls = 0;
    context.rust_audit_calls = 0;
    assert_eq!(
        inventory_patch.inventory().apply(
            &mut context,
            &mut values,
            PatchCommand {
                delta: 2,
                label: "ignored-input".to_owned(),
            },
        ),
        18,
        "the old pinned root must retain its Rust audit selection"
    );
    assert_eq!(values, [1, 3, 5, 7]);
}

fn context(engine: &Engine, values: &mut Vec<i64>) -> RequestContext {
    RequestContext {
        runtime: ServiceRuntimeSlot::new(engine.clone()),
        expected_values_address: values as *mut Vec<i64> as usize,
        rust_inventory_calls: 0,
        rust_audit_calls: 0,
        rust_combine_calls: 0,
        observed_labels: Vec::new(),
        borrowed_values: vec![2],
        borrowed_return_calls: 0,
    }
}

fn stage_snapshot(
    engine: &Engine,
    services: &InteropServices,
    base: &InteropServicesRoot,
    source: &str,
    source_id: SourceId,
) -> InteropServicesCandidate {
    let sources = build_single_source(source_id, source).expect("valid snapshot source");
    let manifest =
        ServiceSourceManifest::link(sources.graph(), services.schema()).expect("snapshot schema");
    let compiled = engine
        .compile_source(source)
        .expect("compile snapshot source");
    let artifact = engine
        .link_compiled_program(compiled)
        .expect("link snapshot artifact");
    let update = manifest
        .bind_artifact(artifact)
        .expect("bind snapshot artifact");
    services
        .stage_snapshot(
            base,
            update,
            ServiceRuntimeBinding::for_context::<RequestContext>(),
            call_options(),
        )
        .expect("stage snapshot")
}

fn stage_delta(
    engine: &Engine,
    services: &InteropServices,
    base: &InteropServicesRoot,
    inherited_source: &str,
    delta_source: &str,
    source_id: SourceId,
) -> InteropServicesCandidate {
    let sources = build_single_source(source_id, delta_source).expect("valid Delta source");
    let manifest =
        ServiceSourceManifest::link(sources.graph(), services.schema()).expect("Delta schema");
    let complete_source = format!("{inherited_source}\n{delta_source}");
    let compiled = engine
        .compile_source(&complete_source)
        .expect("compile complete Delta source");
    let artifact = engine
        .link_compiled_program(compiled)
        .expect("link Delta artifact");
    let update = manifest
        .bind_artifact(artifact)
        .expect("bind Delta artifact");
    let audit = services
        .schema()
        .service("audit")
        .expect("audit service schema");
    let update = LinkedServiceSourceManifest::from_updates(
        update
            .into_updates()
            .into_iter()
            .filter(|update| update.key().service_id == audit.id()),
    )
    .expect("audit-only Delta");
    services
        .stage_delta(
            base,
            update,
            ServiceRuntimeBinding::for_context::<RequestContext>(),
            call_options(),
        )
        .expect("stage exact-base Delta")
}

fn call_options() -> CallOptions {
    CallOptions::new(100_000, 1024 * 1024, 64)
}
