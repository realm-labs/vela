use vela_bytecode::ProgramImage;
use vela_hot_reload::runtime::HotReloadRuntime;
use vela_vm::error::{VmError, VmErrorKind};
use vela_vm::heap::{HeapValue, ScriptHeap};
use vela_vm::value::Value;

use crate::engine::Engine;

pub(super) fn runtime_vm(
    engine: &Engine,
    image: &ProgramImage,
    hot_reload: Option<&HotReloadRuntime>,
) -> vela_vm::Vm {
    if let Some(hot_reload) = hot_reload {
        let current = hot_reload.current();
        engine.into_vm_for_program_image_with_abi(image, current.abi())
    } else {
        engine.into_vm_for_program_image(image)
    }
}

pub(super) fn value_type_id(
    value: &Value,
    heap: &ScriptHeap,
    registry: &vela_reflect::registry::TypeRegistry,
    resolve_host: impl FnOnce(vela_host::path::HostSlotRef) -> Option<vela_host::path::HostRef>,
) -> Option<vela_def::TypeId> {
    match value {
        Value::HeapRef(reference) => match heap.get(*reference)? {
            HeapValue::Record {
                identity: Some(identity),
                ..
            } => Some(identity.type_id),
            HeapValue::Enum {
                identity: Some(identity),
                ..
            } => Some(identity.type_id),
            _ => None,
        },
        Value::HostRef(reference) => resolve_host(*reference)
            .and_then(|reference| registry.type_of_host(reference))
            .map(|desc| desc.key.id),
        _ => None,
    }
}

pub(super) fn unknown_function(name: String) -> VmError {
    VmError::new(VmErrorKind::UnknownFunction { name })
}

pub(super) fn unknown_method(method: String) -> VmError {
    VmError::new(VmErrorKind::UnknownMethod { method })
}
