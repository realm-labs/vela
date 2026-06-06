use vela_common::{FieldId, HostMethodId, HostTypeId, MethodId, TraitId, TypeId, VariantId};
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
use crate::standard::{INT_TYPE_ID, MATH_CLAMP_FUNCTION_ID};

use super::{player_type, trait_desc_with_id};

mod host_method_validation;
mod module_type_validation;
mod native_function_validation;
