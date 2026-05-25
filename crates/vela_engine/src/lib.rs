//! Stable embedding API for installing schemas and native functions into Vela.

mod builder;
mod engine;
mod error;
mod method;
mod native;
mod permission;
mod random;
mod reload;
mod source;

pub use builder::EngineBuilder;
pub use engine::Engine;
pub use error::{EngineError, EngineErrorKind, EngineResult};
pub use method::{NativeMethodDesc, NativeMethodEntry, NativeMethodParamDesc};
pub use native::{
    EffectSet, FunctionAccess, HostNativeFunctionEntry, NativeFunctionDesc, NativeFunctionEntry,
    NativeFunctionId, NativeParamDesc, TypeHint,
};
pub use permission::PermissionSet;
pub use random::{CONTROLLED_RANDOM_PERMISSION, MATH_RANDOM_FUNCTION_ID};
pub use source::{EngineSourceError, EngineSourceErrorKind};
pub use vela_hot_reload::HotReloadPolicy;
pub use vela_reflect::{ReflectPermission, ReflectPermissionSet, ReflectPolicy};

#[cfg(test)]
mod tests;
