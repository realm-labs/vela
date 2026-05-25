//! Stable embedding API for installing schemas and native functions into Vela.

mod args;
mod builder;
mod context;
mod engine;
mod error;
mod method;
mod native;
mod permission;
mod random;
mod reload;
mod runtime;
mod schema;
mod source;
mod typed;

pub use args::{FromScriptArg, IntoScriptArg, ScriptArgsExt};
pub use builder::EngineBuilder;
pub use context::NativeCallContext;
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
pub use vela_common::{HostObjectId, HostTypeId};
pub use vela_host::HostRef;
pub use vela_hot_reload::HotReloadPolicy;
pub use vela_reflect::{ReflectPermission, ReflectPermissionSet, ReflectPolicy};
pub use vela_vm::Value;

#[cfg(test)]
mod tests;
