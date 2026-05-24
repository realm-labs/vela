use std::collections::BTreeMap;
use std::sync::Arc;

use vela_bytecode::compiler::CompilerOptions;
use vela_common::{FunctionId, HostMethodId};
use vela_host::HostPath;
use vela_reflect::TypeRegistry;
use vela_vm::{HostExecution, Value, Vm, VmError, VmErrorKind, VmResult};

use crate::{EngineBuilder, HostNativeFunctionEntry, NativeFunctionDesc, NativeFunctionEntry};
use crate::{FunctionAccess, NativeMethodDesc, NativeMethodEntry, PermissionSet};

#[derive(Clone)]
pub struct Engine {
    registry: Arc<TypeRegistry>,
    native_functions: BTreeMap<FunctionId, NativeFunctionEntry>,
    host_native_functions: BTreeMap<FunctionId, HostNativeFunctionEntry>,
    native_methods: BTreeMap<HostMethodId, NativeMethodEntry>,
    native_function_names: BTreeMap<String, FunctionId>,
    permissions: PermissionSet,
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
        native_methods: Vec<NativeMethodEntry>,
        permissions: PermissionSet,
    ) -> Self {
        let native_functions = native_functions
            .into_iter()
            .map(|entry| (entry.desc.id, entry))
            .collect::<BTreeMap<_, _>>();
        let host_native_functions = host_native_functions
            .into_iter()
            .map(|entry| (entry.desc.id, entry))
            .collect::<BTreeMap<_, _>>();
        let native_methods = native_methods
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
            native_methods,
            native_function_names,
            permissions,
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
    pub fn permissions(&self) -> &PermissionSet {
        &self.permissions
    }

    #[must_use]
    pub fn compiler_options(&self) -> CompilerOptions {
        let mut options = CompilerOptions::new();
        for desc in self.registry.types() {
            options = options.with_host_type(desc.key.name.clone());
            for method in &desc.methods {
                options = options.with_host_method_for_type(
                    desc.key.name.clone(),
                    method.name.clone(),
                    method.id,
                );
            }
        }
        options
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

    #[must_use]
    pub fn native_method(&self, id: HostMethodId) -> Option<&NativeMethodEntry> {
        self.native_methods.get(&id)
    }

    #[must_use]
    pub fn native_method_desc(&self, id: HostMethodId) -> Option<&NativeMethodDesc> {
        self.native_method(id).map(|entry| &entry.desc)
    }

    pub fn call_native_method(
        &self,
        id: HostMethodId,
        receiver: &HostPath,
        args: &[Value],
        host: &mut HostExecution<'_>,
    ) -> VmResult<Value> {
        let entry = self.native_method(id).ok_or_else(|| VmError {
            kind: VmErrorKind::UnknownMethod {
                method: format!("host method {}", id.get()),
            },
        })?;
        check_permissions(&entry.desc.name, &entry.desc.access, &self.permissions)?;
        (entry.function)(receiver, args, host)
    }

    pub fn install(&self, vm: &mut Vm) {
        vm.register_type_registry(Arc::clone(&self.registry));
        for entry in self.native_functions.values() {
            let name = entry.desc.name.clone();
            let access = entry.desc.access.clone();
            let permissions = self.permissions.clone();
            let function = Arc::clone(&entry.function);
            vm.register_native(name.clone(), move |args| {
                check_permissions(&name, &access, &permissions)?;
                function(args)
            });
        }
        for entry in self.host_native_functions.values() {
            let name = entry.desc.name.clone();
            let access = entry.desc.access.clone();
            let permissions = self.permissions.clone();
            let function = Arc::clone(&entry.function);
            vm.register_host_native(name.clone(), move |args, host| {
                check_permissions(&name, &access, &permissions)?;
                function(args, host)
            });
        }
    }

    #[must_use]
    pub fn into_vm(&self) -> Vm {
        let mut vm = Vm::new();
        self.install(&mut vm);
        vm
    }
}

fn check_permissions(
    native: &str,
    access: &FunctionAccess,
    permissions: &PermissionSet,
) -> VmResult<()> {
    if let Some(permission) = permissions.missing_required(&access.required_permissions) {
        return Err(VmError {
            kind: VmErrorKind::PermissionDenied {
                native: native.to_owned(),
                permission: permission.to_owned(),
            },
        });
    }
    Ok(())
}
