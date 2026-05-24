use std::collections::BTreeMap;
use std::sync::Arc;

use vela_common::FunctionId;
use vela_reflect::TypeRegistry;
use vela_vm::Vm;

use crate::{EngineBuilder, HostNativeFunctionEntry, NativeFunctionDesc, NativeFunctionEntry};

#[derive(Clone)]
pub struct Engine {
    registry: Arc<TypeRegistry>,
    native_functions: BTreeMap<FunctionId, NativeFunctionEntry>,
    host_native_functions: BTreeMap<FunctionId, HostNativeFunctionEntry>,
    native_function_names: BTreeMap<String, FunctionId>,
}

impl Engine {
    #[must_use]
    pub fn builder() -> EngineBuilder {
        EngineBuilder::new()
    }

    #[must_use]
    pub(crate) fn new(
        registry: TypeRegistry,
        native_functions: Vec<NativeFunctionEntry>,
        host_native_functions: Vec<HostNativeFunctionEntry>,
    ) -> Self {
        let native_functions = native_functions
            .into_iter()
            .map(|entry| (entry.desc.id, entry))
            .collect::<BTreeMap<_, _>>();
        let host_native_functions = host_native_functions
            .into_iter()
            .map(|entry| (entry.desc.id, entry))
            .collect::<BTreeMap<_, _>>();
        let native_function_names = native_functions
            .values()
            .map(|entry| &entry.desc)
            .chain(host_native_functions.values().map(|entry| &entry.desc))
            .map(|desc| (desc.name.clone(), desc.id))
            .collect();

        Self {
            registry: Arc::new(registry),
            native_functions,
            host_native_functions,
            native_function_names,
        }
    }

    #[must_use]
    pub fn registry(&self) -> Arc<TypeRegistry> {
        Arc::clone(&self.registry)
    }

    #[must_use]
    pub fn native_function(&self, id: FunctionId) -> Option<&NativeFunctionEntry> {
        self.native_functions.get(&id)
    }

    #[must_use]
    pub fn native_function_desc(&self, id: FunctionId) -> Option<&NativeFunctionDesc> {
        self.native_function(id)
            .map(|entry| &entry.desc)
            .or_else(|| self.host_native_function(id).map(|entry| &entry.desc))
    }

    #[must_use]
    pub fn native_function_by_name(&self, name: &str) -> Option<&NativeFunctionEntry> {
        let id = self.native_function_names.get(name)?;
        self.native_function(*id)
    }

    #[must_use]
    pub fn host_native_function(&self, id: FunctionId) -> Option<&HostNativeFunctionEntry> {
        self.host_native_functions.get(&id)
    }

    #[must_use]
    pub fn host_native_function_by_name(&self, name: &str) -> Option<&HostNativeFunctionEntry> {
        let id = self.native_function_names.get(name)?;
        self.host_native_function(*id)
    }

    pub fn install(&self, vm: &mut Vm) {
        vm.register_type_registry(Arc::clone(&self.registry));
        for entry in self.native_functions.values() {
            let name = entry.desc.name.clone();
            let function = Arc::clone(&entry.function);
            vm.register_native(name, move |args| function(args));
        }
        for entry in self.host_native_functions.values() {
            let name = entry.desc.name.clone();
            let function = Arc::clone(&entry.function);
            vm.register_host_native(name, move |args, host| function(args, host));
        }
    }

    #[must_use]
    pub fn into_vm(&self) -> Vm {
        let mut vm = Vm::new();
        self.install(&mut vm);
        vm
    }
}
