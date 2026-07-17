use super::*;
use crate::interop::{
    CallableAccess, CallableIdentity, CallableKind, CallableLanguage, CallableOrigin,
    CallableParameter, CallableReturn, ErrorMode, ReturnMode,
};
use std::task::{Context, Poll, Waker};
use vela_common::{CallableAsyncness, SourceId, Span};
use vela_macros::{ScriptHost, ScriptReflect, export, methods, replaceable};

fn shared_dispatch_runtime(
    engine: crate::engine::Engine,
    program: vela_bytecode::compiler::CompiledProgram,
) -> SharedDispatchRuntime {
    let image = crate::runtime::RuntimeImage::new_compiled(engine, program).into_shared();
    shared_runtime_from_image(image)
}

fn shared_runtime_from_image(image: crate::runtime::SharedImage) -> SharedDispatchRuntime {
    Arc::new(Mutex::new(
        crate::runtime::SharedRuntime::from_shared_image(image).expect("shared runtime"),
    ))
}

#[derive(Debug, Eq, PartialEq)]
struct GameError(String);

type GameResult<T> = Result<T, GameError>;

impl From<vela_vm::error::VmError> for GameError {
    fn from(error: vela_vm::error::VmError) -> Self {
        Self(error.to_string())
    }
}

impl crate::args::IntoScriptArg for GameError {
    fn into_script_arg(self) -> vela_vm::owned_value::OwnedValue {
        crate::args::IntoScriptArg::into_script_arg(self.0)
    }
}

impl crate::args::FromScriptArg for GameError {
    const TYPE_NAME: &'static str = "game error";

    fn from_script_arg(value: &vela_vm::owned_value::OwnedValue) -> VmResult<Self> {
        String::from_script_arg(value).map(Self)
    }
}

impl crate::interop::VelaValueBoundary for GameError {
    fn vela_type_hint() -> TypeHint {
        TypeHint::string()
    }
}

#[derive(ScriptHost, ScriptReflect)]
#[script(path = "host::ActorContext")]
pub struct ActorContext {
    #[script(get, set)]
    calls: i64,
    #[script(skip)]
    root: DispatchRoot,
}

#[methods]
impl ActorContext {
    pub fn call_count(&self) -> i64 {
        self.calls
    }
}

#[derive(ScriptHost, ScriptReflect)]
#[script(path = "host::GameService")]
pub struct GameService {
    #[script(get)]
    offset: i64,
}

#[methods(path = "host::game::GameService")]
impl GameService {
    #[replaceable(
        path = "host::game::GameService::compute",
        authority = "context",
        index = 2
    )]
    pub fn compute(&self, context: &mut ActorContext, value: i64) -> VmResult<i64> {
        context.calls += 10;
        Ok(self.adjacent(value))
    }

    pub fn adjacent(&self, value: i64) -> i64 {
        value + self.offset
    }
}

impl DispatchAuthority for ActorContext {
    fn vela_dispatch_root(&self) -> &DispatchRoot {
        &self.root
    }
}

#[replaceable(path = "host::game::increment", authority = "context", index = 0)]
pub fn replaceable_increment(context: &mut ActorContext, value: i64) -> VmResult<i64> {
    context.calls += 10;
    Ok(value + 1)
}

#[replaceable(path = "host::game::increment_async", authority = "context", index = 1)]
pub async fn replaceable_increment_async(context: &mut ActorContext, value: i64) -> VmResult<i64> {
    context.calls += 10;
    Ok(value + 1)
}

#[replaceable(path = "host::game::plain", authority = "context", index = 0)]
pub fn replaceable_plain(context: &mut ActorContext, value: i64) -> i64 {
    context.calls += 10;
    value + 1
}

#[replaceable(path = "host::game::business", authority = "context", index = 1)]
pub fn replaceable_business(
    context: &mut ActorContext,
    value: GameResult<i64>,
    _divisor: i64,
) -> GameResult<i64> {
    context.calls += 10;
    value
}

#[replaceable(path = "host::game::borrow_context", authority = "context", index = 0)]
pub fn replaceable_borrow_context(context: &ActorContext) -> VmResult<&ActorContext> {
    Ok(context)
}

#[replaceable(path = "host::game::outer", authority = "context", index = 0)]
pub fn replaceable_outer(context: &mut ActorContext, value: i64) -> VmResult<i64> {
    context.calls += 10;
    Ok(value + 1)
}

#[replaceable(path = "host::game::inner", authority = "ctx", index = 1)]
pub fn replaceable_inner(
    ctx: &mut crate::context::NativeCallContext<'_, '_>,
    context: &mut ActorContext,
    value: i64,
) -> VmResult<i64> {
    let _ = ctx;
    context.calls += 10;
    Ok(value + 1)
}

#[export(path = "host::game::call_inner")]
pub fn call_inner(
    ctx: &mut crate::context::NativeCallContext<'_, '_>,
    context: &mut ActorContext,
    value: i64,
) -> VmResult<i64> {
    replaceable_inner(ctx, context, value)
}

#[replaceable(path = "host::game::outer_async", authority = "context", index = 0)]
pub async fn replaceable_outer_async(context: &mut ActorContext, value: i64) -> VmResult<i64> {
    context.calls += 10;
    Ok(value + 1)
}

#[replaceable(path = "host::game::inner_async", authority = "ctx", index = 1)]
pub async fn replaceable_inner_async(
    ctx: &mut crate::context::NativeCallContext<'_, '_>,
    value: i64,
) -> VmResult<i64> {
    let _ = ctx;
    Ok(value + 1)
}

#[export(path = "host::game::call_inner_async")]
pub async fn call_inner_async(
    ctx: &mut crate::context::NativeCallContext<'_, '_>,
    value: i64,
) -> VmResult<i64> {
    replaceable_inner_async(ctx, value).await
}

#[export(path = "host::game::pause_once")]
pub async fn pause_once(value: i64) -> VmResult<i64> {
    let mut pending = true;
    std::future::poll_fn(move |task| {
        if pending {
            pending = false;
            task.waker().wake_by_ref();
            Poll::Pending
        } else {
            Poll::Ready(Ok(value))
        }
    })
    .await
}

fn direct_helper(value: i64) -> i64 {
    value + 1
}

#[test]
fn staged_generation_pins_activation_and_rollback_per_root() {
    let slots = vec![slot(0, "host::math::increment")];
    let engine = crate::engine::Engine::builder()
        .register_replaceable_slots(slots.clone())
        .build()
        .expect("engine");
    let program = engine
        .compile_source(
            r#"
#[override(host::math::increment)]
pub fn patched(value: i64) -> i64 { return value + 2; }
"#,
        )
        .expect("override program");
    let linked_callable = program
        .binding_schema()
        .callables()
        .next()
        .expect("linked override callable");
    assert_eq!(
        linked_callable
            .override_target
            .as_ref()
            .and_then(vela_bytecode::RustBindingOverrideTarget::resolved),
        Some((slots[0].id, slots[0].contract.abi_fingerprint().get()))
    );
    let runtime = shared_dispatch_runtime(engine, program);
    let controller = DispatchController::new(slots).expect("controller");

    let before = DispatchRoot::pin(&controller, Arc::clone(&runtime)).expect("root");
    let candidate = controller
        .stage_current(&runtime)
        .expect("compatible override");
    let previous = controller.activate(candidate).expect("activate candidate");
    let active = DispatchRoot::pin(&controller, Arc::clone(&runtime)).expect("root");

    assert!(before.target(InterceptSlotIndex::new(0)).is_none());
    let target = active
        .target(InterceptSlotIndex::new(0))
        .expect("active override");
    assert_eq!(
        active
            .invocation()
            .call::<i64>(target.clone(), CallArgs::new().with(40_i64)),
        Ok(42)
    );

    controller.rollback(previous).expect("rollback generation");
    let rolled_back = DispatchRoot::pin(&controller, Arc::clone(&runtime)).expect("root");
    assert!(rolled_back.target(InterceptSlotIndex::new(0)).is_none());
    assert_eq!(active.target(InterceptSlotIndex::new(0)), Some(target));
}

#[test]
fn independent_roots_share_code_without_sharing_a_runtime_lock() {
    let slots = vec![slot(0, "host::math::increment")];
    let engine = crate::engine::Engine::builder()
        .register_replaceable_slots(slots.clone())
        .build()
        .expect("engine");
    let program = engine
        .compile_source(
            r#"
#[override(host::math::increment)]
pub fn patched(value: i64) -> i64 { return value + 2; }
"#,
        )
        .expect("override program");
    let image = crate::runtime::RuntimeImage::new_compiled(engine, program).into_shared();
    let first_runtime = shared_runtime_from_image(image.clone());
    let second_runtime = shared_runtime_from_image(image);
    let controller = DispatchController::new(slots).expect("controller");
    let candidate = controller
        .stage_current(&first_runtime)
        .expect("compatible override");
    controller.activate(candidate).expect("activate candidate");
    let first_root =
        DispatchRoot::pin(&controller, Arc::clone(&first_runtime)).expect("first root");
    let second_root =
        DispatchRoot::pin(&controller, Arc::clone(&second_runtime)).expect("second root");

    let _first_guard = first_runtime.lock();
    assert!(
        second_runtime.try_lock().is_some(),
        "roots over one immutable image must own independent runtime locks"
    );
    let target = second_root
        .target(InterceptSlotIndex::new(0))
        .expect("active target");
    assert_eq!(
        second_root
            .invocation()
            .call::<i64>(target, CallArgs::new().with(40_i64)),
        Ok(42)
    );
    assert_eq!(
        first_root.generation().id(),
        second_root.generation().id(),
        "independent runtime sessions still pin one dispatch generation"
    );
}

#[test]
fn same_shaped_controllers_reject_foreign_generations_and_candidates() {
    let slots = vec![slot(0, "host::math::increment")];
    let engine = crate::engine::Engine::builder()
        .register_replaceable_slots(slots.clone())
        .build()
        .expect("engine");
    let program = engine
        .compile_source(
            r#"
#[override(host::math::increment)]
pub fn patched(value: i64) -> i64 { return value + 2; }
"#,
        )
        .expect("override program");
    let runtime = shared_dispatch_runtime(engine, program);
    let first = DispatchController::new(slots.clone()).expect("first");
    let second = DispatchController::new(slots).expect("second");

    let foreign_base = first.current();
    let error = second
        .stage_from(&runtime, Arc::clone(&foreign_base))
        .expect_err("foreign base must be rejected");
    assert_eq!(error.code(), DispatchStageErrorCode::BaseLayoutMismatch);
    assert!(error.to_string().contains("another controller"));

    let foreign_candidate = first
        .stage_current(&runtime)
        .expect("first controller candidate");
    let error = second
        .activate(foreign_candidate)
        .expect_err("foreign candidate must be rejected");
    assert_eq!(error.code(), DispatchStageErrorCode::BaseLayoutMismatch);
    assert!(error.to_string().contains("another controller"));

    let error = second
        .rollback(foreign_base)
        .expect_err("foreign rollback generation must be rejected");
    assert_eq!(error.code(), DispatchStageErrorCode::BaseLayoutMismatch);
    assert!(error.to_string().contains("another controller"));
}

#[test]
fn compilation_links_targets_and_rejects_unknown_or_incompatible_overrides() {
    let slots = vec![slot(0, "host::math::increment")];
    let engine = crate::engine::Engine::builder()
        .register_replaceable_slots(slots.clone())
        .build()
        .expect("engine");
    for (source, expected) in [
        (
            r#"#[override(host::missing)] pub fn patched(value: i64) -> i64 { return value; }"#,
            "unknown replaceable target",
        ),
        (
            r#"#[override(host::math::increment)] pub fn patched(value: String) -> i64 { return 1; }"#,
            "parameter `value`",
        ),
    ] {
        let error = engine
            .compile_source(source)
            .expect_err("Engine compilation must reject invalid override");
        assert!(error.to_string().contains(expected), "{error}");
    }

    let linked_program = engine
        .compile_source(
            r#"#[override(host::math::increment)] pub fn patched(value: i64) -> i64 { return value; }"#,
        )
        .expect("linked override");
    let linked_runtime = shared_dispatch_runtime(engine.clone(), linked_program);
    let mut changed_contract = slots[0].clone();
    changed_contract.contract.returns.error_mode = ErrorMode::Value;
    let changed_controller =
        DispatchController::new(vec![changed_contract]).expect("changed controller");
    let error = changed_controller
        .stage_current(&linked_runtime)
        .expect_err("staging must reject a changed inherited contract");
    assert!(
        error
            .to_string()
            .contains("linked target contract fingerprint"),
        "{error}"
    );

    let program = engine
        .compile_source(
            r#"
#[override(host::math::increment)] pub fn first(value: i64) -> i64 { return value; }
#[override(host::math::increment)] pub fn second(value: i64) -> i64 { return value; }
"#,
        )
        .expect("duplicate override program links each declaration");
    let runtime = shared_dispatch_runtime(engine, program);
    let controller = DispatchController::new(slots).expect("controller");
    let error = controller
        .stage_current(&runtime)
        .expect_err("stage must reject duplicate override selection");
    assert!(error.to_string().contains("both target"), "{error}");
    assert!(error.source().is_some());
}

#[test]
fn staging_accepts_effect_subsets_and_rejects_effect_expansion() {
    let write_slots = vec![ReplaceableSlotDescriptor::new(
        0,
        host_contract(
            "host::game::inspect",
            BoundaryMode::ExclusiveHost,
            EffectSet::host_write(),
        ),
    )];
    let write_ceiling_engine = crate::engine::Engine::builder()
        .register_host_type::<ActorContext>()
        .register_exports(ActorContext::vela_inherent_exports())
        .register_replaceable_slots(write_slots.clone())
        .capability(vela_common::Capability::HostRead)
        .capability(vela_common::Capability::HostWrite)
        .build()
        .expect("engine");
    let read_program = write_ceiling_engine
        .compile_source(
            r#"
#[override(host::game::inspect)]
pub fn inspect(context: ActorContext) -> i64 {
return context.call_count();
}
"#,
        )
        .expect("read-only override program");
    let linked_read = read_program
        .binding_schema()
        .callables()
        .next()
        .expect("linked read override");
    assert_eq!(
        linked_read.parameters[0].mode,
        RustBindingBoundaryMode::ExclusiveHost,
        "the target contract supplies the exact host parameter mode"
    );
    let read_runtime = shared_dispatch_runtime(write_ceiling_engine, read_program);
    let write_ceiling = DispatchController::new(write_slots).expect("write ceiling controller");
    write_ceiling
        .stage_current(&read_runtime)
        .expect("host-read implementation is within a host-write ceiling");

    let read_slots = vec![ReplaceableSlotDescriptor::new(
        0,
        host_contract(
            "host::game::inspect",
            BoundaryMode::SharedHost,
            EffectSet::host_read(),
        ),
    )];
    let read_ceiling_engine = crate::engine::Engine::builder()
        .register_host_type::<ActorContext>()
        .register_exports(ActorContext::vela_inherent_exports())
        .register_replaceable_slots(read_slots)
        .capability(vela_common::Capability::HostRead)
        .capability(vela_common::Capability::HostWrite)
        .build()
        .expect("engine");
    let error = read_ceiling_engine
        .compile_source(
            r#"
#[override(host::game::inspect)]
pub fn inspect(context: ActorContext) -> i64 {
context.calls += 1;
return context.calls;
}
"#,
        )
        .expect_err("host-write implementation must exceed a host-read ceiling");
    assert!(error.to_string().contains("effective effects"), "{error}");
}

#[test]
fn compilation_imports_borrowed_return_contract_before_staging() {
    let slot = borrowed_host_contract("host::game::borrow_context");
    let slots = vec![ReplaceableSlotDescriptor::new(0, slot.clone())];
    let engine = crate::engine::Engine::builder()
        .register_host_type::<ActorContext>()
        .register_replaceable_slots(slots.clone())
        .capability(vela_common::Capability::HostRead)
        .build()
        .expect("engine");
    let program = engine
        .compile_source(
            r#"
#[override(host::game::borrow_context)]
pub fn borrow_context(context: ActorContext) -> ActorContext {
return context;
}
"#,
        )
        .expect("borrowed-return override links");
    let callable = program
        .binding_schema()
        .callables()
        .next()
        .expect("borrowed-return callable");
    assert_eq!(
        callable.parameters[0].mode,
        RustBindingBoundaryMode::SharedHost
    );
    assert_eq!(
        callable.returns.mode,
        vela_bytecode::RustBindingReturnMode::ScopedHost {
            origin: vela_bytecode::RustBindingBorrowedReturnOrigin::Parameter(0),
            child_access: vela_bytecode::RustBindingScopedHostAccess::Shared,
            parent_freeze: vela_bytecode::RustBindingScopedHostAccess::Shared,
        }
    );
    assert_eq!(
        callable.returns.error_mode,
        vela_bytecode::RustBindingErrorMode::RuntimeResult
    );

    let runtime = shared_dispatch_runtime(engine, program);
    DispatchController::new(slots)
        .expect("controller")
        .stage_current(&runtime)
        .expect("staging accepts the imported borrowed-return contract");
}

#[test]
fn staged_delta_rebinds_selected_targets_to_one_coherent_artifact() {
    let slots = vec![slot(0, "host::math::first"), slot(1, "host::math::second")];
    let first_engine = crate::engine::Engine::builder()
        .register_replaceable_slots(slots.clone())
        .build()
        .expect("engine");
    let first_program = first_engine
        .compile_source(
            r#"
#[override(host::math::first)]
fn first(value: i64) -> i64 { return value + 1; }
"#,
        )
        .expect("first delta");
    let first_runtime = shared_dispatch_runtime(first_engine, first_program);
    let second_engine = crate::engine::Engine::builder()
        .register_replaceable_slots(slots.clone())
        .build()
        .expect("engine");
    let second_program = second_engine
        .compile_source(
            r#"
#[override(host::math::first)]
fn first(value: i64) -> i64 { return value + 1; }

#[override(host::math::second)]
fn second(value: i64) -> i64 { return value + 2; }
"#,
        )
        .expect("second delta");
    let second_runtime = shared_dispatch_runtime(second_engine, second_program);
    let controller = DispatchController::new(slots).expect("controller");

    let first = controller
        .stage_current(&first_runtime)
        .expect("first candidate");
    controller.activate(first).expect("activate first delta");
    let second = controller
        .stage_current(&second_runtime)
        .expect("second delta over first generation");
    controller.activate(second).expect("activate second delta");
    let root = DispatchRoot::pin(&controller, Arc::clone(&second_runtime)).expect("root");
    let first_target = root
        .target(InterceptSlotIndex::new(0))
        .expect("preserved first target");
    let second_target = root
        .target(InterceptSlotIndex::new(1))
        .expect("new second target");

    assert_eq!(
        root.invocation()
            .call::<i64>(first_target, CallArgs::new().with(40_i64)),
        Ok(41)
    );
    assert_eq!(
        root.invocation()
            .call::<i64>(second_target, CallArgs::new().with(40_i64)),
        Ok(42)
    );
}

#[test]
fn macro_entry_intercepts_while_old_roots_and_private_fallback_stay_direct() {
    let slots = vec![
        vela_replaceable_slot_replaceable_increment(),
        vela_replaceable_slot_replaceable_increment_async(),
    ];
    let engine = crate::engine::Engine::builder()
        .register_host_type::<ActorContext>()
        .register_replaceable_slots(slots.clone())
        .capability(vela_common::Capability::HostRead)
        .capability(vela_common::Capability::HostWrite)
        .build()
        .expect("engine");
    let program = engine
        .compile_source(
            r#"
#[override(host::game::increment)]
pub fn patched(context: ActorContext, value: i64) -> i64 {
context.calls += 1;
return value + 2;
}

#[override(host::game::increment_async)]
pub async fn patched_async(context: ActorContext, value: i64) -> i64 {
context.calls += 1;
return value + 3;
}
"#,
        )
        .expect("override program");
    let runtime = shared_dispatch_runtime(engine, program);
    let controller = DispatchController::new(slots).expect("controller");
    let mut old_context = ActorContext {
        calls: 0,
        root: DispatchRoot::pin(&controller, Arc::clone(&runtime)).expect("root"),
    };
    assert_eq!(replaceable_increment(&mut old_context, 40), Ok(41));

    let candidate = controller.stage_current(&runtime).expect("override stage");
    controller.activate(candidate).expect("activate candidate");
    let mut active_context = ActorContext {
        calls: 0,
        root: DispatchRoot::pin(&controller, Arc::clone(&runtime)).expect("root"),
    };
    assert_eq!(replaceable_increment(&mut active_context, 40), Ok(42));
    assert_eq!(
        ready(replaceable_increment_async(&mut active_context, 40)),
        Ok(43)
    );
    assert_eq!(active_context.call_count(), 2);

    assert_eq!(replaceable_increment(&mut old_context, 40), Ok(41));
    assert_eq!(
        __vela_rust_replaceable_increment(&mut active_context, 40),
        Ok(41)
    );
    assert_eq!(direct_helper(40), 41);
    assert_eq!(old_context.calls, 20);
    assert_eq!(active_context.calls, 12);
}

#[test]
fn override_error_propagates_without_fallback_retry() {
    let slots = vec![
        vela_replaceable_slot_replaceable_increment(),
        vela_replaceable_slot_replaceable_increment_async(),
    ];
    let engine = crate::engine::Engine::builder()
        .register_host_type::<ActorContext>()
        .register_replaceable_slots(slots.clone())
        .capability(vela_common::Capability::HostRead)
        .capability(vela_common::Capability::HostWrite)
        .build()
        .expect("engine");
    let program = engine
        .compile_source(
            r#"
#[override(host::game::increment)]
pub fn broken(context: ActorContext, value: i64) -> i64 {
context.calls += 1;
return value / 0;
}
"#,
        )
        .expect("override program");
    let runtime = shared_dispatch_runtime(engine, program);
    let controller = DispatchController::new(slots).expect("controller");
    let candidate = controller.stage_current(&runtime).expect("override stage");
    controller.activate(candidate).expect("activate candidate");
    let mut context = ActorContext {
        calls: 0,
        root: DispatchRoot::pin(&controller, Arc::clone(&runtime)).expect("root"),
    };

    assert!(replaceable_increment(&mut context, 40).is_err());
    assert_eq!(context.calls, 1);
}

#[test]
fn replaceable_entries_preserve_plain_and_business_result_returns() {
    let slots = vec![
        vela_replaceable_slot_replaceable_plain(),
        vela_replaceable_slot_replaceable_business(),
    ];
    let engine = crate::engine::Engine::builder()
        .register_host_type::<ActorContext>()
        .register_replaceable_slots(slots.clone())
        .capability(vela_common::Capability::HostRead)
        .capability(vela_common::Capability::HostWrite)
        .build()
        .expect("engine");
    let program = engine
        .compile_source(
            r#"
#[override(host::game::plain)]
pub fn patched_plain(context: ActorContext, value: i64) -> i64 {
context.calls += 1;
return value + 2;
}

#[override(host::game::business)]
pub fn patched_business(
    context: ActorContext,
    value: Result<i64, String>,
    divisor: i64,
) -> Result<i64, String> {
context.calls += 1;
let checked = 1 / divisor;
return value;
}
"#,
        )
        .expect("override program");
    let failure_engine = engine.clone();
    let failure_slots = slots.clone();
    let runtime = shared_dispatch_runtime(engine, program);
    let controller = DispatchController::new(slots).expect("controller");
    let candidate = controller.stage_current(&runtime).expect("override stage");
    controller.activate(candidate).expect("activate candidate");
    let mut context = ActorContext {
        calls: 0,
        root: DispatchRoot::pin(&controller, Arc::clone(&runtime)).expect("root"),
    };

    assert_eq!(replaceable_plain(&mut context, 40), 42);
    assert_eq!(replaceable_business(&mut context, Ok(43), 1), Ok(43));
    assert_eq!(context.calls, 2);

    let failure_program = failure_engine
        .compile_source(
            r#"
#[override(host::game::business)]
pub fn patched_business(
    context: ActorContext,
    value: Result<i64, String>,
    divisor: i64,
) -> Result<i64, String> {
context.calls += 1;
let checked = 1 / divisor;
return value;
}
"#,
        )
        .expect("failing business override program");
    let failure_runtime = shared_dispatch_runtime(failure_engine, failure_program);
    let failure_controller = DispatchController::new(failure_slots).expect("failure controller");
    let candidate = failure_controller
        .stage_current(&failure_runtime)
        .expect("failure override stage");
    failure_controller
        .activate(candidate)
        .expect("activate failure candidate");
    let mut failure_context = ActorContext {
        calls: 0,
        root: DispatchRoot::pin(&failure_controller, Arc::clone(&failure_runtime))
            .expect("failure root"),
    };
    let error = replaceable_business(&mut failure_context, Ok(43), 0)
        .expect_err("VM failures map into the business error family");
    assert!(
        error.0.to_ascii_lowercase().contains("zero")
            || error.0.to_ascii_lowercase().contains("division"),
        "{error:?}"
    );
    assert_eq!(failure_context.calls, 1);
}

#[test]
fn replaceable_borrowed_return_reuses_the_proven_direct_origin() {
    let slots = vec![vela_replaceable_slot_replaceable_borrow_context()];
    let engine = crate::engine::Engine::builder()
        .register_host_type::<ActorContext>()
        .register_replaceable_slots(slots.clone())
        .capability(vela_common::Capability::HostRead)
        .build()
        .expect("engine");
    let program = engine
        .compile_source(
            r#"
#[override(host::game::borrow_context)]
pub fn borrow_context(context: ActorContext) -> ActorContext {
return context;
}
"#,
        )
        .expect("borrowed override program");
    let runtime = shared_dispatch_runtime(engine, program);
    let controller = DispatchController::new(slots).expect("controller");
    let candidate = controller.stage_current(&runtime).expect("override stage");
    controller.activate(candidate).expect("activate candidate");
    let context = ActorContext {
        calls: 0,
        root: DispatchRoot::pin(&controller, Arc::clone(&runtime)).expect("root"),
    };

    let borrowed = replaceable_borrow_context(&context).expect("borrowed origin");
    assert!(std::ptr::eq(borrowed, &context));
}

#[test]
fn nested_replaceable_call_reenters_the_active_runtime_session() {
    let slots = vec![
        vela_replaceable_slot_replaceable_outer(),
        vela_replaceable_slot_replaceable_inner(),
    ];
    let engine = crate::engine::Engine::builder()
        .register_host_type::<ActorContext>()
        .register_exports(vela_export_bundle_call_inner())
        .register_replaceable_slots(slots.clone())
        .capability(vela_common::Capability::HostRead)
        .capability(vela_common::Capability::HostWrite)
        .build()
        .expect("engine");
    let program = engine
        .compile_source(
            r#"
#[override(host::game::outer)]
pub fn outer(context: ActorContext, value: i64) -> i64 {
return host::game::call_inner(context, value);
}

#[override(host::game::inner)]
pub fn inner(context: ActorContext, value: i64) -> i64 {
context.calls += 1;
return value + 2;
}
"#,
        )
        .expect("nested override program");
    let runtime = shared_dispatch_runtime(engine, program);
    let controller = DispatchController::new(slots).expect("controller");
    let candidate = controller.stage_current(&runtime).expect("override stage");
    controller.activate(candidate).expect("activate candidate");
    let mut context = ActorContext {
        calls: 0,
        root: DispatchRoot::pin(&controller, Arc::clone(&runtime)).expect("root"),
    };

    assert_eq!(replaceable_outer(&mut context, 40), Ok(42));
    assert_eq!(context.calls, 1);
}

#[test]
fn nested_replaceable_call_consumes_the_root_remaining_budget() {
    let slots = vec![
        vela_replaceable_slot_replaceable_outer(),
        vela_replaceable_slot_replaceable_inner(),
    ];
    let engine = crate::engine::Engine::builder()
        .register_host_type::<ActorContext>()
        .register_exports(vela_export_bundle_call_inner())
        .register_replaceable_slots(slots.clone())
        .capability(vela_common::Capability::HostRead)
        .capability(vela_common::Capability::HostWrite)
        .build()
        .expect("engine");
    let program = engine
        .compile_source(
            r#"
#[override(host::game::outer)]
pub fn outer(context: ActorContext, value: i64) -> i64 {
return host::game::call_inner(context, value);
}

#[override(host::game::inner)]
pub fn inner(context: ActorContext, value: i64) -> i64 {
context.calls += 1;
let total = 0;
for current in 1..=1000 { total += current; }
return value + total;
}
"#,
        )
        .expect("budget override program");
    let runtime = shared_dispatch_runtime(engine, program);
    let controller = DispatchController::new(slots).expect("controller");
    let candidate = controller.stage_current(&runtime).expect("override stage");
    controller.activate(candidate).expect("activate candidate");
    let root = DispatchRoot::pin_with_options(
        &controller,
        Arc::clone(&runtime),
        CallOptions::new(200, usize::MAX, usize::MAX),
    )
    .expect("budgeted root");
    let mut context = ActorContext { calls: 0, root };

    let error = replaceable_outer(&mut context, 40)
        .expect_err("nested override must not receive a fresh default budget");
    assert!(
        matches!(
            error.kind(),
            vela_vm::error::VmErrorKind::BudgetExceeded {
                budget: vela_vm::budget::ExecutionBudgetKind::ExecutionUnits,
                ..
            }
        ),
        "{error}"
    );
    assert_eq!(
        context.calls, 1,
        "the outer call reached the nested override before sharing exhaustion"
    );
}

#[test]
fn nested_async_replaceable_calls_pin_generation_and_release_on_cancel() {
    let slots = vec![
        vela_replaceable_slot_replaceable_outer_async(),
        vela_replaceable_slot_replaceable_inner_async(),
    ];
    let engine = crate::engine::Engine::builder()
        .register_host_type::<ActorContext>()
        .register_exports(vela_export_bundle_call_inner_async())
        .register_exports(vela_export_bundle_pause_once())
        .register_replaceable_slots(slots.clone())
        .capability(vela_common::Capability::HostRead)
        .capability(vela_common::Capability::HostWrite)
        .build()
        .expect("engine");
    let first_program = engine
        .compile_source(
            r#"
#[override(host::game::outer_async)]
pub async fn outer(context: ActorContext, value: i64) -> i64 {
return host::game::call_inner_async(value).await;
}

#[override(host::game::inner_async)]
pub async fn inner(value: i64) -> i64 {
let paused = host::game::pause_once(value).await;
return paused + 2;
}
"#,
        )
        .expect("first async override program");
    let first_runtime = shared_dispatch_runtime(engine.clone(), first_program);
    let controller = DispatchController::new(slots).expect("controller");
    let candidate = controller
        .stage_current(&first_runtime)
        .expect("first override stage");
    controller.activate(candidate).expect("activate first");

    let cancelled_root =
        DispatchRoot::pin(&controller, Arc::clone(&first_runtime)).expect("cancelled root");
    let mut cancelled_context = ActorContext {
        calls: 0,
        root: cancelled_root,
    };
    {
        let mut future = std::pin::pin!(replaceable_outer_async(&mut cancelled_context, 40));
        let mut task = Context::from_waker(Waker::noop());
        assert!(matches!(future.as_mut().poll(&mut task), Poll::Pending));
    }
    assert_eq!(cancelled_context.calls, 0);
    assert!(
        first_runtime.try_lock().is_some(),
        "dropping a suspended root must release its runtime session"
    );

    let pinned_root =
        DispatchRoot::pin(&controller, Arc::clone(&first_runtime)).expect("pinned old root");
    let old_generation = pinned_root.generation().id();
    let mut pinned_context = ActorContext {
        calls: 0,
        root: pinned_root,
    };
    let old_result = {
        let mut future = std::pin::pin!(replaceable_outer_async(&mut pinned_context, 40));
        let mut task = Context::from_waker(Waker::noop());
        assert!(matches!(future.as_mut().poll(&mut task), Poll::Pending));

        let second_program = engine
            .compile_source(
                r#"
#[override(host::game::outer_async)]
pub async fn outer(context: ActorContext, value: i64) -> i64 {
return host::game::call_inner_async(value).await;
}

#[override(host::game::inner_async)]
pub async fn inner(value: i64) -> i64 {
let paused = host::game::pause_once(value).await;
return paused + 3;
}
"#,
            )
            .expect("second async override program");
        let second_runtime = shared_dispatch_runtime(engine, second_program);
        let candidate = controller
            .stage_current(&second_runtime)
            .expect("second override stage");
        controller.activate(candidate).expect("activate second");

        let old_result = loop {
            if let Poll::Ready(result) = future.as_mut().poll(&mut task) {
                break result;
            }
        };
        (old_result, second_runtime)
    };
    assert_eq!(old_result.0, Ok(42));
    assert_eq!(pinned_context.calls, 0);

    let new_root = DispatchRoot::pin(&controller, Arc::clone(&old_result.1)).expect("new root");
    assert_ne!(new_root.generation().id(), old_generation);
    let mut new_context = ActorContext {
        calls: 0,
        root: new_root,
    };
    assert_eq!(ready(replaceable_outer_async(&mut new_context, 40)), Ok(43));
    assert_eq!(new_context.calls, 0);
}

#[test]
fn replaceable_service_method_preserves_receiver_and_adjacent_rust_method() {
    let slots = vec![
        vela_replaceable_slot_replaceable_increment(),
        vela_replaceable_slot_replaceable_increment_async(),
        GameService::vela_replaceable_slot_compute(),
    ];
    let engine = crate::engine::Engine::builder()
        .register_host_type::<ActorContext>()
        .register_host_type::<GameService>()
        .register_exports(GameService::vela_inherent_exports())
        .register_replaceable_slots(slots.clone())
        .capability(vela_common::Capability::HostRead)
        .capability(vela_common::Capability::HostWrite)
        .build()
        .expect("engine");
    let program = engine
        .compile_source(
            r#"
#[override(host::game::GameService::compute)]
pub fn patched(service: GameService, context: ActorContext, value: i64) -> i64 {
context.calls += 1;
return service.adjacent(value) + 1;
}
"#,
        )
        .expect("service override program");
    let runtime = shared_dispatch_runtime(engine, program);
    let controller = DispatchController::new(slots).expect("controller");
    let service = GameService { offset: 1 };
    let mut old_context = ActorContext {
        calls: 0,
        root: DispatchRoot::pin(&controller, Arc::clone(&runtime)).expect("root"),
    };

    assert_eq!(service.compute(&mut old_context, 40), Ok(41));
    assert_eq!(service.adjacent(40), 41);

    let candidate = controller
        .stage_current(&runtime)
        .expect("service override stage");
    let previous = controller.activate(candidate).expect("activate candidate");
    let mut active_context = ActorContext {
        calls: 0,
        root: DispatchRoot::pin(&controller, Arc::clone(&runtime)).expect("root"),
    };

    assert_eq!(service.compute(&mut active_context, 40), Ok(42));
    assert_eq!(active_context.calls, 1);
    assert_eq!(service.adjacent(40), 41);
    assert_eq!(service.__vela_rust_compute(&mut active_context, 40), Ok(41));
    assert_eq!(active_context.calls, 11);
    assert_eq!(service.compute(&mut old_context, 40), Ok(41));

    controller.rollback(previous).expect("rollback generation");
    let mut rolled_back_context = ActorContext {
        calls: 0,
        root: DispatchRoot::pin(&controller, Arc::clone(&runtime)).expect("root"),
    };
    assert_eq!(service.compute(&mut rolled_back_context, 40), Ok(41));
    assert_eq!(rolled_back_context.calls, 10);
}

fn slot(index: usize, path: &str) -> ReplaceableSlotDescriptor {
    ReplaceableSlotDescriptor::new(index, contract(path))
}

fn contract(path: &str) -> CallableContract {
    CallableContract {
        identity: CallableIdentity::new(CallableKind::RustFunction, 1),
        public_path: path.to_owned(),
        parameters: vec![CallableParameter::new(
            1,
            "value",
            TypeHint::i64(),
            BoundaryMode::Value,
        )],
        returns: CallableReturn::new(
            TypeHint::i64(),
            ReturnMode::OwnedValue,
            ErrorMode::RuntimeResult,
        ),
        asyncness: CallableAsyncness::Sync,
        effects: EffectSet::pure(),
        access: CallableAccess::default(),
        docs: None,
        origin: CallableOrigin {
            language: CallableLanguage::Rust,
            source_span: Some(Span::new(SourceId::new(1), 0, 1)),
        },
    }
}

fn host_contract(path: &str, mode: BoundaryMode, effects: EffectSet) -> CallableContract {
    CallableContract {
        identity: CallableIdentity::new(CallableKind::RustFunction, 2),
        public_path: path.to_owned(),
        parameters: vec![CallableParameter::new(
            1,
            "context",
            TypeHint::Host(vela_reflect::registry::TypeKey::new(
                ActorContext::vela_type_id(),
                "ActorContext",
            )),
            mode,
        )],
        returns: CallableReturn::new(
            TypeHint::i64(),
            ReturnMode::OwnedValue,
            ErrorMode::RuntimeResult,
        ),
        asyncness: CallableAsyncness::Sync,
        effects,
        access: CallableAccess::default(),
        docs: None,
        origin: CallableOrigin {
            language: CallableLanguage::Rust,
            source_span: Some(Span::new(SourceId::new(1), 0, 1)),
        },
    }
}

fn borrowed_host_contract(path: &str) -> CallableContract {
    let host = TypeHint::Host(vela_reflect::registry::TypeKey::new(
        ActorContext::vela_type_id(),
        "ActorContext",
    ));
    CallableContract {
        identity: CallableIdentity::new(CallableKind::RustFunction, 3),
        public_path: path.to_owned(),
        parameters: vec![CallableParameter::new(
            1,
            "context",
            host.clone(),
            BoundaryMode::SharedHost,
        )],
        returns: CallableReturn::new(
            host,
            ReturnMode::ScopedHost {
                origin: crate::interop::BorrowedReturnOrigin::Parameter(0),
                child_access: crate::interop::ScopedHostAccess::Shared,
                parent_freeze: crate::interop::ScopedHostAccess::Shared,
            },
            ErrorMode::RuntimeResult,
        ),
        asyncness: CallableAsyncness::Sync,
        effects: EffectSet::host_read(),
        access: CallableAccess::default(),
        docs: None,
        origin: CallableOrigin {
            language: CallableLanguage::Rust,
            source_span: Some(Span::new(SourceId::new(1), 0, 1)),
        },
    }
}

fn ready<T>(future: impl Future<Output = T>) -> T {
    let mut future = std::pin::pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
    }
}
