use std::error::Error;
use std::future::Future;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::task::{Context, Poll, Waker};

use vela_common::{HostConstructionLifetime, SourceId};
use vela_def::FunctionId;
use vela_engine::args::FromScriptArg;
use vela_engine::engine::Engine;
use vela_engine::native::{EffectSet, NativeFunctionDesc, TypeHint};
use vela_engine::permission::{Capability, CapabilitySet};
use vela_engine::runtime::{CallOptions, Runtime, RuntimeBuildError};
use vela_engine::service::{
    LinkedServiceSourceManifest, PatchEdit, Service, ServiceRuntimeAuthority, ServiceRuntimeSlot,
    ServiceSourceManifest, ServiceUpdateBundle,
};
use vela_engine::type_binding::TypeBinding;
use vela_hir::source_ingestion::build_single_source;
use vela_macros::{ScriptHost, Value, service, service_domain};
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::owned_value::OwnedValue;

const SNAPSHOT_SOURCE: &str = include_str!("snapshot.vela");
const DELTA_POLICY_SOURCE: &str = include_str!("delta_policy.vela");
const DELTA_APPLY_SOURCE: &str = include_str!("delta_apply.vela");

static ROW_CLONES: AtomicUsize = AtomicUsize::new(0);
static ROW_CODEC_ENTRIES: AtomicUsize = AtomicUsize::new(0);
static PATCH_BUFFER_DROPS: AtomicUsize = AtomicUsize::new(0);

type ServiceResult<T> = Result<T, ServiceError>;

#[derive(Clone, Debug, Eq, PartialEq, Value)]
#[vela(path = "coverage::ServiceError")]
pub struct ServiceError {
    message: String,
}

impl ServiceError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ServiceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for ServiceError {}

#[derive(Debug, ScriptHost)]
#[vela(path = "coverage::Row")]
pub struct Row {
    #[vela(get)]
    key: i64,
    #[vela(get)]
    score: i64,
}

#[derive(ScriptHost)]
#[vela(path = "coverage::Table")]
pub struct Table {
    #[vela(skip)]
    rows: Vec<Row>,
}

#[derive(Clone, Debug, Value)]
#[vela(path = "coverage::Request")]
pub struct Request {
    key: i64,
    adjustment: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Value)]
#[vela(path = "coverage::Response")]
pub struct Response {
    key: i64,
    score: i64,
    applied: i64,
    audits: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Value)]
#[vela(path = "coverage::ValueRow")]
pub struct ValueRow {
    amount: i64,
}

#[derive(ScriptHost)]
#[vela(path = "coverage::PatchBuffer")]
pub struct PatchBuffer {
    #[vela(get, set)]
    value: i64,
}

impl Drop for PatchBuffer {
    fn drop(&mut self) {
        PATCH_BUFFER_DROPS.fetch_add(1, Ordering::SeqCst);
    }
}

fn patch_buffer_binding() -> TypeBinding<PatchBuffer> {
    PatchBuffer::vela_type_binding().host_constructor_fn(
        HostConstructionLifetime::CallScoped,
        NativeFunctionDesc::new(
            "PatchBuffer::new",
            FunctionId::new(
                vela_common::stable_id("coverage_constructor", "coverage::PatchBuffer", "new")
                    .into(),
            ),
        )
        .param("value", TypeHint::i64())
        .returns(TypeHint::Host(
            PatchBuffer::vela_host_type_desc().key.clone(),
        ))
        .effects(EffectSet::pure()),
        construct_patch_buffer,
    )
}

fn construct_patch_buffer(
    arguments: &[OwnedValue],
    _host: &mut vela_vm::HostExecution<'_>,
) -> VmResult<PatchBuffer> {
    let [value] = arguments else {
        return Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "coverage::PatchBuffer::new arguments",
        }));
    };
    Ok(PatchBuffer {
        value: i64::from_script_arg(value)?,
    })
}

#[derive(ScriptHost)]
#[vela(path = "coverage::RequestState")]
pub struct RequestState {
    #[vela(get)]
    marker: i64,
    #[vela(skip)]
    services: CoverageServicesRoot,
    #[vela(skip)]
    runtime: ServiceRuntimeSlot,
    #[vela(skip)]
    applied: i64,
    #[vela(skip)]
    audits: Vec<i64>,
    #[vela(skip)]
    rust_copyback_calls: usize,
}

impl ServiceRuntimeAuthority for RequestState {
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

#[service(path = "coverage::lookup")]
pub trait LookupService: Send + Sync {
    fn get<'borrow>(
        &self,
        context: &'borrow mut RequestState,
        present: bool,
    ) -> Option<&'borrow RequestState>;

    fn checked<'borrow>(
        &self,
        context: &'borrow mut RequestState,
        allowed: bool,
    ) -> Result<&'borrow RequestState, ServiceError>;

    fn required<'borrow>(&self, context: &'borrow mut RequestState) -> &'borrow RequestState;
}

pub struct RustLookupService;

impl LookupService for RustLookupService {
    fn get<'borrow>(
        &self,
        context: &'borrow mut RequestState,
        present: bool,
    ) -> Option<&'borrow RequestState> {
        present.then_some(&*context)
    }

    fn checked<'borrow>(
        &self,
        context: &'borrow mut RequestState,
        allowed: bool,
    ) -> Result<&'borrow RequestState, ServiceError> {
        allowed
            .then_some(&*context)
            .ok_or_else(|| ServiceError::new("blocked"))
    }

    fn required<'borrow>(&self, context: &'borrow mut RequestState) -> &'borrow RequestState {
        &*context
    }
}

#[service(path = "coverage::policy")]
pub trait PolicyService: Send + Sync {
    fn score(&self, context: &mut RequestState, row: &Row, adjustment: i64) -> ServiceResult<i64>;
}

pub struct RustPolicyService;

impl PolicyService for RustPolicyService {
    fn score(&self, _context: &mut RequestState, row: &Row, adjustment: i64) -> ServiceResult<i64> {
        Ok(row.score + adjustment)
    }
}

#[service(path = "coverage::apply")]
pub trait ApplyService: Send + Sync {
    fn apply(&self, context: &mut RequestState, row: &Row, score: i64) -> ServiceResult<()>;
}

pub struct RustApplyService;

impl ApplyService for RustApplyService {
    fn apply(&self, context: &mut RequestState, _row: &Row, score: i64) -> ServiceResult<()> {
        context.applied += score;
        Ok(())
    }
}

#[service(path = "coverage::audit")]
pub trait AuditService: Send + Sync {
    fn record(&self, context: &mut RequestState, code: i64);
}

pub struct RustAuditService;

impl AuditService for RustAuditService {
    fn record(&self, context: &mut RequestState, code: i64) {
        context.audits.push(code);
    }
}

#[service(path = "coverage::transform")]
pub trait TransformService: Send + Sync {
    fn consume(&self, context: &mut RequestState, values: Vec<ValueRow>) -> i64;
    fn inspect(&self, context: &mut RequestState, values: &[ValueRow]) -> i64;
    fn inspect_buffer(&self, context: &mut RequestState, buffer: &PatchBuffer) -> i64;
    fn update_buffer(&self, context: &mut RequestState, buffer: &mut PatchBuffer, delta: i64);
    fn collections(&self, context: &mut RequestState, values: Vec<ValueRow>) -> i64;
    fn buffer(&self, context: &mut RequestState) -> i64;
    fn copyback(&self, context: &mut RequestState, values: &mut Vec<i64>) -> i64;
}

pub struct RustTransformService;

impl TransformService for RustTransformService {
    fn consume(&self, _context: &mut RequestState, values: Vec<ValueRow>) -> i64 {
        values.into_iter().map(|value| value.amount).sum()
    }

    fn inspect(&self, _context: &mut RequestState, values: &[ValueRow]) -> i64 {
        values.iter().map(|value| value.amount).sum()
    }

    fn inspect_buffer(&self, _context: &mut RequestState, buffer: &PatchBuffer) -> i64 {
        buffer.value
    }

    fn update_buffer(&self, _context: &mut RequestState, buffer: &mut PatchBuffer, delta: i64) {
        buffer.value += delta;
    }

    fn collections(&self, _context: &mut RequestState, _values: Vec<ValueRow>) -> i64 {
        -1
    }

    fn buffer(&self, _context: &mut RequestState) -> i64 {
        -1
    }

    fn copyback(&self, context: &mut RequestState, values: &mut Vec<i64>) -> i64 {
        context.rust_copyback_calls += 1;
        values.push(99);
        values.iter().sum()
    }
}

#[service(path = "coverage::handler")]
pub trait HandlerService: Send + Sync {
    async fn handle(
        &self,
        context: &mut RequestState,
        table: &Table,
        request: Request,
    ) -> ServiceResult<Response>;
}

pub struct RustHandlerService;

impl HandlerService for RustHandlerService {
    async fn handle(
        &self,
        context: &mut RequestState,
        table: &Table,
        request: Request,
    ) -> ServiceResult<Response> {
        std::future::ready(()).await;
        let services = context.services.clone();
        let row = table
            .rows
            .iter()
            .find(|row| row.key == request.key)
            .ok_or_else(|| ServiceError::new("missing row"))?;
        let score = services.policy().score(context, row, request.adjustment)?;
        services.apply().apply(context, row, score)?;
        services.audit().record(context, score);
        Ok(Response {
            key: row.key,
            score,
            applied: context.applied,
            audits: i64::try_from(context.audits.len())
                .map_err(|_| ServiceError::new("audit count does not fit i64"))?,
        })
    }
}

#[service_domain(context = RequestState)]
pub struct CoverageServices {
    pub lookup: Service<dyn LookupService>,
    pub policy: Service<dyn PolicyService>,
    pub apply: Service<dyn ApplyService>,
    pub audit: Service<dyn AuditService>,
    pub transform: Service<dyn TransformService>,
    pub handler: Service<dyn HandlerService>,
}

fn table() -> Table {
    Table {
        rows: vec![Row { key: 1, score: 5 }, Row { key: 2, score: 8 }],
    }
}

fn state(engine: &Engine, root: &CoverageServicesRoot) -> RequestState {
    RequestState {
        marker: 1,
        services: root.clone(),
        runtime: ServiceRuntimeSlot::new(engine.clone()),
        applied: 0,
        audits: Vec::new(),
        rust_copyback_calls: 0,
    }
}

fn run_handler(
    root: &CoverageServicesRoot,
    engine: &Engine,
    table: &Table,
) -> ServiceResult<(Response, RequestState)> {
    let mut context = state(engine, root);
    let response = block_on(root.handler().handle(
        &mut context,
        table,
        Request {
            key: 1,
            adjustment: 2,
        },
    ))?;
    Ok((response, context))
}

fn linked_update(
    engine: &Engine,
    services: &CoverageServices,
    source_id: u32,
    manifest_source: &str,
    artifact_source: &str,
) -> Result<
    (
        Arc<vela_bytecode::LinkedArtifact>,
        LinkedServiceSourceManifest,
    ),
    Box<dyn Error>,
> {
    let sources = build_single_source(SourceId::new(source_id), manifest_source)
        .map_err(|error| format!("{error:?}"))?;
    let manifest = ServiceSourceManifest::link(sources.graph(), services.schema())?;
    let artifact = engine.link_compiled_program(engine.compile_source(artifact_source)?)?;
    let update = manifest.bind_artifact(Arc::clone(&artifact))?;
    Ok((artifact, update))
}

fn call_options() -> CallOptions {
    CallOptions::new(250_000, 4 * 1024 * 1024, 128)
}

fn block_on<T>(future: impl Future<Output = T>) -> T {
    let mut future = std::pin::pin!(future);
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

fn snapshot_without_initial_policy() -> Result<String, Box<dyn Error>> {
    const BEGIN: &str = "// BEGIN_INITIAL_POLICY";
    const END: &str = "// END_INITIAL_POLICY";
    let begin = SNAPSHOT_SOURCE
        .find(BEGIN)
        .ok_or("snapshot initial-policy begin marker is missing")?;
    let tail = &SNAPSHOT_SOURCE[begin..];
    let relative_end = tail
        .find(END)
        .ok_or("snapshot initial-policy end marker is missing")?;
    let end = begin + relative_end + END.len();
    Ok(format!(
        "{}\n{}",
        &SNAPSHOT_SOURCE[..begin],
        &SNAPSHOT_SOURCE[end..],
    ))
}

fn folded_source() -> Result<String, Box<dyn Error>> {
    Ok(format!(
        "{}\n{}\n{}",
        snapshot_without_initial_policy()?,
        DELTA_POLICY_SOURCE,
        DELTA_APPLY_SOURCE,
    ))
}

fn main() -> Result<(), Box<dyn Error>> {
    ROW_CLONES.store(0, Ordering::SeqCst);
    ROW_CODEC_ENTRIES.store(0, Ordering::SeqCst);
    PATCH_BUFFER_DROPS.store(0, Ordering::SeqCst);

    let app = CoverageServices::builder(
        Engine::builder()
            .capabilities(
                CapabilitySet::new()
                    .with(Capability::HostRead)
                    .with(Capability::HostWrite),
            )
            .register_type::<Row>()
            .register_type::<Table>()
            .register_type::<RequestState>()
            .register_type_binding::<PatchBuffer>(patch_buffer_binding()),
    )
    .lookup(RustLookupService)
    .policy(RustPolicyService)
    .apply(RustApplyService)
    .audit(RustAuditService)
    .transform(RustTransformService)
    .handler(RustHandlerService)
    .actor_runtime::<RequestState>()
    .call_options(call_options())
    .build()?;
    let engine = app.engine().clone();
    let services = app.domain();
    let table = table();

    let rust_root = services.pin();
    let (rust_response, rust_state) = run_handler(&rust_root, &engine, &table)?;
    assert_eq!(
        rust_response,
        Response {
            key: 1,
            score: 7,
            applied: 7,
            audits: 1,
        }
    );
    println!(
        "service_hotfix_coverage rust-default score={} applied={} audits={}",
        rust_response.score,
        rust_state.applied,
        rust_state.audits.len(),
    );

    app.patches()
        .apply(PatchEdit::put("snapshot.vela", SNAPSHOT_SOURCE))?;
    let snapshot_root = services.pin();

    let mut lookup_state = state(&engine, &snapshot_root);
    let expected_state_address = &mut lookup_state as *mut RequestState as usize;
    let some_address = snapshot_root
        .lookup()
        .get(&mut lookup_state, true)
        .map(|state| state as *const RequestState as usize);
    assert_eq!(some_address, Some(expected_state_address));
    let none = snapshot_root
        .lookup()
        .get(&mut lookup_state, false)
        .is_none();
    let checked_address = snapshot_root
        .lookup()
        .checked(&mut lookup_state, true)
        .map(|state| state as *const RequestState as usize)?;
    assert_eq!(checked_address, expected_state_address);
    let checked_error = match snapshot_root.lookup().checked(&mut lookup_state, false) {
        Ok(_) => return Err("blocked lookup unexpectedly returned a Host reference".into()),
        Err(error) => error,
    };
    assert_eq!(checked_error.message, "blocked");
    assert_eq!(
        snapshot_root.lookup().required(&mut lookup_state) as *const RequestState as usize,
        expected_state_address
    );
    let (snapshot_response, snapshot_state) = run_handler(&snapshot_root, &engine, &table)?;
    assert_eq!(snapshot_response.score, 18);
    assert_eq!(snapshot_state.applied, 17);
    println!(
        "service_hotfix_coverage snapshot some={} none={} checked_err={} score={}",
        lookup_state.marker, none, checked_error.message, snapshot_response.score,
    );
    println!(
        "service_hotfix_coverage nested shared={} exclusive-write={}",
        table.rows.len(),
        snapshot_state.applied,
    );

    let policy_complete_source = format!(
        "{}\n{}",
        snapshot_without_initial_policy()?,
        DELTA_POLICY_SOURCE,
    );
    let (policy_artifact, policy_update) = linked_update(
        &engine,
        services,
        202,
        DELTA_POLICY_SOURCE,
        &policy_complete_source,
    )?;
    let policy_bundle = ServiceUpdateBundle::delta(
        services.schema(),
        snapshot_root.generation_id(),
        snapshot_root
            .artifact_checksum()
            .ok_or("snapshot artifact checksum is missing")?,
        policy_artifact,
        policy_update,
    )?;
    app.patches().stage_bundle(policy_bundle)?.activate()?;
    let policy_root = services.pin();
    let (policy_response, _) = run_handler(&policy_root, &engine, &table)?;
    assert_eq!(policy_response.score, 28);
    println!(
        "service_hotfix_coverage delta-1 score={}",
        policy_response.score
    );

    let complete_source = folded_source()?;
    let (apply_artifact, apply_update) =
        linked_update(&engine, services, 203, DELTA_APPLY_SOURCE, &complete_source)?;
    let apply_bundle = ServiceUpdateBundle::delta(
        services.schema(),
        policy_root.generation_id(),
        policy_root
            .artifact_checksum()
            .ok_or("policy artifact checksum is missing")?,
        apply_artifact,
        apply_update,
    )?;
    app.patches().stage_bundle(apply_bundle)?.activate()?;
    let complete_root = services.pin();
    let (complete_response, complete_state) = run_handler(&complete_root, &engine, &table)?;
    assert_eq!(complete_response.score, 28);
    assert_eq!(complete_state.applied, 27);
    assert_eq!(complete_state.audits, [127, 27]);
    println!(
        "service_hotfix_coverage delta-2 score={} applied={} audits={}",
        complete_response.score,
        complete_state.applied,
        complete_state.audits.len(),
    );

    let (old_response, _) = run_handler(&snapshot_root, &engine, &table)?;
    assert_eq!(old_response.score, 18);
    println!(
        "service_hotfix_coverage old-root score={}",
        old_response.score
    );

    let (stale_artifact, stale_update) = linked_update(
        &engine,
        services,
        204,
        DELTA_POLICY_SOURCE,
        &policy_complete_source,
    )?;
    let stale_bundle = ServiceUpdateBundle::delta(
        services.schema(),
        snapshot_root.generation_id(),
        snapshot_root
            .artifact_checksum()
            .ok_or("snapshot artifact checksum is missing")?,
        stale_artifact,
        stale_update,
    )?;
    let stale_rejected = !services
        .dry_run_bundle(&complete_root, &stale_bundle)
        .accepted();
    let incompatible_source = r#"
#[service_impl(coverage::policy)]
impl IncompatiblePolicy {
    fn score(context, row) {
        return Result::Ok(row.score);
    }
}
"#;
    let incompatible_complete_source = format!(
        "{}\n{}\n{}",
        snapshot_without_initial_policy()?,
        incompatible_source,
        DELTA_APPLY_SOURCE,
    );
    let incompatible_attempt = (|| -> Result<bool, Box<dyn Error>> {
        let (artifact, update) = linked_update(
            &engine,
            services,
            206,
            incompatible_source,
            &incompatible_complete_source,
        )?;
        let bundle = ServiceUpdateBundle::delta(
            services.schema(),
            complete_root.generation_id(),
            complete_root
                .artifact_checksum()
                .ok_or("complete artifact checksum is missing")?,
            artifact,
            update,
        )?;
        Ok(!services.dry_run_bundle(&complete_root, &bundle).accepted())
    })();
    let abi_rejected = incompatible_attempt.unwrap_or(true);
    assert!(stale_rejected);
    assert!(abi_rejected);
    println!(
        "service_hotfix_coverage rejected stale={} abi={}",
        stale_rejected, abi_rejected,
    );

    let folded_source = complete_source;
    let (folded_artifact, folded_update) =
        linked_update(&engine, services, 205, &folded_source, &folded_source)?;
    let folded_bundle =
        ServiceUpdateBundle::snapshot(services.schema(), folded_artifact, folded_update)?;
    let rollback = app.patches().stage_bundle(folded_bundle)?.activate()?;
    let folded_root = services.pin();
    let (folded_response, folded_state) = run_handler(&folded_root, &engine, &table)?;
    assert_eq!(folded_response, complete_response);
    assert_eq!(folded_state.audits, complete_state.audits);
    println!(
        "service_hotfix_coverage folded score={}",
        folded_response.score
    );

    let committed_effect = folded_state.applied;
    let restored = app.patches().rollback(rollback)?;
    assert_eq!(restored.generation_id(), complete_root.generation_id());
    assert_eq!(folded_state.applied, committed_effect);
    let (rollback_response, _) = run_handler(&restored, &engine, &table)?;
    assert_eq!(rollback_response, complete_response);
    println!(
        "service_hotfix_coverage rollback score={} effects=preserved",
        rollback_response.score
    );

    assert_eq!(ROW_CLONES.load(Ordering::SeqCst), 0);
    assert_eq!(ROW_CODEC_ENTRIES.load(Ordering::SeqCst), 0);
    println!(
        "service_hotfix_coverage zero-copy clones={} codecs={}",
        ROW_CLONES.load(Ordering::SeqCst),
        ROW_CODEC_ENTRIES.load(Ordering::SeqCst),
    );

    let mut transform_state = state(&engine, &restored);
    let buffer_result = restored.transform().buffer(&mut transform_state);
    assert_eq!(buffer_result, 712);
    assert_eq!(PATCH_BUFFER_DROPS.load(Ordering::SeqCst), 1);
    println!(
        "service_hotfix_coverage construct shared=7 exclusive=12 reclaimed={}",
        PATCH_BUFFER_DROPS.load(Ordering::SeqCst) == 1,
    );

    let collection_result = restored.transform().collections(
        &mut transform_state,
        vec![
            ValueRow { amount: 1 },
            ValueRow { amount: 2 },
            ValueRow { amount: 4 },
        ],
    );
    assert_eq!(collection_result, 808);
    let mut mutable_values = vec![1_i64, 2_i64];
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let copyback_failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        restored
            .transform()
            .copyback(&mut transform_state, &mut mutable_values)
    }));
    std::panic::set_hook(previous_hook);
    assert!(copyback_failure.is_err());
    assert_eq!(transform_state.rust_copyback_calls, 0);
    assert_eq!(mutable_values, [1, 2]);
    println!(
        "service_hotfix_coverage collections owned=8 shared=8 mutable-copyback={}",
        copyback_failure.is_ok(),
    );

    Ok(())
}
