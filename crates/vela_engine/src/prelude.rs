//! Common imports for embedding Vela in a Rust host.

pub use crate::args::host;
pub use crate::builder::EngineBuilder;
pub use crate::context::NativeCallContext;
pub use crate::engine::Engine;
pub use crate::method::NativeMethodDesc;
pub use crate::native::{
    EffectSet, FunctionAccess, NativeFunctionDesc, NativeFunctionId, TypeHint,
};
pub use crate::permission::PermissionSet;
pub use crate::runtime::{
    CallOptions, PatchApplySafePointError, PatchApplySafePointReport, Runtime,
};
pub use crate::schema::{ScriptHostMethodMetadata, ScriptHostSchema, ScriptReflectSchema};
pub use crate::{args, host};
pub use vela_common::{
    FieldId, FunctionId, HostMethodId, HostObjectId, HostTypeId, MethodId, SourceId, TraitId,
    TypeId, VariantId,
};
pub use vela_host::adapter::ScriptStateAdapter;
pub use vela_host::path::{HostPath, HostRef};
pub use vela_host::proxy::PathProxy;
pub use vela_host::tx::PatchTx;
pub use vela_host::value::HostValue;
pub use vela_hot_reload::policy::HotReloadPolicy;
pub use vela_reflect::permissions::{ReflectPermission, ReflectPermissionSet, ReflectPolicy};
pub use vela_reflect::registry::{
    FieldDesc, MethodDesc, MethodParamDesc, SchemaHash, TraitDesc, TraitMethodDesc, TypeDesc,
    TypeKey, TypeKind, VariantDesc,
};
pub use vela_vm::value::Value;
