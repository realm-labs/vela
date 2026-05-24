use std::collections::BTreeSet;

use vela_reflect::{TypeDesc, TypeRegistry};
use vela_vm::{Value, VmResult};

use crate::{
    Engine, EngineError, EngineErrorKind, EngineResult, NativeFunctionDesc, NativeFunctionEntry,
};

#[derive(Clone, Default)]
pub struct EngineBuilder {
    types: Vec<TypeDesc>,
    native_functions: Vec<NativeFunctionEntry>,
}

impl EngineBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn register_type(mut self, desc: TypeDesc) -> Self {
        self.types.push(desc);
        self
    }

    #[must_use]
    pub fn register_native_fn(
        mut self,
        desc: NativeFunctionDesc,
        function: impl Fn(&[Value]) -> VmResult<Value> + Send + Sync + 'static,
    ) -> Self {
        self.native_functions
            .push(NativeFunctionEntry::new(desc, function));
        self
    }

    pub fn build(self) -> EngineResult<Engine> {
        validate_types(&self.types)?;
        validate_native_functions(&self.native_functions)?;

        let mut registry = TypeRegistry::new();
        for desc in self.types {
            registry.register(desc);
        }

        Ok(Engine::new(registry, self.native_functions))
    }
}

fn validate_types(types: &[TypeDesc]) -> EngineResult<()> {
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();
    let mut host_ids = BTreeSet::new();

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
    }

    Ok(())
}

fn validate_native_functions(functions: &[NativeFunctionEntry]) -> EngineResult<()> {
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();

    for entry in functions {
        if !ids.insert(entry.desc.id) {
            return Err(EngineError::new(
                EngineErrorKind::DuplicateNativeFunctionId {
                    id: entry.desc.id.get(),
                },
            ));
        }
        if !names.insert(entry.desc.name.as_str()) {
            return Err(EngineError::new(
                EngineErrorKind::DuplicateNativeFunctionName {
                    name: entry.desc.name.clone(),
                },
            ));
        }
    }

    Ok(())
}
