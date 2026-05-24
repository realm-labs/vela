//! Stable embedding API for installing schemas and native functions into Vela.

mod builder;
mod engine;
mod error;
mod native;

pub use builder::EngineBuilder;
pub use engine::Engine;
pub use error::{EngineError, EngineErrorKind, EngineResult};
pub use native::{
    EffectSet, FunctionAccess, NativeFunctionDesc, NativeFunctionEntry, NativeFunctionId,
    NativeParamDesc, TypeHint,
};

#[cfg(test)]
mod tests;
