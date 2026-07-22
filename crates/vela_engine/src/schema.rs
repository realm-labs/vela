use vela_reflect::registry::TypeDesc;

use crate::builder::EngineBuilder;
use crate::method::NativeMethodDesc;
use crate::type_binding::TypeBinding;
use crate::{args::FromScriptArg, args::IntoScriptArg};

pub trait ScriptHostSchema: Sized + 'static {
    fn script_host_type_desc() -> TypeDesc;

    fn script_host_binding() -> TypeBinding<Self> {
        TypeBinding::host(Self::script_host_type_desc())
    }
}

pub trait ScriptReflectSchema {
    fn script_reflect_type_desc() -> TypeDesc;
}

/// Generated structural Value metadata and its unified TypeBinding entry.
pub trait ScriptValueSchema: IntoScriptArg + FromScriptArg + Sized + 'static {
    fn script_value_type_desc() -> TypeDesc;

    fn script_value_binding() -> TypeBinding<Self> {
        TypeBinding::value(Self::script_value_type_desc())
    }
}

pub trait ScriptHostMethodMetadata {
    fn script_host_method_descs() -> Vec<NativeMethodDesc>;

    fn register_script_host_methods(builder: EngineBuilder) -> EngineBuilder {
        Self::script_host_method_descs()
            .into_iter()
            .fold(builder, EngineBuilder::register_host_method_desc)
    }
}
