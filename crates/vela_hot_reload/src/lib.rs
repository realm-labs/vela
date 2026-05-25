//! Function-level hot reload program versioning.

mod abi;
mod compile;
mod error;
mod function_signature;
mod policy;
mod report;
mod report_detail;
mod report_render;
mod runtime;
mod symbol;
mod version;

pub use abi::{
    AccessAbi, EffectAbi, FunctionAbi, HotReloadAbi, MethodAbi, ParamAbi, SchemaAbi, TraitAbi,
    TraitMethodAbi,
};
pub use compile::{
    compile_initial, compile_initial_with_abi, compile_initial_with_abi_and_options,
    compile_initial_with_options, compile_update, compile_update_with_abi,
    compile_update_with_abi_and_options, compile_update_with_abi_and_options_and_policy,
    compile_update_with_abi_and_policy, compile_update_with_options, compile_update_with_policy,
};
pub use error::{HotReloadError, HotReloadErrorKind, HotReloadResult};
pub use policy::HotReloadPolicy;
pub use report::{HotReloadDiagnostic, HotReloadReport};
pub use report_detail::HotReloadDiagnosticDetail;
pub use report_render::{HotReloadReportLine, HotReloadReportLineKind};
pub use runtime::HotReloadRuntime;
pub use symbol::{FunctionSymbolId, ProgramVersionId};
pub use version::{HotUpdate, ProgramVersion};

#[cfg(test)]
mod tests;
