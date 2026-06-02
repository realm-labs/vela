use vela_bytecode::compiler::options::CompilerOptions;
use vela_reflect::registry::TypeRegistry;

pub(crate) fn compiler_options_from_registry(registry: &TypeRegistry) -> CompilerOptions {
    let mut options = CompilerOptions::new();
    for module in registry.modules() {
        if let Some(root) = module.name.split('.').next() {
            options = options.with_native_module_root(root);
        }
    }
    for function in registry.functions() {
        options = options.with_native_function_params(
            function.name.clone(),
            function.params.iter().map(|param| param.name.clone()),
        );
    }
    for desc in registry.types() {
        options = options.with_host_type(desc.key.name.clone());
        for field in &desc.fields {
            options = options.with_host_field(field.name.clone(), field.id);
        }
        for variant in &desc.variants {
            for field in &variant.fields {
                options = options.with_host_variant_field(field.name.clone(), field.id);
            }
        }
        if desc.host_type_id.is_some() {
            for method in &desc.methods {
                options = options.with_host_method(method.name.clone(), method.id);
                if !method.params.is_empty() {
                    options = options.with_host_method_params(
                        method.id,
                        method
                            .params
                            .iter()
                            .map(|param| (param.name.clone(), param.has_default)),
                    );
                }
                options = options.with_host_method_for_type(
                    desc.key.name.clone(),
                    method.name.clone(),
                    method.id,
                );
            }
        }
    }
    options
}
