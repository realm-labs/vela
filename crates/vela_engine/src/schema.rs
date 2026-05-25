use vela_reflect::TypeDesc;

use crate::NativeMethodDesc;

pub trait ScriptHostSchema {
    fn script_host_type_desc() -> TypeDesc;
}

pub trait ScriptReflectSchema {
    fn script_reflect_type_desc() -> TypeDesc;
}

pub trait ScriptHostMethodMetadata {
    fn script_host_method_descs() -> Vec<NativeMethodDesc>;
}
