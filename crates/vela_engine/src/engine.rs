use std::collections::BTreeMap;
use std::sync::Arc;

use vela_bytecode::Program;
use vela_bytecode::compiler::options::CompilerOptions;
use vela_common::{FunctionId, HostMethodId};
use vela_host::path::HostPath;
use vela_hot_reload::policy::HotReloadPolicy;
use vela_reflect::permissions::ReflectPolicy;
use vela_reflect::registry::TypeRegistry;
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::value::Value;
use vela_vm::{HostExecution, Vm};

use crate::builder::EngineBuilder;
use crate::compiler_options::compiler_options_from_registry;
use crate::method::{NativeMethodDesc, NativeMethodEntry};
use crate::native::{
    ContextHostNativeFunctionEntry, FunctionAccess, HostNativeFunctionEntry, NativeFunctionDesc,
    NativeFunctionEntry,
};
use crate::permission::PermissionSet;

#[derive(Clone)]
pub struct Engine {
    registry: Arc<TypeRegistry>,
    native_functions: BTreeMap<FunctionId, NativeFunctionEntry>,
    host_native_functions: BTreeMap<FunctionId, HostNativeFunctionEntry>,
    context_host_native_functions: BTreeMap<FunctionId, ContextHostNativeFunctionEntry>,
    native_methods: BTreeMap<HostMethodId, NativeMethodEntry>,
    native_function_names: BTreeMap<String, FunctionId>,
    permissions: PermissionSet,
    reflection_policy: Option<ReflectPolicy>,
    hot_reload_policy: HotReloadPolicy,
    standard_natives: bool,
}

pub(crate) struct EngineParts {
    pub(crate) registry: TypeRegistry,
    pub(crate) native_functions: Vec<NativeFunctionEntry>,
    pub(crate) host_native_functions: Vec<HostNativeFunctionEntry>,
    pub(crate) context_host_native_functions: Vec<ContextHostNativeFunctionEntry>,
    pub(crate) native_methods: Vec<NativeMethodEntry>,
    pub(crate) permissions: PermissionSet,
    pub(crate) reflection_policy: Option<ReflectPolicy>,
    pub(crate) hot_reload_policy: HotReloadPolicy,
    pub(crate) standard_natives: bool,
}

impl Engine {
    #[must_use]
    pub fn builder() -> EngineBuilder {
        EngineBuilder::new()
    }

    #[must_use]
    pub(crate) fn new(parts: EngineParts) -> Self {
        let native_functions = parts
            .native_functions
            .into_iter()
            .map(|entry| (entry.desc.id, entry))
            .collect::<BTreeMap<_, _>>();
        let host_native_functions = parts
            .host_native_functions
            .into_iter()
            .map(|entry| (entry.desc.id, entry))
            .collect::<BTreeMap<_, _>>();
        let context_host_native_functions = parts
            .context_host_native_functions
            .into_iter()
            .map(|entry| (entry.desc.id, entry))
            .collect::<BTreeMap<_, _>>();
        let native_methods = parts
            .native_methods
            .into_iter()
            .map(|entry| (entry.desc.id, entry))
            .collect::<BTreeMap<_, _>>();
        let native_function_names = native_functions
            .values()
            .map(|entry| &entry.desc)
            .chain(host_native_functions.values().map(|entry| &entry.desc))
            .chain(
                context_host_native_functions
                    .values()
                    .map(|entry| &entry.desc),
            )
            .map(|desc| (desc.name.clone(), desc.id))
            .collect();

        Self {
            registry: Arc::new(parts.registry),
            native_functions,
            host_native_functions,
            context_host_native_functions,
            native_methods,
            native_function_names,
            permissions: parts.permissions,
            reflection_policy: parts.reflection_policy,
            hot_reload_policy: parts.hot_reload_policy,
            standard_natives: parts.standard_natives,
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
            .or_else(|| {
                self.context_host_native_function(id)
                    .map(|entry| &entry.desc)
            })
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
    pub fn hot_reload_policy(&self) -> &HotReloadPolicy {
        &self.hot_reload_policy
    }

    #[must_use]
    pub fn compiler_options(&self) -> CompilerOptions {
        let mut options = compiler_options_from_registry(&self.registry);
        if self.reflection_policy.is_some() {
            options = options.with_native_module_root("reflect");
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
    pub fn context_host_native_function(
        &self,
        id: FunctionId,
    ) -> Option<&ContextHostNativeFunctionEntry> {
        self.context_host_native_functions.get(&id)
    }

    #[must_use]
    pub fn context_host_native_function_by_name(
        &self,
        name: &str,
    ) -> Option<&ContextHostNativeFunctionEntry> {
        let id = self.native_function_names.get(name)?;
        self.context_host_native_function(*id)
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
            source_span: None,
            call_stack: Default::default(),
        })?;
        check_permissions(&entry.desc.name, &entry.desc.access, &self.permissions)?;
        let tx_checkpoint = host.tx.clone();
        match (entry.function)(receiver, args, host) {
            Ok(value) => Ok(value),
            Err(error) => {
                *host.tx = tx_checkpoint;
                Err(error)
            }
        }
    }

    pub fn install(&self, vm: &mut Vm) {
        self.install_with_registry(vm, Arc::clone(&self.registry));
    }

    pub fn install_program(&self, vm: &mut Vm, program: &Program) {
        self.install_with_registry(vm, self.registry_for_program(program));
    }

    fn install_with_registry(&self, vm: &mut Vm, registry: Arc<TypeRegistry>) {
        if self.standard_natives {
            vm.register_standard_natives();
        }
        self.install_native_functions(vm);
        self.install_host_native_functions(vm);
        self.install_context_host_native_functions(vm);
        if let Some(policy) = &self.reflection_policy {
            let policy = policy
                .clone()
                .with_field_permissions(self.permissions.iter())
                .with_method_permissions(self.permissions.iter());
            let policy = policy.with_function_permissions(self.permissions.iter());
            vm.register_reflection_natives_with_policy(registry, policy.clone());
        } else {
            vm.register_type_registry(registry);
        }
    }

    fn install_native_functions(&self, vm: &mut Vm) {
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
    }

    fn install_host_native_functions(&self, vm: &mut Vm) {
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

    fn install_context_host_native_functions(&self, vm: &mut Vm) {
        for entry in self.context_host_native_functions.values() {
            let name = entry.desc.name.clone();
            let access = entry.desc.access.clone();
            let permissions = self.permissions.clone();
            let function = Arc::clone(&entry.function);
            let engine = self.clone();
            vm.register_budgeted_host_native(name.clone(), move |args, host, budget| {
                check_permissions(&name, &access, &permissions)?;
                let mut context = crate::context::NativeCallContext::new(&engine, host, budget);
                function(args, &mut context)
            });
        }
    }

    fn registry_for_program(&self, program: &Program) -> Arc<TypeRegistry> {
        let Some(graph) = program.script_metadata() else {
            return Arc::clone(&self.registry);
        };
        let mut registry = (*self.registry).clone();
        registry.register_script_types(graph);
        registry.register_script_modules(graph);
        Arc::new(registry)
    }

    #[must_use]
    pub fn into_vm(&self) -> Vm {
        let mut vm = Vm::new();
        self.install(&mut vm);
        vm
    }

    #[must_use]
    pub fn into_vm_for_program(&self, program: &Program) -> Vm {
        let mut vm = Vm::new();
        self.install_program(&mut vm, program);
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
            source_span: None,
            call_stack: Default::default(),
        });
    }
    Ok(())
}
