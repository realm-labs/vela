use std::fmt;
use std::path::{Path, PathBuf};

use vela_bytecode::StateStorage;
use vela_common::{CallableAsyncness, Span};
use vela_mir::MirTypeContract;

use crate::abi::{AccessAbi, EffectAbi, ParamAbi, TraitMethodAbi};
use crate::module_abi::ModuleExportAbi;
use crate::schema_abi::SchemaAbi;

#[derive(Clone, Debug, PartialEq)]
pub struct HotReloadError {
    pub kind: HotReloadErrorKind,
}

impl HotReloadError {
    pub(crate) fn new(kind: HotReloadErrorKind) -> Self {
        Self { kind }
    }

    #[must_use]
    pub const fn code(&self) -> &'static str {
        match &self.kind {
            HotReloadErrorKind::DeletedFunctionParameters { .. } => {
                "reload.function.deleted_parameters"
            }
            HotReloadErrorKind::ChangedFunctionParameters { .. } => {
                "reload.function.changed_parameters"
            }
            HotReloadErrorKind::ChangedFunctionParameterAbi { .. } => {
                "reload.function.parameter_abi_changed"
            }
            HotReloadErrorKind::ChangedFunctionReturnAbi { .. } => {
                "reload.function.return_abi_changed"
            }
            HotReloadErrorKind::AddedFunctionParametersWithoutDefaults { .. } => {
                "reload.function.required_added_parameters"
            }
            HotReloadErrorKind::AddedFunctionParametersDenied { .. } => {
                "reload.function.added_parameters_denied"
            }
            HotReloadErrorKind::NewFunctionDenied { .. } => "reload.function.new_denied",
            HotReloadErrorKind::RemovedFunction { .. } => "reload.function.removed",
            HotReloadErrorKind::RemovedSchema { .. } => "reload.schema.removed",
            HotReloadErrorKind::ChangedSchema { .. } => "reload.schema.changed",
            HotReloadErrorKind::ChangedSchemaAbi { .. } => "reload.schema.abi_changed",
            HotReloadErrorKind::RemovedFunctionAbi { .. } => "reload.function.removed_abi",
            HotReloadErrorKind::ChangedFunctionEvent { .. } => "reload.function.event_changed",
            HotReloadErrorKind::ChangedFunctionAsyncness { .. } => {
                "reload.function.asyncness_changed"
            }
            HotReloadErrorKind::ChangedFunctionEffects { .. } => "reload.function.effects_changed",
            HotReloadErrorKind::ChangedFunctionAccess { .. } => "reload.function.access_changed",
            HotReloadErrorKind::RemovedMethodAbi { .. } => "reload.method.removed_abi",
            HotReloadErrorKind::ChangedMethodParameterAbi { .. } => {
                "reload.method.parameter_abi_changed"
            }
            HotReloadErrorKind::ChangedMethodReturnAbi { .. } => "reload.method.return_abi_changed",
            HotReloadErrorKind::ChangedMethodAsyncness { .. } => "reload.method.asyncness_changed",
            HotReloadErrorKind::ChangedMethodEffects { .. } => "reload.method.effects_changed",
            HotReloadErrorKind::ChangedMethodAccess { .. } => "reload.method.access_changed",
            HotReloadErrorKind::RemovedTraitAbi { .. } => "reload.trait.removed_abi",
            HotReloadErrorKind::ChangedTraitAbi { .. } => "reload.trait.changed_abi",
            HotReloadErrorKind::RemovedModuleAbi { .. } => "reload.module.removed_abi",
            HotReloadErrorKind::ChangedModuleAbi { .. } => "reload.module.changed_abi",
            HotReloadErrorKind::ChangedPackageProviderAbi { .. } => {
                "reload.package_provider.changed_abi"
            }
            HotReloadErrorKind::ChangedStateStorage { .. } => "reload.state.storage_changed",
            HotReloadErrorKind::ChangedStateType { .. } => "reload.state.type_changed",
            HotReloadErrorKind::MissingExternStateBinding { .. } => {
                "reload.state.extern_binding_missing"
            }
            HotReloadErrorKind::InvalidExternStateBinding { .. } => {
                "reload.state.extern_binding_invalid"
            }
            HotReloadErrorKind::StateInitializerFailed { .. } => "reload.state.initializer_failed",
        }
    }

    #[must_use]
    pub fn target(&self) -> Option<String> {
        match &self.kind {
            HotReloadErrorKind::DeletedFunctionParameters { function, .. }
            | HotReloadErrorKind::ChangedFunctionParameters { function, .. }
            | HotReloadErrorKind::ChangedFunctionParameterAbi { function, .. }
            | HotReloadErrorKind::ChangedFunctionReturnAbi { function, .. }
            | HotReloadErrorKind::AddedFunctionParametersWithoutDefaults { function, .. }
            | HotReloadErrorKind::AddedFunctionParametersDenied { function, .. }
            | HotReloadErrorKind::NewFunctionDenied { function }
            | HotReloadErrorKind::RemovedFunction { function }
            | HotReloadErrorKind::RemovedFunctionAbi { function, .. }
            | HotReloadErrorKind::ChangedFunctionEvent { function, .. }
            | HotReloadErrorKind::ChangedFunctionAsyncness { function, .. }
            | HotReloadErrorKind::ChangedFunctionEffects { function, .. }
            | HotReloadErrorKind::ChangedFunctionAccess { function, .. } => Some(function.clone()),
            HotReloadErrorKind::RemovedSchema { type_name, .. }
            | HotReloadErrorKind::ChangedSchema { type_name, .. }
            | HotReloadErrorKind::ChangedSchemaAbi { type_name, .. } => Some(type_name.clone()),
            HotReloadErrorKind::RemovedMethodAbi {
                type_name, method, ..
            }
            | HotReloadErrorKind::ChangedMethodParameterAbi {
                type_name, method, ..
            }
            | HotReloadErrorKind::ChangedMethodReturnAbi {
                type_name, method, ..
            }
            | HotReloadErrorKind::ChangedMethodAsyncness {
                type_name, method, ..
            }
            | HotReloadErrorKind::ChangedMethodEffects {
                type_name, method, ..
            }
            | HotReloadErrorKind::ChangedMethodAccess {
                type_name, method, ..
            } => Some(format!("{type_name}.{method}")),
            HotReloadErrorKind::RemovedTraitAbi { trait_name, .. }
            | HotReloadErrorKind::ChangedTraitAbi { trait_name, .. } => Some(trait_name.clone()),
            HotReloadErrorKind::RemovedModuleAbi { module, .. }
            | HotReloadErrorKind::ChangedModuleAbi { module, .. } => Some(module.clone()),
            HotReloadErrorKind::ChangedPackageProviderAbi { target, .. } => Some(target.clone()),
            HotReloadErrorKind::ChangedStateStorage { state, .. }
            | HotReloadErrorKind::ChangedStateType { state, .. }
            | HotReloadErrorKind::MissingExternStateBinding { state, .. }
            | HotReloadErrorKind::InvalidExternStateBinding { state, .. }
            | HotReloadErrorKind::StateInitializerFailed { state, .. } => Some(state.clone()),
        }
    }

    #[must_use]
    pub fn reason(&self) -> String {
        match &self.kind {
            HotReloadErrorKind::DeletedFunctionParameters { function, .. } => {
                format!("function `{function}` deleted existing parameters")
            }
            HotReloadErrorKind::ChangedFunctionParameters { function, .. } => {
                format!("function `{function}` changed existing parameter names or order")
            }
            HotReloadErrorKind::ChangedFunctionParameterAbi { function, .. } => {
                format!("function `{function}` changed parameter ABI")
            }
            HotReloadErrorKind::ChangedFunctionReturnAbi { function, .. } => {
                format!("function `{function}` changed return ABI")
            }
            HotReloadErrorKind::AddedFunctionParametersWithoutDefaults { function, .. } => {
                format!("function `{function}` added required parameters")
            }
            HotReloadErrorKind::AddedFunctionParametersDenied { function, .. } => {
                format!("function `{function}` added parameters denied by reload policy")
            }
            HotReloadErrorKind::NewFunctionDenied { function } => {
                format!("new function `{function}` is denied by reload policy")
            }
            HotReloadErrorKind::RemovedFunction { function } => {
                format!("function `{function}` was removed from the update source")
            }
            HotReloadErrorKind::RemovedSchema { type_name, .. } => {
                format!("schema `{type_name}` was removed")
            }
            HotReloadErrorKind::ChangedSchema { type_name, .. } => {
                format!("schema `{type_name}` changed incompatibly")
            }
            HotReloadErrorKind::ChangedSchemaAbi { type_name, .. } => {
                format!("schema `{type_name}` changed member ABI incompatibly")
            }
            HotReloadErrorKind::RemovedFunctionAbi { function, .. } => {
                format!("function `{function}` was removed from the hot-reload ABI")
            }
            HotReloadErrorKind::ChangedFunctionEvent { function, .. } => {
                format!("function `{function}` changed event binding ABI")
            }
            HotReloadErrorKind::ChangedFunctionAsyncness { function, .. } => {
                format!("function `{function}` changed asyncness ABI")
            }
            HotReloadErrorKind::ChangedFunctionEffects { function, .. } => {
                format!("function `{function}` changed effect ABI")
            }
            HotReloadErrorKind::ChangedFunctionAccess { function, .. } => {
                format!("function `{function}` changed reflective access ABI")
            }
            HotReloadErrorKind::RemovedMethodAbi {
                type_name, method, ..
            } => {
                format!("method `{type_name}.{method}` was removed from the hot-reload ABI")
            }
            HotReloadErrorKind::ChangedMethodParameterAbi {
                type_name, method, ..
            } => {
                format!("method `{type_name}.{method}` changed parameter ABI")
            }
            HotReloadErrorKind::ChangedMethodReturnAbi {
                type_name, method, ..
            } => {
                format!("method `{type_name}.{method}` changed return ABI")
            }
            HotReloadErrorKind::ChangedMethodAsyncness {
                type_name, method, ..
            } => {
                format!("method `{type_name}.{method}` changed asyncness ABI")
            }
            HotReloadErrorKind::ChangedMethodEffects {
                type_name, method, ..
            } => {
                format!("method `{type_name}.{method}` changed effect ABI")
            }
            HotReloadErrorKind::ChangedMethodAccess {
                type_name, method, ..
            } => {
                format!("method `{type_name}.{method}` changed reflective access ABI")
            }
            HotReloadErrorKind::RemovedTraitAbi { trait_name, .. } => {
                format!("trait `{trait_name}` was removed from the hot-reload ABI")
            }
            HotReloadErrorKind::ChangedTraitAbi { trait_name, .. } => {
                format!("trait `{trait_name}` changed method ABI")
            }
            HotReloadErrorKind::RemovedModuleAbi { module, .. } => {
                format!("module `{module}` was removed from the hot-reload ABI")
            }
            HotReloadErrorKind::ChangedModuleAbi { module, .. } => {
                format!("module `{module}` changed export ABI")
            }
            HotReloadErrorKind::ChangedPackageProviderAbi { target, reason, .. } => {
                format!("{target} changed incompatibly: {reason}")
            }
            HotReloadErrorKind::ChangedStateStorage {
                state, old, new, ..
            } => {
                format!("state `{state}` changed storage from {old:?} to {new:?}")
            }
            HotReloadErrorKind::ChangedStateType {
                state, old, new, ..
            } => {
                format!("state `{state}` changed type contract from {old:?} to {new:?}")
            }
            HotReloadErrorKind::MissingExternStateBinding { state, .. } => {
                format!("new extern state `{state}` has no staged host binding")
            }
            HotReloadErrorKind::InvalidExternStateBinding { state, reason, .. } => {
                format!("extern state `{state}` has an incompatible staged binding: {reason}")
            }
            HotReloadErrorKind::StateInitializerFailed { state, reason, .. } => {
                format!("initializer for new state `{state}` failed: {reason}")
            }
        }
    }

    #[must_use]
    pub fn repair_hint(&self) -> Option<String> {
        match &self.kind {
            HotReloadErrorKind::DeletedFunctionParameters { .. } => {
                Some("restore the previous parameter prefix or add a compatibility wrapper".to_owned())
            }
            HotReloadErrorKind::ChangedFunctionParameters { .. } => {
                Some("preserve existing parameter names and order".to_owned())
            }
            HotReloadErrorKind::ChangedFunctionParameterAbi { .. } => {
                Some("preserve existing parameter names, order, type hints, and defaults".to_owned())
            }
            HotReloadErrorKind::ChangedFunctionReturnAbi { .. } => {
                Some("preserve the previous return type hint or restart with an explicit migration".to_owned())
            }
            HotReloadErrorKind::AddedFunctionParametersWithoutDefaults { .. } => {
                Some("give every appended parameter a default value".to_owned())
            }
            HotReloadErrorKind::AddedFunctionParametersDenied { .. } => {
                Some("enable defaulted parameter additions in HotReloadPolicy or remove the new parameters".to_owned())
            }
            HotReloadErrorKind::NewFunctionDenied { .. } => {
                Some("enable new functions in HotReloadPolicy or remove the new declaration".to_owned())
            }
            HotReloadErrorKind::RemovedFunction { .. } => {
                Some("keep the function declaration or restart with an explicit migration".to_owned())
            }
            HotReloadErrorKind::RemovedSchema { .. } => {
                Some("restore the schema or restart with an explicit migration".to_owned())
            }
            HotReloadErrorKind::ChangedSchema { .. } => {
                Some("keep the existing schema hash stable or restart with an explicit migration".to_owned())
            }
            HotReloadErrorKind::ChangedSchemaAbi { .. } => {
                Some("preserve existing schema members, or add only defaulted fields during reload".to_owned())
            }
            HotReloadErrorKind::RemovedFunctionAbi { .. } => {
                Some("restore the function ABI entry or restart with an explicit migration".to_owned())
            }
            HotReloadErrorKind::ChangedFunctionEvent { .. } => {
                Some("preserve the previous event binding or restart with an explicit migration".to_owned())
            }
            HotReloadErrorKind::ChangedFunctionAsyncness { .. }
            | HotReloadErrorKind::ChangedMethodAsyncness { .. } => {
                Some("preserve whether the callable is sync or async, or restart with an explicit migration".to_owned())
            }
            HotReloadErrorKind::RemovedMethodAbi { .. } => {
                Some("restore the method ABI entry or restart with an explicit migration".to_owned())
            }
            HotReloadErrorKind::RemovedTraitAbi { .. } => {
                Some("restore the trait ABI entry or restart with an explicit migration".to_owned())
            }
            HotReloadErrorKind::ChangedMethodParameterAbi { .. } => {
                Some("preserve existing method parameter names, order, type hints, and defaults".to_owned())
            }
            HotReloadErrorKind::ChangedMethodReturnAbi { .. } => {
                Some("preserve the previous method return type hint or restart with an explicit migration".to_owned())
            }
            HotReloadErrorKind::ChangedFunctionEffects { .. }
            | HotReloadErrorKind::ChangedMethodEffects { .. } => {
                Some("preserve the previous effect set or require host approval before reloading".to_owned())
            }
            HotReloadErrorKind::ChangedFunctionAccess { .. }
            | HotReloadErrorKind::ChangedMethodAccess { .. } => {
                Some("preserve reflective access metadata or require host approval before reloading".to_owned())
            }
            HotReloadErrorKind::ChangedTraitAbi { .. } => {
                Some("preserve existing trait method IDs, names, parameters, return hints, and default status".to_owned())
            }
            HotReloadErrorKind::RemovedModuleAbi { .. } => {
                Some("restore the module ABI entry or restart with an explicit migration".to_owned())
            }
            HotReloadErrorKind::ChangedModuleAbi { .. } => {
                Some("preserve existing module exports or restart with an explicit migration".to_owned())
            }
            HotReloadErrorKind::ChangedPackageProviderAbi { .. } => Some(
                "preserve the package roots, selected providers, provider targets, methods, and capability requirements or explicitly restage the runtime".to_owned(),
            ),
            HotReloadErrorKind::ChangedStateStorage { .. } => Some(
                "preserve the state storage kind or restart with an explicit migration".to_owned(),
            ),
            HotReloadErrorKind::ChangedStateType { .. } => Some(
                "preserve the exact state type contract or restart with an explicit migration".to_owned(),
            ),
            HotReloadErrorKind::MissingExternStateBinding { .. } => Some(
                "stage a type-compatible host object for the new extern state before activation".to_owned(),
            ),
            HotReloadErrorKind::InvalidExternStateBinding { .. } => Some(
                "stage a host object whose registered type exactly matches the extern state contract".to_owned(),
            ),
            HotReloadErrorKind::StateInitializerFailed { .. } => Some(
                "fix the initializer or its configured execution limits before retrying the update".to_owned(),
            ),
        }
    }

    #[must_use]
    pub fn source_span(&self) -> Option<Span> {
        match &self.kind {
            HotReloadErrorKind::RemovedSchema { source_span, .. }
            | HotReloadErrorKind::ChangedSchema { source_span, .. }
            | HotReloadErrorKind::ChangedSchemaAbi { source_span, .. }
            | HotReloadErrorKind::RemovedFunctionAbi { source_span, .. }
            | HotReloadErrorKind::ChangedFunctionParameterAbi { source_span, .. }
            | HotReloadErrorKind::ChangedFunctionReturnAbi { source_span, .. }
            | HotReloadErrorKind::ChangedFunctionEvent { source_span, .. }
            | HotReloadErrorKind::ChangedFunctionAsyncness { source_span, .. }
            | HotReloadErrorKind::ChangedFunctionEffects { source_span, .. }
            | HotReloadErrorKind::ChangedFunctionAccess { source_span, .. }
            | HotReloadErrorKind::RemovedMethodAbi { source_span, .. }
            | HotReloadErrorKind::ChangedMethodParameterAbi { source_span, .. }
            | HotReloadErrorKind::ChangedMethodReturnAbi { source_span, .. }
            | HotReloadErrorKind::ChangedMethodAsyncness { source_span, .. }
            | HotReloadErrorKind::ChangedMethodEffects { source_span, .. }
            | HotReloadErrorKind::ChangedMethodAccess { source_span, .. } => {
                source_span.as_deref().copied()
            }
            HotReloadErrorKind::RemovedTraitAbi { source_span, .. }
            | HotReloadErrorKind::ChangedTraitAbi { source_span, .. } => {
                source_span.as_deref().copied()
            }
            HotReloadErrorKind::RemovedModuleAbi { source_span, .. }
            | HotReloadErrorKind::ChangedModuleAbi { source_span, .. } => {
                source_span.as_deref().copied()
            }
            HotReloadErrorKind::DeletedFunctionParameters { .. }
            | HotReloadErrorKind::ChangedFunctionParameters { .. }
            | HotReloadErrorKind::AddedFunctionParametersWithoutDefaults { .. }
            | HotReloadErrorKind::AddedFunctionParametersDenied { .. }
            | HotReloadErrorKind::NewFunctionDenied { .. }
            | HotReloadErrorKind::RemovedFunction { .. } => None,
            HotReloadErrorKind::ChangedPackageProviderAbi { source_span, .. } => {
                source_span.as_deref().copied()
            }
            HotReloadErrorKind::ChangedStateStorage { source_span, .. }
            | HotReloadErrorKind::ChangedStateType { source_span, .. }
            | HotReloadErrorKind::MissingExternStateBinding { source_span, .. }
            | HotReloadErrorKind::InvalidExternStateBinding { source_span, .. }
            | HotReloadErrorKind::StateInitializerFailed { source_span, .. } => {
                source_span.as_deref().copied()
            }
        }
    }

    #[must_use]
    pub fn manifest_path(&self) -> Option<&Path> {
        match &self.kind {
            HotReloadErrorKind::ChangedPackageProviderAbi { manifest_path, .. } => {
                manifest_path.as_deref()
            }
            _ => None,
        }
    }
}

impl fmt::Display for HotReloadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.kind)
    }
}

impl std::error::Error for HotReloadError {}

#[derive(Clone, Debug, PartialEq)]
pub enum HotReloadErrorKind {
    DeletedFunctionParameters {
        function: String,
        old: Vec<String>,
        new: Vec<String>,
    },
    ChangedFunctionParameters {
        function: String,
        old: Vec<String>,
        new: Vec<String>,
    },
    ChangedFunctionParameterAbi {
        function: String,
        old: Vec<ParamAbi>,
        new: Vec<ParamAbi>,
        source_span: Option<Box<Span>>,
    },
    ChangedFunctionReturnAbi {
        function: String,
        old: Option<String>,
        new: Option<String>,
        source_span: Option<Box<Span>>,
    },
    AddedFunctionParametersWithoutDefaults {
        function: String,
        added: Vec<String>,
    },
    AddedFunctionParametersDenied {
        function: String,
        added: Vec<String>,
    },
    NewFunctionDenied {
        function: String,
    },
    RemovedFunction {
        function: String,
    },
    RemovedSchema {
        type_name: String,
        old_hash: u64,
        source_span: Option<Box<Span>>,
    },
    ChangedSchema {
        type_name: String,
        old_hash: u64,
        new_hash: u64,
        source_span: Option<Box<Span>>,
    },
    ChangedSchemaAbi {
        type_name: String,
        old: Box<SchemaAbi>,
        new: Box<SchemaAbi>,
        source_span: Option<Box<Span>>,
    },
    RemovedFunctionAbi {
        function: String,
        source_span: Option<Box<Span>>,
    },
    ChangedFunctionEvent {
        function: String,
        old: Option<String>,
        new: Option<String>,
        source_span: Option<Box<Span>>,
    },
    ChangedFunctionAsyncness {
        function: String,
        old: CallableAsyncness,
        new: CallableAsyncness,
        source_span: Option<Box<Span>>,
    },
    ChangedFunctionEffects {
        function: String,
        old: EffectAbi,
        new: EffectAbi,
        source_span: Option<Box<Span>>,
    },
    ChangedFunctionAccess {
        function: String,
        old: AccessAbi,
        new: AccessAbi,
        source_span: Option<Box<Span>>,
    },
    RemovedMethodAbi {
        type_name: String,
        method: String,
        source_span: Option<Box<Span>>,
    },
    ChangedMethodParameterAbi {
        type_name: String,
        method: String,
        old: Vec<ParamAbi>,
        new: Vec<ParamAbi>,
        source_span: Option<Box<Span>>,
    },
    ChangedMethodReturnAbi {
        type_name: String,
        method: String,
        old: Option<String>,
        new: Option<String>,
        source_span: Option<Box<Span>>,
    },
    ChangedMethodAsyncness {
        type_name: String,
        method: String,
        old: CallableAsyncness,
        new: CallableAsyncness,
        source_span: Option<Box<Span>>,
    },
    ChangedMethodEffects {
        type_name: String,
        method: String,
        old: EffectAbi,
        new: EffectAbi,
        source_span: Option<Box<Span>>,
    },
    ChangedMethodAccess {
        type_name: String,
        method: String,
        old: AccessAbi,
        new: AccessAbi,
        source_span: Option<Box<Span>>,
    },
    RemovedTraitAbi {
        trait_name: String,
        source_span: Option<Box<Span>>,
    },
    ChangedTraitAbi {
        trait_name: String,
        old: Vec<TraitMethodAbi>,
        new: Vec<TraitMethodAbi>,
        source_span: Option<Box<Span>>,
    },
    RemovedModuleAbi {
        module: String,
        source_span: Option<Box<Span>>,
    },
    ChangedModuleAbi {
        module: String,
        old: Vec<ModuleExportAbi>,
        new: Vec<ModuleExportAbi>,
        source_span: Option<Box<Span>>,
    },
    ChangedPackageProviderAbi {
        target: String,
        reason: String,
        source_span: Option<Box<Span>>,
        manifest_path: Option<PathBuf>,
    },
    ChangedStateStorage {
        state: String,
        old: StateStorage,
        new: StateStorage,
        source_span: Option<Box<Span>>,
    },
    ChangedStateType {
        state: String,
        old: Box<MirTypeContract>,
        new: Box<MirTypeContract>,
        source_span: Option<Box<Span>>,
    },
    MissingExternStateBinding {
        state: String,
        source_span: Option<Box<Span>>,
    },
    InvalidExternStateBinding {
        state: String,
        reason: String,
        source_span: Option<Box<Span>>,
    },
    StateInitializerFailed {
        state: String,
        reason: String,
        source_span: Option<Box<Span>>,
    },
}

pub type HotReloadResult<T> = Result<T, HotReloadError>;
