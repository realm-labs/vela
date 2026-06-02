use std::collections::BTreeSet;

use vela_reflect::modules::ModuleDesc;
use vela_reflect::registry::{MethodParamDesc, TypeDesc};

use crate::error::{EngineError, EngineErrorKind, EngineResult};
use crate::native::{
    ContextHostNativeFunctionEntry, HostNativeFunctionEntry, NativeFunctionDesc,
    NativeFunctionEntry,
};

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ModuleValidationOptions {
    include_standard_modules: bool,
    include_context_module: bool,
}

impl ModuleValidationOptions {
    pub(crate) const fn include_standard_modules(mut self, include: bool) -> Self {
        self.include_standard_modules = include;
        self
    }

    pub(crate) const fn include_context_module(mut self, include: bool) -> Self {
        self.include_context_module = include;
        self
    }
}

pub(crate) fn validate_modules(
    modules: &[ModuleDesc],
    options: ModuleValidationOptions,
) -> EngineResult<()> {
    let mut names = BTreeSet::new();
    if options.include_standard_modules {
        for module in crate::standard::standard_module_descs() {
            validate_module_desc(&module, &mut names)?;
        }
    }
    if options.include_context_module {
        validate_module_desc(&crate::clock::context_module_desc(), &mut names)?;
    }
    for module in modules {
        validate_module_desc(module, &mut names)?;
    }
    Ok(())
}

fn validate_module_desc(module: &ModuleDesc, names: &mut BTreeSet<String>) -> EngineResult<()> {
    if !names.insert(module.name.clone()) {
        return Err(EngineError::new(EngineErrorKind::DuplicateModuleName {
            name: module.name.clone(),
        }));
    }
    Ok(())
}

pub(crate) fn validate_types(types: &[TypeDesc], include_standard_types: bool) -> EngineResult<()> {
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();
    let mut host_ids = BTreeSet::new();
    let mut host_method_ids = BTreeSet::new();
    let mut host_method_names = BTreeSet::new();

    if include_standard_types {
        for desc in crate::standard::standard_type_descs() {
            validate_type_desc(
                &desc,
                &mut ids,
                &mut names,
                &mut host_ids,
                &mut host_method_ids,
                &mut host_method_names,
            )?;
        }
    }

    for desc in types {
        validate_type_desc(
            desc,
            &mut ids,
            &mut names,
            &mut host_ids,
            &mut host_method_ids,
            &mut host_method_names,
        )?;
    }

    Ok(())
}

fn validate_type_desc(
    desc: &TypeDesc,
    ids: &mut BTreeSet<vela_common::TypeId>,
    names: &mut BTreeSet<String>,
    host_ids: &mut BTreeSet<vela_common::HostTypeId>,
    host_method_ids: &mut BTreeSet<vela_common::HostMethodId>,
    host_method_names: &mut BTreeSet<(String, String)>,
) -> EngineResult<()> {
    if !ids.insert(desc.key.id) {
        return Err(EngineError::new(EngineErrorKind::DuplicateTypeId {
            id: desc.key.id.get(),
        }));
    }
    if !names.insert(desc.key.name.clone()) {
        return Err(EngineError::new(EngineErrorKind::DuplicateTypeName {
            name: desc.key.name.clone(),
        }));
    }
    if let Some(host_type_id) = desc.host_type_id
        && !host_ids.insert(host_type_id)
    {
        return Err(EngineError::new(EngineErrorKind::DuplicateHostTypeId {
            id: host_type_id.get(),
        }));
    }

    validate_type_fields(desc)?;
    validate_type_variants(desc)?;
    validate_type_traits(desc)?;

    for method in &desc.methods {
        if !host_method_ids.insert(method.id) {
            return Err(EngineError::new(EngineErrorKind::DuplicateHostMethodId {
                id: method.id.get(),
            }));
        }
        if !host_method_names.insert((desc.key.name.clone(), method.name.clone())) {
            return Err(EngineError::new(EngineErrorKind::DuplicateHostMethodName {
                name: method.name.clone(),
            }));
        }
        validate_host_method_params(desc.key.name.as_str(), method.name.as_str(), &method.params)?;
    }

    Ok(())
}

fn validate_type_fields(desc: &TypeDesc) -> EngineResult<()> {
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();
    for field in &desc.fields {
        if !ids.insert(field.id) {
            return Err(EngineError::new(EngineErrorKind::DuplicateFieldId {
                type_name: desc.key.name.clone(),
                id: field.id.get(),
            }));
        }
        if !names.insert(field.name.as_str()) {
            return Err(EngineError::new(EngineErrorKind::DuplicateFieldName {
                type_name: desc.key.name.clone(),
                name: field.name.clone(),
            }));
        }
    }
    Ok(())
}

fn validate_type_variants(desc: &TypeDesc) -> EngineResult<()> {
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();
    for variant in &desc.variants {
        if !ids.insert(variant.id) {
            return Err(EngineError::new(EngineErrorKind::DuplicateVariantId {
                type_name: desc.key.name.clone(),
                id: variant.id.get(),
            }));
        }
        if !names.insert(variant.name.as_str()) {
            return Err(EngineError::new(EngineErrorKind::DuplicateVariantName {
                type_name: desc.key.name.clone(),
                name: variant.name.clone(),
            }));
        }
        validate_variant_fields(desc.key.name.as_str(), variant)?;
    }
    Ok(())
}

fn validate_variant_fields(
    type_name: &str,
    variant: &vela_reflect::registry::VariantDesc,
) -> EngineResult<()> {
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();
    for field in &variant.fields {
        if !ids.insert(field.id) {
            return Err(EngineError::new(EngineErrorKind::DuplicateVariantFieldId {
                type_name: type_name.to_owned(),
                variant: variant.name.clone(),
                id: field.id.get(),
            }));
        }
        if !names.insert(field.name.as_str()) {
            return Err(EngineError::new(
                EngineErrorKind::DuplicateVariantFieldName {
                    type_name: type_name.to_owned(),
                    variant: variant.name.clone(),
                    name: field.name.clone(),
                },
            ));
        }
    }
    Ok(())
}

fn validate_type_traits(desc: &TypeDesc) -> EngineResult<()> {
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();
    for trait_desc in &desc.traits {
        if !ids.insert(trait_desc.id) {
            return Err(EngineError::new(EngineErrorKind::DuplicateTraitId {
                type_name: desc.key.name.clone(),
                id: trait_desc.id.get(),
            }));
        }
        if !names.insert(trait_desc.name.as_str()) {
            return Err(EngineError::new(EngineErrorKind::DuplicateTraitName {
                type_name: desc.key.name.clone(),
                name: trait_desc.name.clone(),
            }));
        }
        validate_trait_methods(desc.key.name.as_str(), trait_desc)?;
    }
    Ok(())
}

fn validate_trait_methods(
    type_name: &str,
    trait_desc: &vela_reflect::registry::TraitDesc,
) -> EngineResult<()> {
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();
    for method in &trait_desc.methods {
        if !ids.insert(method.id) {
            return Err(EngineError::new(EngineErrorKind::DuplicateTraitMethodId {
                type_name: type_name.to_owned(),
                trait_name: trait_desc.name.clone(),
                id: method.id.get(),
            }));
        }
        if !names.insert(method.name.as_str()) {
            return Err(EngineError::new(
                EngineErrorKind::DuplicateTraitMethodName {
                    type_name: type_name.to_owned(),
                    trait_name: trait_desc.name.clone(),
                    name: method.name.clone(),
                },
            ));
        }
        validate_trait_method_params(
            type_name,
            trait_desc.name.as_str(),
            method.name.as_str(),
            &method.params,
        )?;
    }
    Ok(())
}

fn validate_host_method_params(
    type_name: &str,
    method: &str,
    params: &[MethodParamDesc],
) -> EngineResult<()> {
    let mut names = BTreeSet::new();
    for param in params {
        if !names.insert(param.name.as_str()) {
            return Err(EngineError::new(
                EngineErrorKind::DuplicateHostMethodParamName {
                    type_name: type_name.to_owned(),
                    method: method.to_owned(),
                    name: param.name.clone(),
                },
            ));
        }
    }
    Ok(())
}

fn validate_trait_method_params(
    type_name: &str,
    trait_name: &str,
    method: &str,
    params: &[MethodParamDesc],
) -> EngineResult<()> {
    let mut names = BTreeSet::new();
    for param in params {
        if !names.insert(param.name.as_str()) {
            return Err(EngineError::new(
                EngineErrorKind::DuplicateTraitMethodParamName {
                    type_name: type_name.to_owned(),
                    trait_name: trait_name.to_owned(),
                    method: method.to_owned(),
                    name: param.name.clone(),
                },
            ));
        }
    }
    Ok(())
}

pub(crate) fn validate_native_functions(
    functions: &[NativeFunctionEntry],
    host_functions: &[HostNativeFunctionEntry],
    context_host_functions: &[ContextHostNativeFunctionEntry],
    include_standard_natives: bool,
) -> EngineResult<()> {
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();

    if include_standard_natives {
        for desc in crate::standard::standard_native_function_descs() {
            validate_native_function_desc(&desc, &mut ids, &mut names)?;
        }
    }

    for desc in functions
        .iter()
        .map(|entry| &entry.desc)
        .chain(host_functions.iter().map(|entry| &entry.desc))
        .chain(context_host_functions.iter().map(|entry| &entry.desc))
    {
        validate_native_function_desc(desc, &mut ids, &mut names)?;
    }

    Ok(())
}

fn validate_native_function_desc(
    desc: &NativeFunctionDesc,
    ids: &mut BTreeSet<crate::native::NativeFunctionId>,
    names: &mut BTreeSet<String>,
) -> EngineResult<()> {
    if !ids.insert(desc.id) {
        return Err(EngineError::new(
            EngineErrorKind::DuplicateNativeFunctionId { id: desc.id.get() },
        ));
    }
    if !names.insert(desc.name.clone()) {
        return Err(EngineError::new(
            EngineErrorKind::DuplicateNativeFunctionName {
                name: desc.name.clone(),
            },
        ));
    }
    validate_native_function_params(desc)
}

fn validate_native_function_params(desc: &NativeFunctionDesc) -> EngineResult<()> {
    let mut names = BTreeSet::new();
    for param in &desc.params {
        if !names.insert(param.name.as_str()) {
            return Err(EngineError::new(
                EngineErrorKind::DuplicateNativeFunctionParamName {
                    function: desc.name.clone(),
                    name: param.name.clone(),
                },
            ));
        }
    }
    Ok(())
}
