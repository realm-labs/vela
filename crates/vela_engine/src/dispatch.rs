//! Optional immutable-generation dispatch for explicitly replaceable Rust entries.

mod returning;

use std::collections::BTreeMap;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::RwLock;
use vela_bytecode::{
    RustBindingBoundaryMode, RustBindingCallable, RustBindingEffectSet, RustBindingType,
};
use vela_common::{DispatchGenerationId, InterceptSlotIndex, ReplaceableSlotId, Span, stable_id};
use vela_def::FunctionId;

use crate::args::FromScriptArg;
use crate::binding::VmResult;
use crate::interop::{BoundaryMode, CallableContract};
use crate::native::{EffectSet, TypeHint};
use crate::runtime::handles::StableVelaFunction;
use crate::runtime::{CallArgs, CallOptions, SharedImage, SharedRuntime};

pub use returning::{
    BusinessResultReturn, DispatchOriginPayload, FromDispatchReturn, RuntimeResultReturn,
    ValueReturn, validate_business_dispatch_origin_payload, validate_dispatch_origin_payload,
    validate_optional_dispatch_origin_payload,
};

const DEFAULT_EXECUTION_UNITS: u64 = 1_000_000;
const DEFAULT_MEMORY_BYTES: usize = 8 * 1024 * 1024;
const DEFAULT_CALL_DEPTH: usize = 128;

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
        self.slot == other.slot && self.function == other.function
    }
}

impl Eq for VelaOverrideTarget {}

#[derive(Clone)]
pub struct DispatchGeneration {
    id: DispatchGenerationId,
    layout: Arc<DispatchLayoutIdentity>,
    targets: Box<[Option<VelaOverrideTarget>]>,
    image: Option<SharedImage>,
}

impl fmt::Debug for DispatchGeneration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DispatchGeneration")
            .field("id", &self.id)
            .field("targets", &self.targets)
            .field("has_artifact", &self.image.is_some())
            .finish()
    }
}

#[derive(Debug)]
struct DispatchLayoutIdentity {
    slot_ids: Box<[ReplaceableSlotId]>,
}

impl DispatchGeneration {
    #[must_use]
    pub const fn id(&self) -> DispatchGenerationId {
        self.id
    }

    #[must_use]
    pub fn target(&self, index: InterceptSlotIndex) -> Option<VelaOverrideTarget> {
        let target = self.targets.get(index.get())?.as_ref()?;
        (self.layout.slot_ids.get(index.get()) == Some(&target.slot)).then(|| target.clone())
    }

    fn target_for_slot(&self, slot: ReplaceableSlotId) -> Option<&VelaOverrideTarget> {
        let index = self
            .layout
            .slot_ids
            .iter()
            .position(|candidate| *candidate == slot)?;
        self.targets.get(index)?.as_ref()
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
    by_id: BTreeMap<ReplaceableSlotId, usize>,
    layout: Arc<DispatchLayoutIdentity>,
    current: RwLock<Arc<DispatchGeneration>>,
    next_generation: AtomicU64,
}

impl DispatchController {
    pub fn new(mut slots: Vec<ReplaceableSlotDescriptor>) -> Result<Self, DispatchStageError> {
        validate_slot_layout(&slots)?;
        slots.sort_by_key(|slot| slot.index);
        let mut by_id = BTreeMap::new();
        for (index, slot) in slots.iter().enumerate() {
            by_id.insert(slot.id, index);
        }
        let layout = Arc::new(DispatchLayoutIdentity {
            slot_ids: slots.iter().map(|slot| slot.id).collect(),
        });
        let initial = Arc::new(DispatchGeneration {
            id: DispatchGenerationId::new(0),
            layout: Arc::clone(&layout),
            targets: vec![None; slots.len()].into_boxed_slice(),
            image: None,
        });
        Ok(Self {
            inner: Arc::new(DispatchControllerInner {
                slots: slots.into_boxed_slice(),
                by_id,
                layout,
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
        runtime: &SharedRuntime,
    ) -> Result<DispatchCandidate, DispatchStageError> {
        self.stage_from(runtime, self.current())
    }

    pub fn stage_from(
        &self,
        runtime: &SharedRuntime,
        base: Arc<DispatchGeneration>,
    ) -> Result<DispatchCandidate, DispatchStageError> {
        if !Arc::ptr_eq(&base.layout, &self.inner.layout) {
            return Err(DispatchStageError::new(
                DispatchStageErrorCode::BaseLayoutMismatch,
                "dispatch base belongs to another controller",
                None,
            ));
        }
        let staged_image = runtime.shared_image();
        let same_artifact = base
            .image
            .as_ref()
            .is_some_and(|base_image| base_image.same_image(&staged_image));
        let mut targets = if same_artifact {
            base.targets.to_vec()
        } else {
            vec![None; base.targets.len()]
        };
        let mut seen = BTreeMap::<usize, String>::new();
        for callable in runtime.active_binding_schema().callables() {
            let Some(override_target) = callable.override_target.as_ref() else {
                continue;
            };
            let path = override_target.public_path();
            let Some((slot_id, contract_fingerprint)) = override_target.resolved() else {
                return Err(DispatchStageError::new(
                    DispatchStageErrorCode::UnknownTarget,
                    format!(
                        "Vela override `{}` target `{path}` was not linked by Engine compilation",
                        callable.public_path
                    ),
                    Some(callable.source),
                ));
            };
            let Some(index) = self.inner.by_id.get(&slot_id).copied() else {
                return Err(DispatchStageError::new(
                    DispatchStageErrorCode::UnknownTarget,
                    format!(
                        "Vela override `{}` links unknown replaceable slot {} (`{path}`)",
                        callable.public_path,
                        slot_id.get()
                    ),
                    Some(callable.source),
                ));
            };
            if let Some(existing) = seen.insert(index, callable.public_path.clone()) {
                return Err(DispatchStageError::new(
                    DispatchStageErrorCode::DuplicateTarget,
                    format!(
                        "Vela overrides `{existing}` and `{}` both target `{path}`",
                        callable.public_path
                    ),
                    Some(callable.source),
                ));
            }
            let slot = &self.inner.slots[index];
            if slot.contract.abi_fingerprint().get() != contract_fingerprint {
                return Err(incompatible(
                    slot,
                    callable,
                    "linked target contract fingerprint",
                ));
            }
            validate_override(slot, callable)?;
            targets[index] = Some(VelaOverrideTarget {
                slot: slot.id,
                function: callable.executable,
            });
        }
        if !same_artifact {
            for (index, previous) in base.targets.iter().enumerate() {
                if previous.is_some() && targets[index].is_none() {
                    return Err(DispatchStageError::new(
                        DispatchStageErrorCode::ArtifactMismatch,
                        format!(
                            "staged artifact omits active override `{}`; coherent replacement artifacts must materialize every selected slot",
                            self.inner.slots[index].contract.public_path
                        ),
                        None,
                    ));
                }
            }
        }
        Ok(DispatchCandidate(Arc::new(DispatchGeneration {
            id: DispatchGenerationId::new(
                self.inner.next_generation.fetch_add(1, Ordering::Relaxed),
            ),
            layout: Arc::clone(&self.inner.layout),
            targets: targets.into_boxed_slice(),
            image: Some(staged_image),
        })))
    }

    pub fn activate(
        &self,
        candidate: DispatchCandidate,
    ) -> Result<Arc<DispatchGeneration>, DispatchStageError> {
        self.ensure_owned_generation(&candidate.0, "dispatch candidate")?;
        Ok(std::mem::replace(
            &mut *self.inner.current.write(),
            candidate.0,
        ))
    }

    pub fn rollback(
        &self,
        generation: Arc<DispatchGeneration>,
    ) -> Result<Arc<DispatchGeneration>, DispatchStageError> {
        self.ensure_owned_generation(&generation, "rollback generation")?;
        Ok(std::mem::replace(
            &mut *self.inner.current.write(),
            generation,
        ))
    }

    fn ensure_owned_generation(
        &self,
        generation: &DispatchGeneration,
        subject: &str,
    ) -> Result<(), DispatchStageError> {
        if Arc::ptr_eq(&generation.layout, &self.inner.layout) {
            return Ok(());
        }
        Err(DispatchStageError::new(
            DispatchStageErrorCode::BaseLayoutMismatch,
            format!("{subject} belongs to another controller"),
            None,
        ))
    }
}

pub(crate) fn validate_slot_layout(
    slots: &[ReplaceableSlotDescriptor],
) -> Result<(), DispatchStageError> {
    let mut ordered = slots.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|slot| slot.index);
    let mut by_path = BTreeMap::new();
    let mut by_id = BTreeMap::new();
    for (expected, slot) in ordered.into_iter().enumerate() {
        if slot.index.get() != expected {
            return Err(DispatchStageError::new(
                DispatchStageErrorCode::InvalidSlotLayout,
                format!(
                    "replaceable slot indices must be dense from zero; expected {expected}, got {}",
                    slot.index.get()
                ),
                None,
            ));
        }
        if by_path
            .insert(slot.contract.public_path.as_str(), slot.index)
            .is_some()
        {
            return Err(DispatchStageError::new(
                DispatchStageErrorCode::InvalidSlotLayout,
                format!(
                    "duplicate replaceable slot path `{}`",
                    slot.contract.public_path
                ),
                None,
            ));
        }
        if let Some(existing) = by_id.insert(slot.id, slot.contract.public_path.as_str()) {
            return Err(DispatchStageError::new(
                DispatchStageErrorCode::InvalidSlotLayout,
                format!(
                    "replaceable slots `{existing}` and `{}` share stable id {}",
                    slot.contract.public_path,
                    slot.id.get()
                ),
                None,
            ));
        }
    }
    Ok(())
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
    options: CallOptions,
}

impl DispatchRoot {
    #[must_use]
    pub fn pin(controller: &DispatchController) -> Self {
        Self::pin_with_options(controller, default_dispatch_call_options())
    }

    #[must_use]
    pub fn pin_with_options(controller: &DispatchController, options: CallOptions) -> Self {
        Self {
            generation: controller.current(),
            options,
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

    pub fn invocation<'turn>(
        &self,
        runtime: &'turn mut SharedRuntime,
    ) -> VmResult<DispatchInvocation<'turn>> {
        if let Some(expected) = self.generation.image.as_ref()
            && !expected.same_image(&runtime.shared_image())
        {
            return Err(vela_vm::error::VmError::new(
                vela_vm::error::VmErrorKind::TypeMismatch {
                    operation: "dispatch generation artifact",
                },
            ));
        }
        Ok(DispatchInvocation {
            generation: Arc::clone(&self.generation),
            runtime,
            options: self.options.clone(),
        })
    }
}

pub trait DispatchAuthority {
    fn vela_dispatch_root(&self) -> &DispatchRoot;

    fn vela_dispatch_invocation(&mut self) -> VmResult<DispatchInvocation<'_>>;
}

pub struct DispatchInvocation<'turn> {
    generation: Arc<DispatchGeneration>,
    runtime: &'turn mut SharedRuntime,
    options: CallOptions,
}

impl DispatchInvocation<'_> {
    pub fn call_owned(
        &mut self,
        target: VelaOverrideTarget,
        args: CallArgs<'_>,
    ) -> VmResult<vela_vm::owned_value::OwnedValue> {
        self.validate_target(&target)?;
        self.validate_runtime()?;
        let value = self.runtime.call(
            StableVelaFunction {
                function: target.function,
                diagnostic_name: "Vela dispatch override",
            },
            args,
            dispatch_call_options(&self.options, Arc::clone(&self.generation)),
        )?;
        self.runtime.value_to_owned(&value)
    }

    pub fn call<R>(&mut self, target: VelaOverrideTarget, args: CallArgs<'_>) -> VmResult<R>
    where
        R: FromScriptArg,
    {
        let owned = self.call_owned(target, args)?;
        R::from_script_arg(&owned)
    }

    pub fn call_owned_async<'call>(
        &'call mut self,
        target: VelaOverrideTarget,
        args: CallArgs<'call>,
    ) -> DispatchCallFuture<'call, vela_vm::owned_value::OwnedValue> {
        if let Err(error) = self.validate_target(&target) {
            return Box::pin(async move { Err(error) });
        }
        let generation = Arc::clone(&self.generation);
        let options = self.options.clone();
        let function = target.function;
        Box::pin(async move {
            self.validate_runtime()?;
            let value = self
                .runtime
                .call_async(
                    StableVelaFunction {
                        function,
                        diagnostic_name: "Vela dispatch override",
                    },
                    args,
                    dispatch_call_options(&options, generation),
                )
                .await?;
            self.runtime.value_to_owned(&value)
        })
    }

    pub fn call_async<'call, R>(
        &'call mut self,
        target: VelaOverrideTarget,
        args: CallArgs<'call>,
    ) -> DispatchCallFuture<'call, R>
    where
        R: FromScriptArg + Send + 'call,
    {
        Box::pin(async move {
            let owned = self.call_owned_async(target, args).await?;
            R::from_script_arg(&owned)
        })
    }

    fn validate_target(&self, target: &VelaOverrideTarget) -> VmResult<()> {
        if self.generation.target_for_slot(target.slot) != Some(target) {
            return Err(vela_vm::error::VmError::new(
                vela_vm::error::VmErrorKind::TypeMismatch {
                    operation: "dispatch target generation",
                },
            ));
        }
        Ok(())
    }

    fn validate_runtime(&self) -> VmResult<()> {
        if self
            .generation
            .image
            .as_ref()
            .is_some_and(|expected| !expected.same_image(&self.runtime.shared_image()))
        {
            return Err(vela_vm::error::VmError::new(
                vela_vm::error::VmErrorKind::TypeMismatch {
                    operation: "dispatch generation artifact",
                },
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DispatchStageError {
    code: DispatchStageErrorCode,
    message: String,
    source: Option<Span>,
}

impl DispatchStageError {
    fn new(code: DispatchStageErrorCode, message: impl Into<String>, source: Option<Span>) -> Self {
        Self {
            code,
            message: message.into(),
            source,
        }
    }

    #[must_use]
    pub const fn code(&self) -> DispatchStageErrorCode {
        self.code
    }

    #[must_use]
    pub const fn source(&self) -> Option<Span> {
        self.source
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DispatchStageErrorCode {
    InvalidSlotLayout,
    BaseLayoutMismatch,
    UnknownTarget,
    DuplicateTarget,
    IncompatibleContract,
    ArtifactMismatch,
}

impl fmt::Display for DispatchStageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for DispatchStageError {}

fn default_dispatch_call_options() -> CallOptions {
    CallOptions::new(
        DEFAULT_EXECUTION_UNITS,
        DEFAULT_MEMORY_BYTES,
        DEFAULT_CALL_DEPTH,
    )
}

fn dispatch_call_options(
    options: &CallOptions,
    generation: Arc<DispatchGeneration>,
) -> CallOptions {
    options.clone().with_dispatch_generation(generation)
}

pub(crate) fn validate_override(
    slot: &ReplaceableSlotDescriptor,
    callable: &RustBindingCallable,
) -> Result<(), DispatchStageError> {
    validate_override_contract(slot, callable, true)
}

pub(crate) fn validate_override_source(
    slot: &ReplaceableSlotDescriptor,
    callable: &RustBindingCallable,
) -> Result<(), DispatchStageError> {
    validate_override_contract(slot, callable, false)
}

fn validate_override_contract(
    slot: &ReplaceableSlotDescriptor,
    callable: &RustBindingCallable,
    inherited: bool,
) -> Result<(), DispatchStageError> {
    if callable.asyncness != slot.contract.asyncness {
        return Err(incompatible(slot, callable, "sync/async shape"));
    }
    let implementation_effects = binding_effects(callable.effects);
    if !slot.contract.effects.contains_all(implementation_effects) {
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
        if !(if inherited {
            expected_mode == actual.mode
        } else {
            compatible_source_mode(expected_mode, actual.mode)
        }) || !compatible_type(&expected.ty, &actual.ty)
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
        return Err(incompatible(
            slot,
            callable,
            &format!(
                "return type (expected {:?}, found {:?})",
                slot.contract.returns.ty, callable.returns.ty
            ),
        ));
    }
    if inherited {
        if callable.returns.mode != binding_return_mode(slot.contract.returns.mode) {
            return Err(incompatible(slot, callable, "return mode"));
        }
        let expected_error_mode = match slot.contract.returns.error_mode {
            crate::interop::ErrorMode::Value => vela_bytecode::RustBindingErrorMode::Value,
            crate::interop::ErrorMode::RuntimeResult => {
                vela_bytecode::RustBindingErrorMode::RuntimeResult
            }
        };
        if callable.returns.error_mode != expected_error_mode {
            return Err(incompatible(slot, callable, "error mode"));
        }
    }
    Ok(())
}

fn binding_effects(effects: RustBindingEffectSet) -> EffectSet {
    let mut boundary = EffectSet::pure();
    for (present, effect) in [
        (effects.host_read, EffectSet::host_read()),
        (effects.host_write, EffectSet::host_write()),
        (effects.reflection_read, EffectSet::reflection_read()),
        (effects.reflection_write, EffectSet::reflection_write()),
        (effects.reflection_call, EffectSet::reflection_call()),
        (effects.emits_event, EffectSet::event_emit()),
        (effects.reads_time, EffectSet::time()),
        (effects.uses_random, EffectSet::random()),
        (effects.reads_io, EffectSet::io_read()),
        (effects.writes_io, EffectSet::io_write()),
    ] {
        if present {
            boundary = boundary.union(effect);
        }
    }
    boundary
}

fn compatible_source_mode(
    expected: RustBindingBoundaryMode,
    inferred: RustBindingBoundaryMode,
) -> bool {
    match expected {
        RustBindingBoundaryMode::Value => inferred == RustBindingBoundaryMode::Value,
        RustBindingBoundaryMode::SharedHost | RustBindingBoundaryMode::ExclusiveHost => matches!(
            inferred,
            RustBindingBoundaryMode::SharedHost | RustBindingBoundaryMode::ExclusiveHost
        ),
    }
}

fn binding_return_mode(mode: crate::interop::ReturnMode) -> vela_bytecode::RustBindingReturnMode {
    match mode {
        crate::interop::ReturnMode::OwnedValue => vela_bytecode::RustBindingReturnMode::OwnedValue,
        crate::interop::ReturnMode::StructuredValue => {
            vela_bytecode::RustBindingReturnMode::StructuredValue
        }
        crate::interop::ReturnMode::ScopedHost {
            origin,
            child_access,
            parent_freeze,
        } => vela_bytecode::RustBindingReturnMode::ScopedHost {
            origin: match origin {
                crate::interop::BorrowedReturnOrigin::Receiver => {
                    vela_bytecode::RustBindingBorrowedReturnOrigin::Receiver
                }
                crate::interop::BorrowedReturnOrigin::Parameter(index) => {
                    vela_bytecode::RustBindingBorrowedReturnOrigin::Parameter(index)
                }
            },
            child_access: binding_scoped_access(child_access),
            parent_freeze: binding_scoped_access(parent_freeze),
        },
    }
}

fn binding_scoped_access(
    access: crate::interop::ScopedHostAccess,
) -> vela_bytecode::RustBindingScopedHostAccess {
    match access {
        crate::interop::ScopedHostAccess::Shared => {
            vela_bytecode::RustBindingScopedHostAccess::Shared
        }
        crate::interop::ScopedHostAccess::Exclusive => {
            vela_bytecode::RustBindingScopedHostAccess::Exclusive
        }
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
        ) => {
            arguments.is_empty()
                && segments.len() == 1
                && (segments[0] == tag.name()
                    || matches!(
                        (tag, segments[0].as_str()),
                        (vela_common::PrimitiveTag::String, "String")
                            | (vela_common::PrimitiveTag::Bytes, "Bytes")
                            | (vela_common::PrimitiveTag::Unit, "Unit")
                    ))
        }
        (
            TypeHint::Host(key),
            RustBindingType::Host {
                semantic_type_id, ..
            },
        ) => key.id == *semantic_type_id,
        (
            TypeHint::Host(key),
            RustBindingType::Path {
                segments,
                arguments,
            },
        ) => arguments.is_empty() && segments.last().is_some_and(|name| name == &key.name),
        (
            TypeHint::Record(key) | TypeHint::Enum(key),
            RustBindingType::Definition { type_id, .. },
        ) => key.id == *type_id,
        (TypeHint::ArrayOf(expected), actual) => compatible_unary("Array", expected, actual),
        (TypeHint::Array, actual) => compatible_named("Array", 0, actual),
        (TypeHint::Map, actual) => compatible_named("Map", 0, actual),
        (TypeHint::MapOf { key, value }, actual) => compatible_binary("Map", key, value, actual),
        (TypeHint::Set, actual) => compatible_named("Set", 0, actual),
        (TypeHint::OptionOf(expected), actual) => compatible_unary("Option", expected, actual),
        (TypeHint::SetOf(expected), actual) => compatible_unary("Set", expected, actual),
        (TypeHint::Iterator, actual) => compatible_named("Iterator", 0, actual),
        (TypeHint::IteratorOf(expected), actual) => compatible_unary("Iterator", expected, actual),
        (
            TypeHint::TupleOf(expected),
            RustBindingType::Path {
                segments,
                arguments,
            },
        ) => {
            segments.last().is_some_and(|segment| segment == "Tuple")
                && expected.len() == arguments.len()
                && expected
                    .iter()
                    .zip(arguments.iter())
                    .all(|(expected, actual)| compatible_type(expected, actual))
        }
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
        (TypeHint::PathProxy, actual) => compatible_named("PathProxy", 0, actual),
        (
            TypeHint::Trait(expected),
            RustBindingType::Path {
                segments,
                arguments,
            },
        ) => arguments.is_empty() && segments.last() == Some(expected),
        (TypeHint::Function, actual) => compatible_named("Function", 0, actual),
        _ => false,
    }
}

fn compatible_named(expected_name: &str, arity: usize, actual: &RustBindingType) -> bool {
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
        && arguments.len() == arity
}

fn compatible_binary(
    expected_name: &str,
    first: &TypeHint,
    second: &TypeHint,
    actual: &RustBindingType,
) -> bool {
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
        && arguments.len() == 2
        && compatible_type(first, &arguments[0])
        && compatible_type(second, &arguments[1])
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
    DispatchStageError::new(
        DispatchStageErrorCode::IncompatibleContract,
        format!(
            "Vela override `{}` is incompatible with replaceable target `{}` at {field}",
            callable.public_path, slot.contract.public_path
        ),
        Some(callable.source),
    )
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "dispatch/returning_tests.rs"]
mod returning_tests;

#[cfg(test)]
#[path = "dispatch/business_macro_tests.rs"]
mod business_macro_tests;
