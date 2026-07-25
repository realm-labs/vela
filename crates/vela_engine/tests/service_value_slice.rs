use std::sync::Arc;

use vela_common::SourceId;
use vela_engine::engine::Engine;
use vela_engine::permission::Capability;
use vela_engine::runtime::{CallOptions, Runtime, RuntimeBuildError};
use vela_engine::service::{
    ServiceRuntimeAuthority, ServiceRuntimeBinding, ServiceRuntimeSlot, ServiceSourceManifest,
};
use vela_hir::source_ingestion::build_single_source;
use vela_macros::{ScriptHost, Value, service, service_set};

#[derive(Clone, Debug, Value)]
#[script(path = "slice_service::Entry")]
pub struct Entry {
    amount: i64,
}

#[derive(ScriptHost)]
#[script(path = "slice_service::Context")]
pub struct RequestContext {
    #[script(skip)]
    runtime: ServiceRuntimeSlot,
    #[script(skip)]
    rust_calls: usize,
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

#[service(path = "slice_service::totals")]
pub trait TotalService: Send + Sync {
    fn sum(&self, context: &mut RequestContext, values: &[Entry]) -> i64;
}

struct RustTotalService;

impl TotalService for RustTotalService {
    fn sum(&self, context: &mut RequestContext, values: &[Entry]) -> i64 {
        context.rust_calls += 1;
        values.iter().map(|value| value.amount).sum()
    }
}

#[service_set(context = RequestContext)]
pub struct TestServices {
    #[vela::default(RustTotalService)]
    pub totals: dyn TotalService,
}

#[test]
fn same_generation_base_decodes_read_only_value_slice_for_rust_default() {
    let engine = TestServices::register_types(
        Engine::builder()
            .capability(Capability::HostWrite)
            .register_rust_type::<RequestContext>(RequestContext::vela_type_binding()),
    )
    .build()
    .expect("service engine");
    let services = TestServices::new(&engine.type_bindings()).expect("service set");
    let initial = services.pin();
    let source = r#"
#[service_impl(slice_service::totals)]
impl TotalPatch {
    fn sum(context, values) {
        return base.sum(context, values) + values.len();
    }
}
"#;
    let sources = build_single_source(SourceId::new(1), source).expect("valid service source");
    let manifest =
        ServiceSourceManifest::link(sources.graph(), services.schema()).expect("service manifest");
    let artifact = engine
        .link_compiled_program(engine.compile_source(source).expect("compiled source"))
        .expect("linked artifact");
    let update = manifest
        .bind_artifact(artifact)
        .expect("artifact-bound update");
    let candidate = services
        .stage_snapshot(
            &initial,
            update,
            ServiceRuntimeBinding::for_context::<RequestContext>(),
            CallOptions::unbounded(),
        )
        .expect("staged Snapshot");
    services
        .activate_if_current(candidate)
        .expect("activated Snapshot");

    let root = services.pin();
    let mut context = RequestContext {
        runtime: ServiceRuntimeSlot::new(engine),
        rust_calls: 0,
    };
    let values = [
        Entry { amount: 4 },
        Entry { amount: 7 },
        Entry { amount: 10 },
    ];

    assert_eq!(root.totals().sum(&mut context, &values), 24);
    assert_eq!(context.rust_calls, 1);
}
