use super::*;
use crate::interop::{
    CallableAccess, CallableIdentity, CallableKind, CallableLanguage, CallableOrigin,
    CallableParameter, CallableReturn, ErrorMode, ReturnMode,
};
use std::task::{Context, Poll, Waker};
use vela_common::{CallableAsyncness, SourceId, Span};
use vela_macros::{ScriptHost, ScriptReflect, methods, replaceable};

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
    let runtime = Arc::new(Mutex::new(Runtime::new(engine, program).expect("runtime")));
    let controller = DispatchController::new(slots).expect("controller");

    let before = DispatchRoot::pin(&controller);
    let candidate = controller
        .stage_current(&runtime)
        .expect("compatible override");
    let previous = controller.activate(candidate).expect("activate candidate");
    let active = DispatchRoot::pin(&controller);

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
    let rolled_back = DispatchRoot::pin(&controller);
    assert!(rolled_back.target(InterceptSlotIndex::new(0)).is_none());
    assert_eq!(active.target(InterceptSlotIndex::new(0)), Some(target));
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
    let runtime = Arc::new(Mutex::new(Runtime::new(engine, program).expect("runtime")));
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
    let linked_runtime = Arc::new(Mutex::new(
        Runtime::new(engine.clone(), linked_program).expect("linked runtime"),
    ));
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
    let runtime = Arc::new(Mutex::new(Runtime::new(engine, program).expect("runtime")));
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
    let read_runtime = Arc::new(Mutex::new(
        Runtime::new(write_ceiling_engine, read_program).expect("read runtime"),
    ));
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

    let runtime = Arc::new(Mutex::new(Runtime::new(engine, program).expect("runtime")));
    DispatchController::new(slots)
        .expect("controller")
        .stage_current(&runtime)
        .expect("staging accepts the imported borrowed-return contract");
}

#[test]
fn staged_delta_preserves_targets_and_their_owning_runtimes() {
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
    let first_runtime = Arc::new(Mutex::new(
        Runtime::new(first_engine, first_program).expect("first runtime"),
    ));
    let second_engine = crate::engine::Engine::builder()
        .register_replaceable_slots(slots.clone())
        .build()
        .expect("engine");
    let second_program = second_engine
        .compile_source(
            r#"
#[override(host::math::second)]
fn second(value: i64) -> i64 { return value + 2; }
"#,
        )
        .expect("second delta");
    let second_runtime = Arc::new(Mutex::new(
        Runtime::new(second_engine, second_program).expect("second runtime"),
    ));
    let controller = DispatchController::new(slots).expect("controller");

    let first = controller
        .stage_current(&first_runtime)
        .expect("first candidate");
    controller.activate(first).expect("activate first delta");
    let second = controller
        .stage_current(&second_runtime)
        .expect("second delta over first generation");
    controller.activate(second).expect("activate second delta");
    let root = DispatchRoot::pin(&controller);
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
    let runtime = Arc::new(Mutex::new(Runtime::new(engine, program).expect("runtime")));
    let controller = DispatchController::new(slots).expect("controller");
    let mut old_context = ActorContext {
        calls: 0,
        root: DispatchRoot::pin(&controller),
    };
    assert_eq!(replaceable_increment(&mut old_context, 40), Ok(41));

    let candidate = controller.stage_current(&runtime).expect("override stage");
    controller.activate(candidate).expect("activate candidate");
    let mut active_context = ActorContext {
        calls: 0,
        root: DispatchRoot::pin(&controller),
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
    let runtime = Arc::new(Mutex::new(Runtime::new(engine, program).expect("runtime")));
    let controller = DispatchController::new(slots).expect("controller");
    let candidate = controller.stage_current(&runtime).expect("override stage");
    controller.activate(candidate).expect("activate candidate");
    let mut context = ActorContext {
        calls: 0,
        root: DispatchRoot::pin(&controller),
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
    let runtime = Arc::new(Mutex::new(Runtime::new(engine, program).expect("runtime")));
    let controller = DispatchController::new(slots).expect("controller");
    let candidate = controller.stage_current(&runtime).expect("override stage");
    controller.activate(candidate).expect("activate candidate");
    let mut context = ActorContext {
        calls: 0,
        root: DispatchRoot::pin(&controller),
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
    let failure_runtime = Arc::new(Mutex::new(
        Runtime::new(failure_engine, failure_program).expect("failure runtime"),
    ));
    let failure_controller = DispatchController::new(failure_slots).expect("failure controller");
    let candidate = failure_controller
        .stage_current(&failure_runtime)
        .expect("failure override stage");
    failure_controller
        .activate(candidate)
        .expect("activate failure candidate");
    let mut failure_context = ActorContext {
        calls: 0,
        root: DispatchRoot::pin(&failure_controller),
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
    let runtime = Arc::new(Mutex::new(Runtime::new(engine, program).expect("runtime")));
    let controller = DispatchController::new(slots).expect("controller");
    let candidate = controller.stage_current(&runtime).expect("override stage");
    controller.activate(candidate).expect("activate candidate");
    let context = ActorContext {
        calls: 0,
        root: DispatchRoot::pin(&controller),
    };

    let borrowed = replaceable_borrow_context(&context).expect("borrowed origin");
    assert!(std::ptr::eq(borrowed, &context));
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
    let runtime = Arc::new(Mutex::new(Runtime::new(engine, program).expect("runtime")));
    let controller = DispatchController::new(slots).expect("controller");
    let service = GameService { offset: 1 };
    let mut old_context = ActorContext {
        calls: 0,
        root: DispatchRoot::pin(&controller),
    };

    assert_eq!(service.compute(&mut old_context, 40), Ok(41));
    assert_eq!(service.adjacent(40), 41);

    let candidate = controller
        .stage_current(&runtime)
        .expect("service override stage");
    let previous = controller.activate(candidate).expect("activate candidate");
    let mut active_context = ActorContext {
        calls: 0,
        root: DispatchRoot::pin(&controller),
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
        root: DispatchRoot::pin(&controller),
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
