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
    fn one(&self, context: &mut RequestContext, value: &Entry) -> i64;
    fn owned(&self, context: &mut RequestContext, values: Vec<Entry>) -> i64;
    fn mutate(&self, context: &mut RequestContext, values: &mut Vec<i64>) -> i64;
}

struct RustTotalService;

impl TotalService for RustTotalService {
    fn sum(&self, context: &mut RequestContext, values: &[Entry]) -> i64 {
        context.rust_calls += 1;
        values.iter().map(|value| value.amount).sum()
    }

    fn one(&self, context: &mut RequestContext, value: &Entry) -> i64 {
        context.rust_calls += 1;
        value.amount
    }

    fn owned(&self, context: &mut RequestContext, values: Vec<Entry>) -> i64 {
        context.rust_calls += 1;
        values.into_iter().map(|value| value.amount).sum()
    }

    fn mutate(&self, context: &mut RequestContext, values: &mut Vec<i64>) -> i64 {
        context.rust_calls += 1;
        values.push(13);
        values.iter().sum()
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
        let transformed = values.map(|value| value);
        return base.sum(context, transformed) + transformed.len();
    }

    fn one(context, value) {
        return base.one(context, value) + 2;
    }

    fn owned(context, values) {
        let transformed = values.map(|value| value);
        return base.owned(context, transformed) + transformed.len();
    }

    fn mutate(context, values) {
        return base.mutate(context, values);
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
        runtime: ServiceRuntimeSlot::new(engine.clone()),
        rust_calls: 0,
    };
    let values = [
        Entry { amount: 4 },
        Entry { amount: 7 },
        Entry { amount: 10 },
    ];

    assert_eq!(root.totals().sum(&mut context, &values), 24);
    assert_eq!(root.totals().one(&mut context, &values[1]), 9);
    assert_eq!(
        root.totals().owned(
            &mut context,
            vec![Entry { amount: 3 }, Entry { amount: 8 },],
        ),
        13
    );
    let mut mutable = Vec::with_capacity(4);
    mutable.extend([2_i64, 5_i64]);
    let address = mutable.as_ptr();
    assert_eq!(root.totals().mutate(&mut context, &mut mutable), 20);
    assert_eq!(mutable.as_ptr(), address);
    assert_eq!(mutable.last().copied(), Some(13));
    assert_eq!(context.rust_calls, 4);

    let invalid_copy_back = r#"
#[service_impl(slice_service::totals)]
impl InvalidCopyBack {
    fn sum(context, values) {
        return base.sum(context, values);
    }

    fn one(context, value) {
        return base.one(context, value);
    }

    fn owned(context, values) {
        return base.owned(context, values);
    }

    fn mutate(context, values) {
        let transformed = values.map(|value| value);
        return base.mutate(context, transformed);
    }
}
"#;
    let invalid_sources =
        build_single_source(SourceId::new(2), invalid_copy_back).expect("valid negative source");
    let invalid_manifest = ServiceSourceManifest::link(invalid_sources.graph(), services.schema())
        .expect("negative source manifest");
    let invalid_artifact = engine
        .link_compiled_program(
            engine
                .compile_source(invalid_copy_back)
                .expect("negative source compiles before representation validation"),
        )
        .expect("negative source links");
    let invalid_update = invalid_manifest
        .bind_artifact(invalid_artifact)
        .expect("negative artifact-bound update");
    let invalid_candidate = services
        .stage_snapshot(
            &root,
            invalid_update,
            ServiceRuntimeBinding::for_context::<RequestContext>(),
            CallOptions::unbounded(),
        )
        .expect("negative Snapshot stages");
    services
        .activate_if_current(invalid_candidate)
        .expect("negative Snapshot activates");
    let invalid_root = services.pin();
    let calls_before = context.rust_calls;
    let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        invalid_root.totals().mutate(&mut context, &mut mutable)
    }));
    assert!(
        failure.is_err(),
        "a script-owned transformed Array must not satisfy &mut Vec<i64>",
    );
    assert_eq!(
        context.rust_calls, calls_before,
        "mutable copy-back must fail before the authored Rust body executes",
    );
}
