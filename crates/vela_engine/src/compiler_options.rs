use vela_bytecode::compiler::CompilerOptions;
use vela_reflect::TypeRegistry;

pub(crate) fn compiler_options_from_registry(registry: &TypeRegistry) -> CompilerOptions {
    let mut options = CompilerOptions::new();
    for desc in registry.types() {
        options = options.with_host_type(desc.key.name.clone());
        for field in &desc.fields {
            options = options.with_host_field(field.name.clone(), field.id);
        }
        for method in &desc.methods {
            options = options
                .with_host_method(method.name.clone(), method.id)
                .with_host_method_for_type(desc.key.name.clone(), method.name.clone(), method.id);
        }
    }
    options
}
