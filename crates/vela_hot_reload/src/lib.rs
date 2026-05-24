//! Function-level hot reload program versioning.

mod abi;
mod compile;
mod error;
mod runtime;
mod symbol;
mod version;

pub use abi::{AccessAbi, EffectAbi, FunctionAbi, HotReloadAbi, MethodAbi, SchemaAbi};
pub use compile::{
    compile_initial, compile_initial_with_abi, compile_initial_with_abi_and_options,
    compile_initial_with_options, compile_update, compile_update_with_abi,
    compile_update_with_abi_and_options, compile_update_with_options,
};
pub use error::{HotReloadError, HotReloadErrorKind, HotReloadResult};
pub use runtime::HotReloadRuntime;
pub use symbol::{FunctionSymbolId, ProgramVersionId};
pub use version::{HotUpdate, ProgramVersion};

#[cfg(test)]
mod tests;
