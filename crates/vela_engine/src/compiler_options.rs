use vela_bytecode::compiler::options::CompilerOptions;
use vela_reflect::registry::TypeRegistry;

pub(crate) fn compiler_options_from_registry(registry: &TypeRegistry) -> CompilerOptions {
    let mut options = CompilerOptions::new();
    for module in registry.modules() {
        if let Some(root) = module.name.split("::").next() {
            options = options.with_native_module_root(root);
        }
    }
    options
}
