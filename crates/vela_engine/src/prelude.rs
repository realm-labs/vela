//! Common imports for embedding Vela in a Rust host.

pub use crate::args::{FromScriptArg, IntoHostArg, IntoScriptArg, ScriptArgsExt, host};
pub use crate::builder::EngineBuilder;
pub use crate::context::NativeCallContext;
pub use crate::engine::Engine;
pub use crate::method::NativeMethodDesc;
pub use crate::native::{
    EffectSet, FunctionAccess, NativeFunctionDesc, NativeFunctionId, TypeHint,
};
pub use crate::permission::{Capability, CapabilitySet, ExecutionProfile};
pub use crate::reload::{
    EngineHotReloadSourceError, EngineHotReloadSourceErrorKind, EngineHotReloadSourceResult,
};
pub use crate::runtime::{CallArgs, CallOptions, EventCallSafePointReport, Runtime};
pub use crate::schema::{ScriptHostMethodMetadata, ScriptHostSchema, ScriptReflectSchema};
pub use crate::source::{EngineSourceError, EngineSourceErrorKind};
pub use crate::{args, host};
pub use vela_bytecode::{
    CodeObject,
    script_methods::{ScriptMethod, ScriptMethodTable},
};
pub use vela_common::{
    FieldId, FunctionId, HostMethodId, HostObjectId, HostTypeId, MethodId, SourceId, TraitId,
    TypeId, VariantId,
};
pub use vela_hir::ids::{HirDeclId, ModuleId};
pub use vela_hir::module_graph::{
    Declaration, DeclarationIndex, DeclarationKind, Import, ImportResolution, ModuleGraph,
    ModulePath, ModuleSource, ResolvedImport,
};
pub use vela_host::adapter::ScriptStateAdapter;
pub use vela_host::path::{HostPath, HostRef};
pub use vela_host::proxy::PathProxy;
pub use vela_host::tx::PatchTx;
pub use vela_host::value::HostValue;
pub use vela_hot_reload::error::{HotReloadError, HotReloadErrorKind, HotReloadResult};
pub use vela_hot_reload::policy::HotReloadPolicy;
pub use vela_hot_reload::report::{HotReloadDiagnostic, HotReloadReport};
pub use vela_hot_reload::report_detail::HotReloadDiagnosticDetail;
pub use vela_hot_reload::report_render::{HotReloadReportLine, HotReloadReportLineKind};
pub use vela_hot_reload::symbol::ProgramVersionId;
pub use vela_hot_reload::version::{HotUpdate, ProgramVersion};
pub use vela_reflect::permissions::{ReflectPermission, ReflectPermissionSet, ReflectPolicy};
pub use vela_reflect::registry::{
    FieldDesc, MethodDesc, MethodParamDesc, SchemaHash, TraitDesc, TraitMethodDesc, TypeDesc,
    TypeKey, TypeKind, VariantDesc,
};
pub use vela_vm::owned_value::OwnedValue;
