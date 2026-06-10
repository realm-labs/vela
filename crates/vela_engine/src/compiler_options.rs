use vela_bytecode::compiler::options::{CompilerOptions, HostIndexCapabilityInfo};
use vela_reflect::registry::TypeRegistry;

pub(crate) fn compiler_options_from_registry(registry: &TypeRegistry) -> CompilerOptions {
    let mut options = CompilerOptions::new();
    for module in registry.modules() {
        if let Some(root) = module.name.split("::").next() {
            options = options.with_native_module_root(root);
        }
    }
    for desc in registry.types() {
        if let Some(index) = &desc.index_capability {
            options = options.with_host_index_capability(
                desc.key.name.clone(),
                HostIndexCapabilityInfo {
                    readable: index.readable,
                    writable: index.writable,
                    addable: index.addable,
                    removable: index.removable,
                    key_type: index.key_type.clone(),
                    value_type: index.value_type.clone(),
                },
            );
        }
    }
    options
}
