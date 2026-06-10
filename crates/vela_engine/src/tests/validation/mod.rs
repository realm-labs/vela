use vela_common::{HostMethodId, HostTypeId};
use vela_def::{FieldId, MethodId, TraitId, TypeId, VariantId};
use vela_reflect::access::FieldAccess;
use vela_reflect::modules::ModuleDesc;
use vela_reflect::registry::{
    FieldDesc, MethodDesc, MethodParamDesc, TraitMethodDesc, TypeDesc, TypeKey, TypeKind,
    VariantDesc,
};

use vela_vm::owned_value::OwnedValue;

use crate::engine::Engine;
use crate::error::EngineErrorKind;
use crate::method::NativeMethodDesc;
use crate::native::{NativeFunctionDesc, NativeFunctionId, TypeHint};

use super::{player_type, trait_desc_with_id};

mod host_method_validation;
mod module_type_validation;
mod native_function_validation;

fn standard_type_id(name: &str) -> TypeId {
    let Some(id) = vela_stdlib::std_type_id(name) else {
        panic!("missing standard type identity for {name}");
    };
    id
}
