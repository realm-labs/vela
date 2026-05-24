//! Stable embedding API for installing schemas and native functions into Vela.

mod builder;
mod engine;
mod error;
mod method;
mod native;
mod permission;

pub use builder::EngineBuilder;
pub use engine::Engine;
pub use error::{EngineError, EngineErrorKind, EngineResult};
pub use method::{NativeMethodDesc, NativeMethodEntry, NativeMethodParamDesc};
pub use native::{
    EffectSet, FunctionAccess, HostNativeFunctionEntry, NativeFunctionDesc, NativeFunctionEntry,
    NativeFunctionId, NativeParamDesc, TypeHint,
};
pub use permission::PermissionSet;
pub use vela_reflect::{ReflectPermission, ReflectPermissionSet};

#[cfg(test)]
mod tests;
