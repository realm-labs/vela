use std::collections::BTreeMap;
use std::sync::Arc;

use vela_bytecode::compiler::options::CompilerOptions;
use vela_bytecode::{ProgramImage, UnlinkedProgram};
use vela_common::HostMethodId;
use vela_def::FunctionId;
use vela_host::path::HostPath;
use vela_hot_reload::abi::HotReloadAbi;
use vela_hot_reload::policy::HotReloadPolicy;
use vela_reflect::permissions::ReflectPolicy;
use vela_reflect::registry::TypeRegistry;
use vela_registry::{DefinitionRegistry, RegistryCompileView};
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::owned_value::OwnedValue;
use vela_vm::{HostExecution, Vm};

use crate::builder::EngineBuilder;
use crate::compiler_options::compiler_options_from_registry;
use crate::method::{NativeMethodDesc, NativeMethodEntry};
use crate::native::{
    ContextHostNativeFunctionEntry, HostNativeFunctionEntry, NativeFunctionDesc,
    NativeFunctionEntry,
};
use crate::permission::CapabilitySet;

#[derive(Clone)]
pub struct Engine {
    registry: Arc<TypeRegistry>,
    definition_registry: Arc<DefinitionRegistry>,
    native_functions: BTreeMap<FunctionId, NativeFunctionEntry>,
    host_native_functions: BTreeMap<FunctionId, HostNativeFunctionEntry>,
    context_host_native_functions: BTreeMap<FunctionId, ContextHostNativeFunctionEntry>,
    native_methods: BTreeMap<HostMethodId, NativeMethodEntry>,
    native_function_names: BTreeMap<String, FunctionId>,
    capabilities: CapabilitySet,
    reflection_policy: Option<ReflectPolicy>,
    hot_reload_policy: HotReloadPolicy,
    standard_natives: bool,
}

pub(crate) struct EngineParts {
    pub(crate) registry: TypeRegistry,
    pub(crate) definition_registry: DefinitionRegistry,
    pub(crate) native_functions: Vec<NativeFunctionEntry>,
    pub(crate) host_native_functions: Vec<HostNativeFunctionEntry>,
    pub(crate) context_host_native_functions: Vec<ContextHostNativeFunctionEntry>,
    pub(crate) native_methods: Vec<NativeMethodEntry>,
    pub(crate) capabilities: CapabilitySet,
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
            definition_registry: Arc::new(parts.definition_registry),
            native_functions,
            host_native_functions,
            context_host_native_functions,
            native_methods,
            native_function_names,
            capabilities: parts.capabilities,
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
    pub(crate) fn compiler_registry(&self) -> RegistryCompileView<'_> {
        self.definition_registry.compile_view()
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
    pub const fn capabilities(&self) -> CapabilitySet {
        self.capabilities
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
        args: &[OwnedValue],
        host: &mut HostExecution<'_>,
    ) -> VmResult<OwnedValue> {
        let entry = self.native_method(id).ok_or_else(|| {
            VmError::new(VmErrorKind::UnknownMethod {
                method: format!("host method {}", id.get()),
            })
        })?;
        check_capabilities(&entry.desc.name, &entry.desc.effects, self.capabilities)?;
        (entry.function)(receiver, args, host)
    }

    pub fn install(&self, vm: &mut Vm) {
        self.install_with_registry(vm, Arc::clone(&self.registry));
    }

    pub fn install_program(&self, vm: &mut Vm, program: &UnlinkedProgram) {
        self.install_with_registry(vm, self.registry_for_program(program));
    }

    pub fn install_program_image(&self, vm: &mut Vm, image: &ProgramImage) {
        self.install_with_registry(vm, self.registry_for_program_image(image));
    }

    fn install_with_registry(&self, vm: &mut Vm, registry: Arc<TypeRegistry>) {
        if self.standard_natives {
            vm.register_standard_natives();
        }
        self.install_native_functions(vm);
        self.install_host_native_functions(vm);
        self.install_context_host_native_functions(vm);
        if let Some(policy) = &self.reflection_policy {
            let policy = policy.clone();
            vm.register_reflection_natives_with_policy(registry, policy.clone());
        } else {
            vm.register_type_registry(registry);
        }
    }

    fn install_with_registry_and_abi(
        &self,
        vm: &mut Vm,
        registry: Arc<TypeRegistry>,
        abi: &HotReloadAbi,
    ) {
        self.install_with_registry(vm, registry);
        self.install_native_function_aliases(vm, abi);
    }

    fn install_native_functions(&self, vm: &mut Vm) {
        for entry in self.native_functions.values() {
            let id = entry.desc.id;
            let name = entry.desc.name.clone();
            let effects = entry.desc.effects;
            let capabilities = self.capabilities;
            let function = Arc::clone(&entry.function);
            vm.register_native_with_id(id, name.clone(), move |args| {
                check_capabilities(&name, &effects, capabilities)?;
                function(args)
            });
        }
    }

    fn install_host_native_functions(&self, vm: &mut Vm) {
        for entry in self.host_native_functions.values() {
            let id = entry.desc.id;
            let name = entry.desc.name.clone();
            let effects = entry.desc.effects;
            let capabilities = self.capabilities;
            let function = Arc::clone(&entry.function);
            vm.register_host_native_with_id(id, name.clone(), move |args, host| {
                check_capabilities(&name, &effects, capabilities)?;
                function(args, host)
            });
        }
    }

    fn install_context_host_native_functions(&self, vm: &mut Vm) {
        for entry in self.context_host_native_functions.values() {
            let id = entry.desc.id;
            let name = entry.desc.name.clone();
            let effects = entry.desc.effects;
            let capabilities = self.capabilities;
            let function = Arc::clone(&entry.function);
            let engine = self.clone();
            vm.register_budgeted_host_native_with_id(
                id,
                name.clone(),
                move |args, host, budget| {
                    check_capabilities(&name, &effects, capabilities)?;
                    let mut context = crate::context::NativeCallContext::new(&engine, host, budget);
                    function(args, &mut context)
                },
            );
        }
    }

    fn install_native_function_aliases(&self, vm: &mut Vm, abi: &HotReloadAbi) {
        for (id, alias) in abi.host_function_aliases() {
            if self.native_function_names.contains_key(alias) {
                continue;
            }
            let id = FunctionId::new(id);
            if let Some(entry) = self.native_functions.get(&id) {
                let alias = alias.to_owned();
                let effects = entry.desc.effects;
                let capabilities = self.capabilities;
                let function = Arc::clone(&entry.function);
                vm.register_native(alias.clone(), move |args| {
                    check_capabilities(&alias, &effects, capabilities)?;
                    function(args)
                });
            } else if let Some(entry) = self.host_native_functions.get(&id) {
                let alias = alias.to_owned();
                let effects = entry.desc.effects;
                let capabilities = self.capabilities;
                let function = Arc::clone(&entry.function);
                vm.register_host_native(alias.clone(), move |args, host| {
                    check_capabilities(&alias, &effects, capabilities)?;
                    function(args, host)
                });
            } else if let Some(entry) = self.context_host_native_functions.get(&id) {
                let alias = alias.to_owned();
                let effects = entry.desc.effects;
                let capabilities = self.capabilities;
                let function = Arc::clone(&entry.function);
                let engine = self.clone();
                vm.register_budgeted_host_native(alias.clone(), move |args, host, budget| {
                    check_capabilities(&alias, &effects, capabilities)?;
                    let mut context = crate::context::NativeCallContext::new(&engine, host, budget);
                    function(args, &mut context)
                });
            }
        }
    }

    fn registry_for_program(&self, program: &UnlinkedProgram) -> Arc<TypeRegistry> {
        let Some(graph) = program.script_metadata() else {
            return Arc::clone(&self.registry);
        };
        let mut registry = (*self.registry).clone();
        registry.register_script_types(graph);
        registry.register_script_modules(graph);
        Arc::new(registry)
    }

    fn registry_for_program_image(&self, image: &ProgramImage) -> Arc<TypeRegistry> {
        let Some(graph) = image.script_metadata() else {
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
    pub fn into_vm_for_program(&self, program: &UnlinkedProgram) -> Vm {
        let mut vm = Vm::new();
        self.install_program(&mut vm, program);
        vm
    }

    #[must_use]
    pub fn into_vm_for_program_image(&self, image: &ProgramImage) -> Vm {
        let mut vm = Vm::new();
        self.install_program_image(&mut vm, image);
        vm
    }

    #[must_use]
    pub fn into_vm_for_program_with_abi(
        &self,
        program: &UnlinkedProgram,
        abi: &HotReloadAbi,
    ) -> Vm {
        let mut vm = Vm::new();
        self.install_with_registry_and_abi(&mut vm, self.registry_for_program(program), abi);
        vm
    }

    #[must_use]
    pub fn into_vm_for_program_image_with_abi(
        &self,
        image: &ProgramImage,
        abi: &HotReloadAbi,
    ) -> Vm {
        let mut vm = Vm::new();
        self.install_with_registry_and_abi(&mut vm, self.registry_for_program_image(image), abi);
        vm
    }
}

fn check_capabilities(
    native: &str,
    effects: &crate::native::EffectSet,
    capabilities: CapabilitySet,
) -> VmResult<()> {
    let required = effects.required_capability_set();
    if capabilities.contains_all(required) {
        return Ok(());
    }

    if let Some(capability) = required.difference(capabilities).iter().next() {
        return Err(VmError::new(VmErrorKind::PermissionDenied {
            native: native.to_owned(),
            capability: capability.as_str().to_owned(),
        }));
    }
    Ok(())
}
