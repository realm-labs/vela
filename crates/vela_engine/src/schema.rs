use vela_reflect::TypeDesc;

use crate::{EngineBuilder, NativeMethodDesc};

pub trait ScriptHostSchema {
    fn script_host_type_desc() -> TypeDesc;
}

pub trait ScriptReflectSchema {
    fn script_reflect_type_desc() -> TypeDesc;
}

pub trait ScriptHostMethodMetadata {
    fn script_host_method_descs() -> Vec<NativeMethodDesc>;

    fn register_script_host_methods(builder: EngineBuilder) -> EngineBuilder {
        Self::script_host_method_descs()
            .into_iter()
            .fold(builder, EngineBuilder::register_host_method_desc)
    }
}
