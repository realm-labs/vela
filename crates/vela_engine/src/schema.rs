use vela_reflect::registry::TypeDesc;

use crate::type_binding::TypeBinding;
use crate::{args::FromScriptArg, args::IntoScriptArg};

pub trait ScriptHostSchema: Sized {
    fn script_host_type_desc() -> TypeDesc;

    fn script_host_binding() -> TypeBinding<Self>
    where
        Self: 'static,
    {
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
