use std::collections::HashMap;

use vela_bytecode::compiler::options::CompilerOptions;
use vela_reflect::registry::MethodDesc;
use vela_reflect::registry::TypeRegistry;

pub(crate) fn compiler_options_from_registry(registry: &TypeRegistry) -> CompilerOptions {
    let mut options = CompilerOptions::new();
    let mut value_method_params = HashMap::new();
    for module in registry.modules() {
        if let Some(root) = module.name.split("::").next() {
            options = options.with_native_module_root(root);
        }
    }
    for function in registry.functions() {
        options = options.with_native_function(
            function.name.clone(),
            function.id,
            function.params.iter().map(|param| param.name.clone()),
        );
    }
    for desc in registry.types() {
        options = options.with_host_type(desc.key.name.clone());
        for field in &desc.fields {
            options = options.with_host_field(field.name.clone(), field.id);
            options = options.with_host_field_for_type(
                desc.key.name.clone(),
                field.name.clone(),
                field.id,
                field.access.writable,
            );
        }
        for variant in &desc.variants {
            for field in &variant.fields {
                options = options.with_host_variant_field(field.name.clone(), field.id);
            }
        }
        for method in desc
            .methods
            .iter()
            .filter(|method| method.attrs.get("stdlib").is_some())
        {
            options = options.with_value_method_for_type(
                desc.key.name.clone(),
                method.name.clone(),
                method.id,
                method_params(method),
            );
        }
        collect_value_method_params(&mut value_method_params, &desc.methods);
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
    for (method, params) in value_method_params {
        if let Some(params) = params {
            options = options.with_value_method_params(method, params);
        }
    }
    options
}

fn method_params(method: &MethodDesc) -> Vec<(String, bool)> {
    method
        .params
        .iter()
        .map(|param| (param.name.clone(), param.has_default))
        .collect()
}

fn collect_value_method_params(
    value_method_params: &mut HashMap<String, Option<Vec<(String, bool)>>>,
    methods: &[MethodDesc],
) {
    for method in methods
        .iter()
        .filter(|method| method.attrs.get("stdlib").is_some())
    {
        let params = method_params(method);
        match value_method_params.get_mut(&method.name) {
            Some(Some(existing)) if existing == &params => {}
            Some(existing) => {
                *existing = None;
            }
            None => {
                value_method_params.insert(method.name.clone(), Some(params));
            }
        }
    }
}
