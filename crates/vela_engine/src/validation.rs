use std::collections::BTreeSet;

use vela_reflect::TypeDesc;

use crate::{
    ContextHostNativeFunctionEntry, EngineError, EngineErrorKind, EngineResult,
    HostNativeFunctionEntry, NativeFunctionEntry,
};

pub(crate) fn validate_types(types: &[TypeDesc]) -> EngineResult<()> {
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();
    let mut host_ids = BTreeSet::new();
    let mut host_method_ids = BTreeSet::new();
    let mut host_method_names = BTreeSet::new();

    for desc in types {
        if !ids.insert(desc.key.id) {
            return Err(EngineError::new(EngineErrorKind::DuplicateTypeId {
                id: desc.key.id.get(),
            }));
        }
        if !names.insert(desc.key.name.as_str()) {
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
            if !host_method_names.insert((desc.key.name.as_str(), method.name.as_str())) {
                return Err(EngineError::new(EngineErrorKind::DuplicateHostMethodName {
                    name: method.name.clone(),
                }));
            }
        }
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
    variant: &vela_reflect::VariantDesc,
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
    trait_desc: &vela_reflect::TraitDesc,
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
    }
    Ok(())
}

pub(crate) fn validate_native_functions(
    functions: &[NativeFunctionEntry],
    host_functions: &[HostNativeFunctionEntry],
    context_host_functions: &[ContextHostNativeFunctionEntry],
) -> EngineResult<()> {
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();

    for desc in functions
        .iter()
        .map(|entry| &entry.desc)
        .chain(host_functions.iter().map(|entry| &entry.desc))
        .chain(context_host_functions.iter().map(|entry| &entry.desc))
    {
        if !ids.insert(desc.id) {
            return Err(EngineError::new(
                EngineErrorKind::DuplicateNativeFunctionId { id: desc.id.get() },
            ));
        }
        if !names.insert(desc.name.as_str()) {
            return Err(EngineError::new(
                EngineErrorKind::DuplicateNativeFunctionName {
                    name: desc.name.clone(),
                },
            ));
        }
    }

    Ok(())
}
