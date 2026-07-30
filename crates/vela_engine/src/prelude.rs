//! Common imports for embedding Vela in a Rust host.

pub use crate::args::{FromScriptArg, IntoHostArg, IntoScriptArg, ScriptArgsExt, host};
pub use crate::builder::EngineBuilder;
pub use crate::context::NativeCallContext;
pub use crate::engine::Engine;
pub use crate::io::FsSandbox;
pub use crate::native::{
    EffectSet, FunctionAccess, NativeFunctionDesc, NativeFunctionId, TypeHint,
};
pub use crate::permission::{Capability, CapabilitySet, ExecutionProfile};
pub use crate::reload::{
    EngineHotReloadSourceError, EngineHotReloadSourceErrorKind, EngineHotReloadSourceResult,
};
pub use crate::runtime::{
    CallArgs, CallOptions, Runtime, RuntimeBuildError, RuntimeBuilder, RuntimeCallFuture,
    RuntimeInitializationLimits, VelaFunction, VelaMethod, VelaMethodTarget, VelaValue,
};
pub use crate::schema::{ScriptHostSchema, ScriptReflectSchema, ScriptValueSchema};
pub use crate::service::{
    PatchEdit, PatchRevision, PatchRevisionChecksum, PatchSources, Service,
    ServiceDomainBuildError, ServiceDryRunReport, ServicePatch, ServicePatchError,
    ServicePatchWorkspaceError, ServiceRuntimeAuthority, ServiceRuntimeSlot, ServiceUpdateBundle,
};
#[cfg(feature = "artifact-codec")]
pub use crate::service::{PortableServiceBundleError, PortableServiceUpdateBundle};
pub use crate::source::{EngineSourceError, EngineSourceErrorKind};
pub use crate::type_registration::VelaType;
pub use crate::{args, host};
pub use vela_hot_reload::report::{HotReloadDiagnostic, HotReloadReport};
pub use vela_hot_reload::version::{HotUpdate, ProgramVersion};
pub use vela_vm::owned_value::OwnedValue;
#[cfg(feature = "serde")]
pub use vela_vm::serde::{from_owned_value, to_owned_value};
pub use vela_vm::{owned_array, owned_enum, owned_map, owned_record, owned_set};
