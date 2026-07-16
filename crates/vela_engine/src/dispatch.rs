//! Optional immutable-generation dispatch for explicitly replaceable Rust entries.

use std::collections::BTreeMap;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::{Mutex, RwLock};
use vela_bytecode::{RustBindingBoundaryMode, RustBindingCallable, RustBindingType};
use vela_common::{DispatchGenerationId, InterceptSlotIndex, ReplaceableSlotId, stable_id};
use vela_def::FunctionId;

use crate::args::FromScriptArg;
use crate::binding::VmResult;
use crate::interop::{BoundaryMode, CallableContract};
use crate::native::TypeHint;
use crate::runtime::handles::StableVelaFunction;
use crate::runtime::{CallArgs, CallOptions, Runtime};

const DEFAULT_EXECUTION_UNITS: u64 = 1_000_000;
const DEFAULT_MEMORY_BYTES: usize = 8 * 1024 * 1024;
const DEFAULT_CALL_DEPTH: usize = 128;

pub type SharedDispatchRuntime = Arc<Mutex<Runtime>>;
pub type DispatchCallFuture<'call, T> = Pin<Box<dyn Future<Output = VmResult<T>> + Send + 'call>>;

#[derive(Clone, Debug)]
pub struct ReplaceableSlotDescriptor {
    pub id: ReplaceableSlotId,
    pub index: InterceptSlotIndex,
    pub contract: CallableContract,
}

impl ReplaceableSlotDescriptor {
    #[must_use]
    pub fn new(index: usize, contract: CallableContract) -> Self {
        Self {
            id: ReplaceableSlotId::new(u128::from(stable_id(
                "replaceable_slot",
                "",
                &contract.public_path,
            ))),
            index: InterceptSlotIndex::new(index),
            contract,
        }
    }
}

#[derive(Clone)]
pub struct VelaOverrideTarget {
    pub slot: ReplaceableSlotId,
    pub function: FunctionId,
    runtime: SharedDispatchRuntime,
}

impl fmt::Debug for VelaOverrideTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VelaOverrideTarget")
            .field("slot", &self.slot)
            .field("function", &self.function)
            .finish_non_exhaustive()
    }
}

impl PartialEq for VelaOverrideTarget {
    fn eq(&self, other: &Self) -> bool {
        self.slot == other.slot
            && self.function == other.function
            && Arc::ptr_eq(&self.runtime, &other.runtime)
    }
}

impl Eq for VelaOverrideTarget {}

#[derive(Clone, Debug)]
pub struct DispatchGeneration {
    id: DispatchGenerationId,
    targets: Box<[Option<VelaOverrideTarget>]>,
}

impl DispatchGeneration {
    #[must_use]
    pub const fn id(&self) -> DispatchGenerationId {
        self.id
    }

    #[must_use]
    pub fn target(&self, index: InterceptSlotIndex) -> Option<VelaOverrideTarget> {
        self.targets.get(index.get()).cloned().flatten()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.targets.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }
}

#[derive(Clone)]
pub struct DispatchController {
    inner: Arc<DispatchControllerInner>,
}

struct DispatchControllerInner {
    slots: Box<[ReplaceableSlotDescriptor]>,
    by_path: BTreeMap<String, usize>,
    current: RwLock<Arc<DispatchGeneration>>,
    next_generation: AtomicU64,
}

impl DispatchController {
    pub fn new(mut slots: Vec<ReplaceableSlotDescriptor>) -> Result<Self, DispatchStageError> {
        slots.sort_by_key(|slot| slot.index);
        let mut by_path = BTreeMap::new();
        for (expected, slot) in slots.iter().enumerate() {
            if slot.index.get() != expected {
                return Err(DispatchStageError::new(format!(
                    "replaceable slot indices must be dense from zero; expected {expected}, got {}",
                    slot.index.get()
                )));
            }
            if by_path
                .insert(slot.contract.public_path.clone(), expected)
                .is_some()
            {
                return Err(DispatchStageError::new(format!(
                    "duplicate replaceable slot path `{}`",
                    slot.contract.public_path
                )));
            }
        }
        let initial = Arc::new(DispatchGeneration {
            id: DispatchGenerationId::new(0),
            targets: vec![None; slots.len()].into_boxed_slice(),
        });
        Ok(Self {
            inner: Arc::new(DispatchControllerInner {
                slots: slots.into_boxed_slice(),
                by_path,
                current: RwLock::new(initial),
                next_generation: AtomicU64::new(1),
            }),
        })
    }

    #[must_use]
    pub fn current(&self) -> Arc<DispatchGeneration> {
        Arc::clone(&self.inner.current.read())
    }

    pub fn stage_current(
        &self,
        runtime: &SharedDispatchRuntime,
    ) -> Result<DispatchCandidate, DispatchStageError> {
        self.stage_from(runtime, self.current())
    }

    pub fn stage_from(
        &self,
        runtime: &SharedDispatchRuntime,
        base: Arc<DispatchGeneration>,
    ) -> Result<DispatchCandidate, DispatchStageError> {
        if base.len() != self.inner.slots.len() {
            return Err(DispatchStageError::new(
                "dispatch base belongs to another slot layout",
            ));
        }
        let mut targets = base.targets.to_vec();
        let mut seen = BTreeMap::<usize, String>::new();
        let runtime_guard = runtime.lock();
        for callable in runtime_guard.active_binding_schema().callables() {
            let Some(path) = callable.override_target.as_deref() else {
                continue;
            };
            let Some(index) = self.inner.by_path.get(path).copied() else {
                return Err(DispatchStageError::new(format!(
                    "Vela override `{}` names unknown replaceable target `{path}`",
                    callable.public_path
                )));
            };
            if let Some(existing) = seen.insert(index, callable.public_path.clone()) {
                return Err(DispatchStageError::new(format!(
                    "Vela overrides `{existing}` and `{}` both target `{path}`",
                    callable.public_path
                )));
            }
            let slot = &self.inner.slots[index];
            validate_override(slot, callable)?;
            targets[index] = Some(VelaOverrideTarget {
                slot: slot.id,
                function: callable.executable,
                runtime: Arc::clone(runtime),
            });
        }
        Ok(DispatchCandidate(Arc::new(DispatchGeneration {
            id: DispatchGenerationId::new(
                self.inner.next_generation.fetch_add(1, Ordering::Relaxed),
            ),
            targets: targets.into_boxed_slice(),
        })))
    }

    pub fn activate(&self, candidate: DispatchCandidate) -> Arc<DispatchGeneration> {
        std::mem::replace(&mut *self.inner.current.write(), candidate.0)
    }

    pub fn rollback(&self, generation: Arc<DispatchGeneration>) -> Arc<DispatchGeneration> {
        std::mem::replace(&mut *self.inner.current.write(), generation)
    }
}

#[derive(Debug)]
pub struct DispatchCandidate(Arc<DispatchGeneration>);

impl DispatchCandidate {
    #[must_use]
    pub fn generation(&self) -> &Arc<DispatchGeneration> {
        &self.0
    }
}

#[derive(Clone)]
pub struct DispatchRoot {
    generation: Arc<DispatchGeneration>,
}

impl DispatchRoot {
    #[must_use]
    pub fn pin(controller: &DispatchController) -> Self {
        Self {
            generation: controller.current(),
        }
    }

    #[must_use]
    pub fn generation(&self) -> &Arc<DispatchGeneration> {
        &self.generation
    }

    #[must_use]
    pub fn target(&self, index: InterceptSlotIndex) -> Option<VelaOverrideTarget> {
        self.generation.target(index)
    }

    #[must_use]
    pub fn invocation(&self) -> DispatchInvocation {
        DispatchInvocation
    }
}

pub trait DispatchAuthority {
    fn vela_dispatch_root(&self) -> &DispatchRoot;
}

#[derive(Clone, Copy)]
pub struct DispatchInvocation;

impl DispatchInvocation {
    pub fn call<R>(&self, target: VelaOverrideTarget, args: CallArgs<'_>) -> VmResult<R>
    where
        R: FromScriptArg,
    {
        let mut runtime = target.runtime.lock();
        let value = runtime.call(
            StableVelaFunction {
                function: target.function,
                diagnostic_name: "Vela dispatch override",
            },
            args,
            dispatch_call_options(),
        )?;
        let owned = runtime.value_to_owned(&value)?;
        R::from_script_arg(&owned)
    }

    pub fn call_async<'call, R>(
        &'call self,
        target: VelaOverrideTarget,
        args: CallArgs<'call>,
    ) -> DispatchCallFuture<'call, R>
    where
        R: FromScriptArg + Send + 'call,
    {
        let runtime = Arc::clone(&target.runtime);
        let function = target.function;
        Box::pin(async move {
            let mut runtime = runtime.lock_arc();
            let value = runtime
                .call_async(
                    StableVelaFunction {
                        function,
                        diagnostic_name: "Vela dispatch override",
                    },
                    args,
                    dispatch_call_options(),
                )
                .await?;
            let owned = runtime.value_to_owned(&value)?;
            R::from_script_arg(&owned)
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DispatchStageError {
    message: String,
}

impl DispatchStageError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for DispatchStageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for DispatchStageError {}

fn dispatch_call_options() -> CallOptions {
    CallOptions::new(
        DEFAULT_EXECUTION_UNITS,
        DEFAULT_MEMORY_BYTES,
        DEFAULT_CALL_DEPTH,
    )
}

fn validate_override(
    slot: &ReplaceableSlotDescriptor,
    callable: &RustBindingCallable,
) -> Result<(), DispatchStageError> {
    if callable.asyncness != slot.contract.asyncness {
        return Err(incompatible(slot, callable, "sync/async shape"));
    }
    if callable.required_capabilities != slot.contract.required_capabilities() {
        return Err(incompatible(slot, callable, "effective effects"));
    }
    let expected = slot
        .contract
        .parameters
        .iter()
        .filter(|parameter| parameter.mode != BoundaryMode::HiddenContext)
        .collect::<Vec<_>>();
    if expected.len() != callable.parameters.len() {
        return Err(incompatible(slot, callable, "parameter count"));
    }
    for (expected, actual) in expected.into_iter().zip(&callable.parameters) {
        let expected_mode = match expected.mode {
            BoundaryMode::Value | BoundaryMode::ReadOnlyValueBorrow => {
                RustBindingBoundaryMode::Value
            }
            BoundaryMode::SharedHost => RustBindingBoundaryMode::SharedHost,
            BoundaryMode::ExclusiveHost => RustBindingBoundaryMode::ExclusiveHost,
            BoundaryMode::HiddenContext => unreachable!("hidden contexts were filtered"),
        };
        if !compatible_mode(expected_mode, actual.mode)
            || !compatible_type(&expected.ty, &actual.ty)
        {
            return Err(incompatible(
                slot,
                callable,
                &format!(
                    "parameter `{}` (expected {:?} {:?}, found {:?} {:?})",
                    expected.name, expected.mode, expected.ty, actual.mode, actual.ty
                ),
            ));
        }
    }
    if !compatible_type(&slot.contract.returns.ty, &callable.returns.ty) {
        return Err(incompatible(slot, callable, "return type"));
    }
    Ok(())
}

fn compatible_mode(expected: RustBindingBoundaryMode, inferred: RustBindingBoundaryMode) -> bool {
    match expected {
        RustBindingBoundaryMode::Value => inferred == RustBindingBoundaryMode::Value,
        // Vela source does not spell Rust borrow modes. An override inherits
        // shared versus exclusive authority from its replaceable slot. The
        // binding schema can only conservatively classify every host
        // parameter as exclusive when the function has any host write.
        RustBindingBoundaryMode::SharedHost | RustBindingBoundaryMode::ExclusiveHost => matches!(
            inferred,
            RustBindingBoundaryMode::SharedHost | RustBindingBoundaryMode::ExclusiveHost
        ),
    }
}

fn compatible_type(expected: &TypeHint, actual: &RustBindingType) -> bool {
    match (expected, actual) {
        (TypeHint::Any, _) | (_, RustBindingType::Any) => true,
        (
            TypeHint::Primitive(tag),
            RustBindingType::Path {
                segments,
                arguments,
            },
        ) => arguments.is_empty() && segments.len() == 1 && segments[0] == tag.name(),
        (
            TypeHint::Host(key),
            RustBindingType::Host {
                semantic_type_id, ..
            },
        ) => key.id == *semantic_type_id,
        (
            TypeHint::Record(key) | TypeHint::Enum(key),
            RustBindingType::Definition { type_id, .. },
        ) => key.id == *type_id,
        (TypeHint::ArrayOf(expected), actual) => compatible_unary("Array", expected, actual),
        (TypeHint::OptionOf(expected), actual) => compatible_unary("Option", expected, actual),
        (TypeHint::SetOf(expected), actual) => compatible_unary("Set", expected, actual),
        (
            TypeHint::ResultOf { ok, err },
            RustBindingType::Path {
                segments,
                arguments,
            },
        ) => {
            segments.last().is_some_and(|segment| segment == "Result")
                && arguments.len() == 2
                && compatible_type(ok, &arguments[0])
                && compatible_type(err, &arguments[1])
        }
        _ => false,
    }
}

fn compatible_unary(expected_name: &str, expected: &TypeHint, actual: &RustBindingType) -> bool {
    let RustBindingType::Path {
        segments,
        arguments,
    } = actual
    else {
        return false;
    };
    segments
        .last()
        .is_some_and(|segment| segment == expected_name)
        && arguments.len() == 1
        && compatible_type(expected, &arguments[0])
}

fn incompatible(
    slot: &ReplaceableSlotDescriptor,
    callable: &RustBindingCallable,
    field: &str,
) -> DispatchStageError {
    DispatchStageError::new(format!(
        "Vela override `{}` is incompatible with replaceable target `{}` at {field}",
        callable.public_path, slot.contract.public_path
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interop::{
        CallableAccess, CallableIdentity, CallableKind, CallableLanguage, CallableOrigin,
        CallableParameter, CallableReturn, ErrorMode, ReturnMode,
    };
    use crate::native::EffectSet;
    use std::task::{Context, Poll, Waker};
    use vela_common::{CallableAsyncness, SourceId, Span};
    use vela_macros::{ScriptHost, ScriptReflect, methods, replaceable};

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
    pub async fn replaceable_increment_async(
        context: &mut ActorContext,
        value: i64,
    ) -> VmResult<i64> {
        context.calls += 10;
        Ok(value + 1)
    }

    fn direct_helper(value: i64) -> i64 {
        value + 1
    }

    #[test]
    fn staged_generation_pins_activation_and_rollback_per_root() {
        let engine = crate::engine::Engine::builder().build().expect("engine");
        let program = engine
            .compile_source(
                r#"
#[override(host::math::increment)]
pub fn patched(value: i64) -> i64 { return value + 2; }
"#,
            )
            .expect("override program");
        let runtime = Arc::new(Mutex::new(Runtime::new(engine, program).expect("runtime")));
        let controller =
            DispatchController::new(vec![slot(0, "host::math::increment")]).expect("controller");

        let before = DispatchRoot::pin(&controller);
        let candidate = controller
            .stage_current(&runtime)
            .expect("compatible override");
        let previous = controller.activate(candidate);
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

        controller.rollback(previous);
        let rolled_back = DispatchRoot::pin(&controller);
        assert!(rolled_back.target(InterceptSlotIndex::new(0)).is_none());
        assert_eq!(active.target(InterceptSlotIndex::new(0)), Some(target));
    }

    #[test]
    fn staging_rejects_unknown_duplicate_and_incompatible_overrides() {
        let controller =
            DispatchController::new(vec![slot(0, "host::math::increment")]).expect("controller");
        for (source, expected) in [
            (
                r#"#[override(host::missing)] pub fn patched(value: i64) -> i64 { return value; }"#,
                "unknown replaceable target",
            ),
            (
                r#"
#[override(host::math::increment)] pub fn first(value: i64) -> i64 { return value; }
#[override(host::math::increment)] pub fn second(value: i64) -> i64 { return value; }
"#,
                "both target",
            ),
            (
                r#"#[override(host::math::increment)] pub fn patched(value: String) -> i64 { return 1; }"#,
                "parameter `value`",
            ),
        ] {
            let engine = crate::engine::Engine::builder().build().expect("engine");
            let program = engine.compile_source(source).expect("override program");
            let runtime = Arc::new(Mutex::new(Runtime::new(engine, program).expect("runtime")));
            let error = controller
                .stage_current(&runtime)
                .expect_err("stage must reject invalid override");
            assert!(error.to_string().contains(expected), "{error}");
        }
    }

    #[test]
    fn staged_delta_preserves_targets_and_their_owning_runtimes() {
        let first_engine = crate::engine::Engine::builder().build().expect("engine");
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
        let second_engine = crate::engine::Engine::builder().build().expect("engine");
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
        let controller = DispatchController::new(vec![
            slot(0, "host::math::first"),
            slot(1, "host::math::second"),
        ])
        .expect("controller");

        let first = controller
            .stage_current(&first_runtime)
            .expect("first candidate");
        controller.activate(first);
        let second = controller
            .stage_current(&second_runtime)
            .expect("second delta over first generation");
        controller.activate(second);
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
        let engine = crate::engine::Engine::builder()
            .register_host_type::<ActorContext>()
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
        let controller = DispatchController::new(vec![
            vela_replaceable_slot_replaceable_increment(),
            vela_replaceable_slot_replaceable_increment_async(),
        ])
        .expect("controller");
        let mut old_context = ActorContext {
            calls: 0,
            root: DispatchRoot::pin(&controller),
        };
        assert_eq!(replaceable_increment(&mut old_context, 40), Ok(41));

        let candidate = controller.stage_current(&runtime).expect("override stage");
        controller.activate(candidate);
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
        let engine = crate::engine::Engine::builder()
            .register_host_type::<ActorContext>()
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
        let controller = DispatchController::new(vec![
            vela_replaceable_slot_replaceable_increment(),
            vela_replaceable_slot_replaceable_increment_async(),
        ])
        .expect("controller");
        let candidate = controller.stage_current(&runtime).expect("override stage");
        controller.activate(candidate);
        let mut context = ActorContext {
            calls: 0,
            root: DispatchRoot::pin(&controller),
        };

        assert!(replaceable_increment(&mut context, 40).is_err());
        assert_eq!(context.calls, 1);
    }

    #[test]
    fn replaceable_service_method_preserves_receiver_and_adjacent_rust_method() {
        let engine = crate::engine::Engine::builder()
            .register_host_type::<ActorContext>()
            .register_host_type::<GameService>()
            .register_exports(GameService::vela_inherent_exports())
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
        let controller = DispatchController::new(vec![
            vela_replaceable_slot_replaceable_increment(),
            vela_replaceable_slot_replaceable_increment_async(),
            GameService::vela_replaceable_slot_compute(),
        ])
        .expect("controller");
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
        let previous = controller.activate(candidate);
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

        controller.rollback(previous);
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

    fn ready<T>(future: impl Future<Output = T>) -> T {
        let mut future = std::pin::pin!(future);
        let mut context = Context::from_waker(Waker::noop());
        loop {
            if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
                return value;
            }
        }
    }
}
