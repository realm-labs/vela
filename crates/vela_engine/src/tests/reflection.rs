use vela_common::{SourceId, Span};
use vela_def::{FieldId, TypeId};
use vela_hot_reload::abi::{AccessAbi, FunctionAbi};
use vela_reflect::access::FieldAccess;
use vela_reflect::error::ReflectErrorKind;
use vela_reflect::modules::ModuleDesc;
use vela_reflect::permissions::{ReflectPermission, ReflectPermissionSet};
use vela_reflect::registry::{FieldDesc, TypeDesc, TypeKey};
use vela_vm::HostExecution;
use vela_vm::error::VmErrorKind;

use crate::engine::Engine;
use crate::native::{EffectSet, FunctionAccess, NativeFunctionDesc, NativeFunctionId, TypeHint};
use crate::schema::ScriptReflectSchema;

struct ReflectOnlyPlayer;

impl ScriptReflectSchema for ReflectOnlyPlayer {
    fn script_reflect_type_desc() -> TypeDesc {
        TypeDesc::new(TypeKey::new(TypeId::new(9901), "ReflectOnlyPlayer"))
            .kind(vela_reflect::registry::TypeKind::ScriptStruct)
            .field(FieldDesc::new(FieldId::new(1), "level"))
    }
}

mod metadata;
mod permissions;
mod reflect_calls;
mod reflection_natives;
