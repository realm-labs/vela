use vela_bytecode::compiler::{compile_program_source, compile_program_source_with_options};
use vela_common::{FieldId, HostMethodId, HostObjectId, HostTypeId, SourceId, Span, TypeId};
use vela_host::mock::MockStateAdapter;
use vela_host::patch::PatchOp;
use vela_host::path::{HostPath, HostRef};
use vela_host::tx::PatchTx;
use vela_host::value::HostValue;
use vela_hot_reload::abi::{AccessAbi, FunctionAbi, MethodAbi};
use vela_reflect::access::FieldAccess;
use vela_reflect::error::ReflectErrorKind;
use vela_reflect::modules::ModuleDesc;
use vela_reflect::permissions::{ReflectPermission, ReflectPermissionSet};
use vela_reflect::registry::{FieldDesc, MethodDesc, TypeDesc, TypeKey};
use vela_vm::HostExecution;
use vela_vm::error::VmErrorKind;

use crate::engine::Engine;
use crate::method::NativeMethodDesc;
use crate::native::{EffectSet, FunctionAccess, NativeFunctionDesc, NativeFunctionId, TypeHint};
use crate::permission::Capability;
use crate::schema::{ScriptHostMethodMetadata, ScriptReflectSchema};

use super::player_type;

struct ReflectOnlyPlayer;

impl ScriptReflectSchema for ReflectOnlyPlayer {
    fn script_reflect_type_desc() -> TypeDesc {
        TypeDesc::new(TypeKey::new(TypeId::new(9901), "ReflectOnlyPlayer"))
            .kind(vela_reflect::registry::TypeKind::Host)
            .host_type(HostTypeId::new(9901))
            .field(FieldDesc::new(FieldId::new(1), "level"))
    }
}

struct MetadataOnlyPlayerMethods;

impl ScriptHostMethodMetadata for MetadataOnlyPlayerMethods {
    fn script_host_method_descs() -> Vec<NativeMethodDesc> {
        vec![
            NativeMethodDesc::new(
                TypeKey::new(TypeId::new(1), "Player"),
                HostMethodId::new(44),
                "metadata_bonus",
            )
            .param("amount", TypeHint::Int)
            .returns(TypeHint::Int)
            .effects(EffectSet::host_read()),
        ]
    }
}

mod metadata;
mod permissions;
mod reflect_calls;
mod reflection_natives;
