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
