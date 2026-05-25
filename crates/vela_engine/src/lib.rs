//! Stable embedding API for installing schemas and native functions into Vela.

#![allow(clippy::result_large_err)]

mod args;
mod builder;
mod clock;
mod compiler_options;
mod context;
mod context_schema;
mod engine;
mod error;
mod metadata;
mod method;
mod native;
mod permission;
mod random;
mod reload;
mod runtime;
mod schema;
mod source;
mod typed;
mod validation;

pub use args::{FromScriptArg, IntoScriptArg, ScriptArgsExt};
pub use builder::EngineBuilder;
pub use clock::{CONTEXT_TIME_PERMISSION, CTX_NOW_FUNCTION_ID, CTX_TICK_FUNCTION_ID};
pub use context::NativeCallContext;
pub use context_schema::{
    CONTEXT_EMIT_METHOD_ID, CONTEXT_HOST_TYPE_ID, CONTEXT_LOG_METHOD_ID, CONTEXT_NOW_FIELD_ID,
    CONTEXT_TICK_FIELD_ID, CONTEXT_TYPE_ID, context_host_type_desc,
};
pub use engine::Engine;
pub use error::{EngineError, EngineErrorKind, EngineResult};
pub use method::{NativeMethodDesc, NativeMethodEntry, NativeMethodParamDesc};
pub use native::{
    ContextHostNativeFunctionEntry, EffectSet, FunctionAccess, HostNativeFunctionEntry,
    NativeFunctionDesc, NativeFunctionEntry, NativeFunctionId, NativeParamDesc, TypeHint,
};
pub use permission::PermissionSet;
pub use random::{CONTROLLED_RANDOM_PERMISSION, MATH_RANDOM_FUNCTION_ID};
pub use runtime::{CallOptions, Runtime};
pub use schema::{ScriptHostMethodMetadata, ScriptHostSchema, ScriptReflectSchema};
pub use source::{EngineSourceError, EngineSourceErrorKind};
pub use typed::{
    IntoNativeReturn, TypedContextHostNativeFunction, TypedHostNativeFunction, TypedNativeFunction,
    TypedNativeMethodFunction,
};
pub use vela_common::{FieldId, HostObjectId, HostTypeId};
pub use vela_host::{HostPath, HostRef, PathProxy};
pub use vela_hot_reload::{
    HotReloadPolicy, HotReloadReport, HotReloadResult, HotUpdate, ProgramVersion,
};
pub use vela_reflect::{ReflectPermission, ReflectPermissionSet, ReflectPolicy};
pub use vela_vm::Value;

#[cfg(test)]
mod tests;
