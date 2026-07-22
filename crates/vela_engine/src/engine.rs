use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use vela_bytecode::compiler::options::CompilerOptions;
use vela_bytecode::{LinkError, LinkedArtifact, Linker, ProgramImage, UnlinkedProgram};
use vela_common::{HostMethodId, ReceiverCapability};
use vela_def::FunctionId;
use vela_host::error::HostErrorKind;
use vela_host::lease::HostLeaseKind;
use vela_host::path::HostPath;
use vela_hot_reload::abi::HotReloadAbi;
use vela_hot_reload::policy::HotReloadPolicy;
use vela_reflect::permissions::ReflectPolicy;
use vela_reflect::registry::TypeRegistry;
use vela_registry::{DefinitionRegistry, RegistryCompileView};
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::owned_value::OwnedValue;
use vela_vm::{ConditionalAsyncNativeFunction, ConditionalHostNativeOutcome, HostExecution, Vm};

use crate::builder::EngineBuilder;
use crate::compiler_options::{add_native_signature_hints, compiler_options_from_registry};
use crate::method::{
    AsyncNativeMethodEntry, AsyncNativeMethodImplementation, NativeMethodDesc, NativeMethodEntry,
};
use crate::native::{
    AsyncContextHostNativeFunctionEntry, AsyncDirectHostNativeFunctionEntry,
    AsyncHostNativeFunctionEntry, AsyncNativeFunctionEntry, ContextHostNativeFunctionEntry,
    HostNativeFunctionEntry, NativeFunctionDesc, NativeFunctionEntry,
    ScopedHostNativeFunctionEntry, ScopedHostNativeOutcome,
};
use crate::permission::CapabilitySet;
use crate::type_binding::TypeBindingRegistry;

#[derive(Clone)]
pub struct Engine {
    registry: Arc<TypeRegistry>,
    type_bindings: Arc<TypeBindingRegistry>,
    definition_registry: Arc<DefinitionRegistry>,
    native_functions: BTreeMap<FunctionId, NativeFunctionEntry>,
    async_native_functions: BTreeMap<FunctionId, AsyncNativeFunctionEntry>,
    async_host_native_functions: BTreeMap<FunctionId, AsyncHostNativeFunctionEntry>,
    async_direct_host_native_functions: BTreeMap<FunctionId, AsyncDirectHostNativeFunctionEntry>,
    scoped_host_native_functions: BTreeMap<FunctionId, ScopedHostNativeFunctionEntry>,
    async_context_host_native_functions: BTreeMap<FunctionId, AsyncContextHostNativeFunctionEntry>,
    host_native_functions: BTreeMap<FunctionId, HostNativeFunctionEntry>,
    context_host_native_functions: BTreeMap<FunctionId, ContextHostNativeFunctionEntry>,
    native_methods: BTreeMap<HostMethodId, NativeMethodEntry>,
    async_native_methods: BTreeMap<HostMethodId, AsyncNativeMethodEntry>,
    native_function_names: BTreeMap<String, FunctionId>,
    capabilities: CapabilitySet,
    reflection_policy: Option<ReflectPolicy>,
    hot_reload_policy: HotReloadPolicy,
    standard_natives: bool,
    execution_data: crate::runtime::execution_data::SharedGenerationExecutionRegistry,
}

pub(crate) struct EngineParts {
    pub(crate) registry: TypeRegistry,
    pub(crate) type_bindings: TypeBindingRegistry,
    pub(crate) definition_registry: DefinitionRegistry,
    pub(crate) native_functions: Vec<NativeFunctionEntry>,
    pub(crate) async_native_functions: Vec<AsyncNativeFunctionEntry>,
    pub(crate) async_host_native_functions: Vec<AsyncHostNativeFunctionEntry>,
    pub(crate) async_direct_host_native_functions: Vec<AsyncDirectHostNativeFunctionEntry>,
    pub(crate) scoped_host_native_functions: Vec<ScopedHostNativeFunctionEntry>,
    pub(crate) async_context_host_native_functions: Vec<AsyncContextHostNativeFunctionEntry>,
    pub(crate) host_native_functions: Vec<HostNativeFunctionEntry>,
    pub(crate) context_host_native_functions: Vec<ContextHostNativeFunctionEntry>,
    pub(crate) native_methods: Vec<NativeMethodEntry>,
    pub(crate) async_native_methods: Vec<AsyncNativeMethodEntry>,
    pub(crate) capabilities: CapabilitySet,
    pub(crate) reflection_policy: Option<ReflectPolicy>,
    pub(crate) hot_reload_policy: HotReloadPolicy,
    pub(crate) standard_natives: bool,
}

impl Engine {
    #[cfg(test)]
    pub(crate) fn link_test_program(
        &self,
        program: &UnlinkedProgram,
    ) -> Result<Arc<LinkedArtifact>, LinkError> {
        let mut linker = Linker::with_registry(&self.definition_registry);
        for id in self.native_implementation_ids() {
            linker.add_native_implementation(id);
        }
        linker.link_test_program(program)
    }

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
        let async_native_functions = parts
            .async_native_functions
            .into_iter()
            .map(|entry| (entry.desc.id, entry))
            .collect::<BTreeMap<_, _>>();
        let async_host_native_functions = parts
            .async_host_native_functions
            .into_iter()
            .map(|entry| (entry.desc.id, entry))
            .collect::<BTreeMap<_, _>>();
        let async_direct_host_native_functions = parts
            .async_direct_host_native_functions
            .into_iter()
            .map(|entry| (entry.desc.id, entry))
            .collect::<BTreeMap<_, _>>();
        let scoped_host_native_functions = parts
            .scoped_host_native_functions
            .into_iter()
            .map(|entry| (entry.desc.id, entry))
            .collect::<BTreeMap<_, _>>();
        let async_context_host_native_functions = parts
            .async_context_host_native_functions
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
        let async_native_methods = parts
            .async_native_methods
            .into_iter()
            .map(|entry| (entry.desc.id, entry))
            .collect::<BTreeMap<_, _>>();
        let native_function_names = native_functions
            .values()
            .map(|entry| &entry.desc)
            .chain(async_native_functions.values().map(|entry| &entry.desc))
            .chain(
                async_host_native_functions
                    .values()
                    .map(|entry| &entry.desc),
            )
            .chain(
                async_direct_host_native_functions
                    .values()
                    .map(|entry| &entry.desc),
            )
            .chain(
                scoped_host_native_functions
                    .values()
                    .map(|entry| &entry.desc),
            )
            .chain(
                async_context_host_native_functions
                    .values()
                    .map(|entry| &entry.desc),
            )
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
            type_bindings: Arc::new(parts.type_bindings),
            definition_registry: Arc::new(parts.definition_registry),
            native_functions,
            async_native_functions,
            async_host_native_functions,
            async_direct_host_native_functions,
            scoped_host_native_functions,
            async_context_host_native_functions,
            host_native_functions,
            context_host_native_functions,
            native_methods,
            async_native_methods,
            native_function_names,
            capabilities: parts.capabilities,
            reflection_policy: parts.reflection_policy,
            hot_reload_policy: parts.hot_reload_policy,
            standard_natives: parts.standard_natives,
            execution_data: Arc::new(Mutex::new(
                crate::runtime::execution_data::GenerationExecutionRegistry::new(),
            )),
        }
    }

    pub(crate) fn generation_execution_data(
        &self,
        artifact: &Arc<LinkedArtifact>,
    ) -> crate::runtime::execution_data::SharedGenerationExecutionData {
        let mut registry = self
            .execution_data
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        registry.data_for(artifact)
    }

    pub(crate) fn enable_bytecode_profile(&self) {
        self.execution_data
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .enable_bytecode_profile();
    }

    #[must_use]
    pub fn registry(&self) -> Arc<TypeRegistry> {
        Arc::clone(&self.registry)
    }

    #[must_use]
    pub fn type_bindings(&self) -> Arc<TypeBindingRegistry> {
        Arc::clone(&self.type_bindings)
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
    pub fn async_native_function(&self, id: FunctionId) -> Option<&AsyncNativeFunctionEntry> {
        self.async_native_functions.get(&id)
    }

    #[must_use]
    pub fn async_host_native_function(
        &self,
        id: FunctionId,
    ) -> Option<&AsyncHostNativeFunctionEntry> {
        self.async_host_native_functions.get(&id)
    }

    #[must_use]
    pub fn async_direct_host_native_function(
        &self,
        id: FunctionId,
    ) -> Option<&AsyncDirectHostNativeFunctionEntry> {
        self.async_direct_host_native_functions.get(&id)
    }

    #[must_use]
    pub fn scoped_host_native_function(
        &self,
        id: FunctionId,
    ) -> Option<&ScopedHostNativeFunctionEntry> {
        self.scoped_host_native_functions.get(&id)
    }

    #[must_use]
    pub fn async_context_host_native_function(
        &self,
        id: FunctionId,
    ) -> Option<&AsyncContextHostNativeFunctionEntry> {
        self.async_context_host_native_functions.get(&id)
    }

    #[must_use]
    pub fn native_function_desc(&self, id: FunctionId) -> Option<&NativeFunctionDesc> {
        self.native_function(id)
            .map(|entry| &entry.desc)
            .or_else(|| self.async_native_function(id).map(|entry| &entry.desc))
            .or_else(|| self.async_host_native_function(id).map(|entry| &entry.desc))
            .or_else(|| {
                self.async_direct_host_native_function(id)
                    .map(|entry| &entry.desc)
            })
            .or_else(|| {
                self.scoped_host_native_function(id)
                    .map(|entry| &entry.desc)
            })
            .or_else(|| {
                self.async_context_host_native_function(id)
                    .map(|entry| &entry.desc)
            })
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
    pub fn async_native_function_by_name(&self, name: &str) -> Option<&AsyncNativeFunctionEntry> {
        let id = self.native_function_names.get(name)?;
        self.async_native_function(*id)
    }

    #[must_use]
    pub fn async_host_native_function_by_name(
        &self,
        name: &str,
    ) -> Option<&AsyncHostNativeFunctionEntry> {
        let id = self.native_function_names.get(name)?;
        self.async_host_native_function(*id)
    }

    #[must_use]
    pub fn async_context_host_native_function_by_name(
        &self,
        name: &str,
    ) -> Option<&AsyncContextHostNativeFunctionEntry> {
        let id = self.native_function_names.get(name)?;
        self.async_context_host_native_function(*id)
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
        for desc in self
            .native_functions
            .values()
            .map(|entry| &entry.desc)
            .chain(
                self.async_native_functions
                    .values()
                    .map(|entry| &entry.desc),
            )
            .chain(
                self.async_host_native_functions
                    .values()
                    .map(|entry| &entry.desc),
            )
            .chain(
                self.async_direct_host_native_functions
                    .values()
                    .map(|entry| &entry.desc),
            )
            .chain(
                self.scoped_host_native_functions
                    .values()
                    .map(|entry| &entry.desc),
            )
            .chain(
                self.async_context_host_native_functions
                    .values()
                    .map(|entry| &entry.desc),
            )
            .chain(self.host_native_functions.values().map(|entry| &entry.desc))
            .chain(
                self.context_host_native_functions
                    .values()
                    .map(|entry| &entry.desc),
            )
        {
            options = add_native_signature_hints(options, desc);
            if desc.callable_contract.as_ref().is_some_and(|contract| {
                matches!(
                    contract.returns.mode,
                    crate::interop::ReturnMode::ScopedHost { .. }
                ) && matches!(contract.returns.ty, crate::native::TypeHint::Host(_))
            }) {
                options = options.with_scoped_borrow_function(desc.id);
            }
        }
        for entry in self.native_methods.values() {
            if entry
                .desc
                .callable_contract
                .as_ref()
                .is_some_and(|contract| {
                    matches!(
                        contract.returns.mode,
                        crate::interop::ReturnMode::ScopedHost { .. }
                    ) && matches!(contract.returns.ty, crate::native::TypeHint::Host(_))
                })
            {
                options = options.with_scoped_borrow_method(entry.desc.id);
            }
        }
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
        self.native_method(id)
            .map(|entry| &entry.desc)
            .or_else(|| self.async_native_method(id).map(|entry| &entry.desc))
    }

    #[must_use]
    pub fn async_native_method(&self, id: HostMethodId) -> Option<&AsyncNativeMethodEntry> {
        self.async_native_methods.get(&id)
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
        check_method_receiver(entry.desc.receiver, receiver, host)?;
        (entry.function)(receiver, args, host)
    }

    pub fn link_compiled_program(
        &self,
        program: vela_bytecode::compiler::CompiledProgram,
    ) -> Result<Arc<LinkedArtifact>, LinkError> {
        let mut linker = Linker::with_registry(&self.definition_registry);
        for id in self.native_implementation_ids() {
            linker.add_native_implementation(id);
        }
        linker.link_compiled_program(program)
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
        self.install_async_native_functions(vm);
        self.install_async_host_native_functions(vm);
        self.install_async_direct_host_native_functions(vm);
        self.install_scoped_host_native_functions(vm);
        self.install_async_context_host_native_functions(vm);
        self.install_native_methods(vm);
        self.install_async_native_methods(vm);
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
            vm.register_native_with_id(id, {
                let function = Arc::clone(&function);
                move |args| {
                    check_capabilities(&name, &effects, capabilities)?;
                    function(args)
                }
            });
        }
    }

    fn install_async_native_functions(&self, vm: &mut Vm) {
        for entry in self.async_native_functions.values() {
            let id = entry.desc.id;
            let name = entry.desc.name.clone();
            let effects = entry.desc.effects;
            let capabilities = self.capabilities;
            let function = Arc::clone(&entry.function);
            vm.register_async_native_with_id(id, move |args| {
                if let Err(error) = check_capabilities(&name, &effects, capabilities) {
                    return Box::pin(async move { Err(error) });
                }
                function(args)
            });
        }
    }

    fn install_async_host_native_functions(&self, vm: &mut Vm) {
        for entry in self.async_host_native_functions.values() {
            let id = entry.desc.id;
            let name = entry.desc.name.clone();
            let effects = entry.desc.effects;
            let capabilities = self.capabilities;
            let function = Arc::clone(&entry.function);
            vm.register_async_host_native_with_id(id, move |args, host, _budget| {
                if let Err(error) = check_capabilities(&name, &effects, capabilities) {
                    return Box::pin(async move { Err(error) });
                }
                function(args, host)
            });
        }
    }

    fn install_async_direct_host_native_functions(&self, vm: &mut Vm) {
        for entry in self.async_direct_host_native_functions.values() {
            let id = entry.desc.id;
            let name = entry.desc.name.clone();
            let effects = entry.desc.effects;
            let capabilities = self.capabilities;
            let requests = Arc::clone(&entry.requests);
            let function = Arc::clone(&entry.function);
            vm.register_conditional_host_native_with_id(id, move |args, _host, _budget| {
                check_capabilities(&name, &effects, capabilities)?;
                Ok(ConditionalHostNativeOutcome::Async {
                    function: ConditionalAsyncNativeFunction::DirectHostFunction {
                        function: Arc::clone(&function),
                        requests: requests(args)?,
                    },
                    args: args.to_vec(),
                    diagnostic_name: name.clone(),
                })
            });
        }
    }

    fn install_scoped_host_native_functions(&self, vm: &mut Vm) {
        for entry in self.scoped_host_native_functions.values() {
            let id = entry.desc.id;
            let name = entry.desc.name.clone();
            let effects = entry.desc.effects;
            let capabilities = self.capabilities;
            let requests = Arc::clone(&entry.requests);
            let function = Arc::clone(&entry.function);
            vm.register_host_native_with_id(id, move |args, host| {
                check_capabilities(&name, &effects, capabilities)?;
                let requests = requests(args)?;
                let mut invocation_result = None;
                let mut envelope = ScopedHostEnvelope::Direct;
                let retained =
                    host.adapter.with_scoped_host_return(
                        &requests,
                        &mut |leases| match function(leases, args.to_vec()) {
                            Ok(ScopedHostNativeOutcome::Direct(returned)) => Ok(Some(
                                vela_host::adapter::ScopedHostReturns::Single(returned),
                            )),
                            Ok(ScopedHostNativeOutcome::OptionSome(returned)) => {
                                envelope = ScopedHostEnvelope::OptionSome;
                                Ok(Some(vela_host::adapter::ScopedHostReturns::Single(
                                    returned,
                                )))
                            }
                            Ok(ScopedHostNativeOutcome::ResultOk(returned)) => {
                                envelope = ScopedHostEnvelope::ResultOk;
                                Ok(Some(vela_host::adapter::ScopedHostReturns::Single(
                                    returned,
                                )))
                            }
                            Ok(ScopedHostNativeOutcome::Tuple(returned)) => {
                                envelope = ScopedHostEnvelope::Tuple;
                                Ok(Some(vela_host::adapter::ScopedHostReturns::Group(returned)))
                            }
                            Ok(ScopedHostNativeOutcome::OptionSomeTuple(returned)) => {
                                envelope = ScopedHostEnvelope::OptionSomeTuple;
                                Ok(Some(vela_host::adapter::ScopedHostReturns::Group(returned)))
                            }
                            Ok(ScopedHostNativeOutcome::ResultOkTuple(returned)) => {
                                envelope = ScopedHostEnvelope::ResultOkTuple;
                                Ok(Some(vela_host::adapter::ScopedHostReturns::Group(returned)))
                            }
                            Ok(ScopedHostNativeOutcome::Value(value)) => {
                                invocation_result = Some(Ok(value));
                                Ok(None)
                            }
                            Err(error) => {
                                invocation_result = Some(Err(error));
                                Ok(None)
                            }
                        },
                    )?;
                match retained {
                    Some(roots) => Ok(envelope.wrap(roots)),
                    None => invocation_result.expect("missing scoped host invocation result"),
                }
            });
        }
    }

    fn install_async_context_host_native_functions(&self, vm: &mut Vm) {
        for entry in self.async_context_host_native_functions.values() {
            let id = entry.desc.id;
            let name = entry.desc.name.clone();
            let effects = entry.desc.effects;
            let capabilities = self.capabilities;
            let function = Arc::clone(&entry.function);
            let engine = self.clone();
            vm.register_async_host_native_with_id(id, move |args, host, budget| {
                let capability_error = check_capabilities(&name, &effects, capabilities).err();
                let function = Arc::clone(&function);
                let engine = engine.clone();
                Box::pin(async move {
                    if let Some(error) = capability_error {
                        return Err(error);
                    }
                    let mut context = crate::context::NativeCallContext::new(
                        &engine,
                        host,
                        budget,
                        None,
                        effects.required_capability_set(),
                    );
                    function(args, &mut context).await
                })
            });
        }
    }

    fn install_async_native_methods(&self, vm: &mut Vm) {
        for entry in self.async_native_methods.values() {
            let id = entry.desc.id;
            let name = entry.desc.name.clone();
            let effects = entry.desc.effects;
            let capabilities = self.capabilities;
            match &entry.function {
                AsyncNativeMethodImplementation::HostPath(function) => {
                    let function = Arc::clone(function);
                    let receiver_capability = entry.desc.receiver;
                    vm.register_async_host_method_with_id(
                        id,
                        move |receiver, args, host, _budget| {
                            if let Err(error) = check_capabilities(&name, &effects, capabilities) {
                                return Box::pin(async move { Err(error) });
                            }
                            if let Err(error) =
                                check_method_receiver(receiver_capability, receiver, host)
                            {
                                return Box::pin(async move { Err(error) });
                            }
                            function(receiver, args, host)
                        },
                    );
                }
                AsyncNativeMethodImplementation::Direct {
                    lease_kind,
                    function,
                } => {
                    let lease_kind = *lease_kind;
                    let function = Arc::clone(function);
                    vm.register_async_direct_host_method_with_id(
                        id,
                        lease_kind,
                        move |root, lease, args| {
                            if let Err(error) = check_capabilities(&name, &effects, capabilities) {
                                return Box::pin(async move { Err(error) });
                            }
                            function(root, lease, args)
                        },
                    );
                }
                AsyncNativeMethodImplementation::DirectContext { lease_kind, .. } => {
                    let lease_kind = *lease_kind;
                    vm.register_async_direct_host_method_with_id(
                        id,
                        lease_kind,
                        move |_root, _lease, _args| {
                            Box::pin(async {
                                Err(VmError::new(VmErrorKind::TypeMismatch {
                                    operation: "context direct method outside Runtime execution",
                                }))
                            })
                        },
                    );
                }
            }
        }
    }

    fn install_native_methods(&self, vm: &mut Vm) {
        for entry in self.native_methods.values() {
            let id = entry.desc.id;
            let name = entry.desc.name.clone();
            let effects = entry.desc.effects;
            let capabilities = self.capabilities;
            let receiver_capability = entry.desc.receiver;
            let function = Arc::clone(&entry.function);
            vm.register_host_method_with_id(id, move |receiver, args, host| {
                check_capabilities(&name, &effects, capabilities)?;
                check_method_receiver(receiver_capability, receiver, host)?;
                function(receiver, args, host)
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
            vm.register_host_native_with_id(id, {
                let function = Arc::clone(&function);
                move |args, host| {
                    check_capabilities(&name, &effects, capabilities)?;
                    function(args, host)
                }
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
            vm.register_context_host_native_with_id(id, move |args, host, budget| {
                check_capabilities(&name, &effects, capabilities)?;
                let mut context = crate::context::NativeCallContext::new(
                    &engine,
                    host,
                    budget,
                    None,
                    effects.required_capability_set(),
                );
                function(args, &mut context)
            });
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
                vm.register_native_with_id(id, move |args| {
                    check_capabilities(&alias, &effects, capabilities)?;
                    function(args)
                });
            } else if let Some(entry) = self.async_native_functions.get(&id) {
                let alias = alias.to_owned();
                let effects = entry.desc.effects;
                let capabilities = self.capabilities;
                let function = Arc::clone(&entry.function);
                vm.register_async_native_with_id(id, move |args| {
                    if let Err(error) = check_capabilities(&alias, &effects, capabilities) {
                        return Box::pin(async move { Err(error) });
                    }
                    function(args)
                });
            } else if let Some(entry) = self.async_host_native_functions.get(&id) {
                let alias = alias.to_owned();
                let effects = entry.desc.effects;
                let capabilities = self.capabilities;
                let function = Arc::clone(&entry.function);
                vm.register_async_host_native_with_id(id, move |args, host, _budget| {
                    if let Err(error) = check_capabilities(&alias, &effects, capabilities) {
                        return Box::pin(async move { Err(error) });
                    }
                    function(args, host)
                });
            } else if let Some(entry) = self.async_direct_host_native_functions.get(&id) {
                let alias = alias.to_owned();
                let effects = entry.desc.effects;
                let capabilities = self.capabilities;
                let requests = Arc::clone(&entry.requests);
                let function = Arc::clone(&entry.function);
                vm.register_conditional_host_native_with_id(id, move |args, _host, _budget| {
                    check_capabilities(&alias, &effects, capabilities)?;
                    Ok(ConditionalHostNativeOutcome::Async {
                        function: ConditionalAsyncNativeFunction::DirectHostFunction {
                            function: Arc::clone(&function),
                            requests: requests(args)?,
                        },
                        args: args.to_vec(),
                        diagnostic_name: alias.clone(),
                    })
                });
            } else if let Some(entry) = self.scoped_host_native_functions.get(&id) {
                let alias = alias.to_owned();
                let effects = entry.desc.effects;
                let capabilities = self.capabilities;
                let requests = Arc::clone(&entry.requests);
                let function = Arc::clone(&entry.function);
                vm.register_host_native_with_id(id, move |args, host| {
                    check_capabilities(&alias, &effects, capabilities)?;
                    let requests = requests(args)?;
                    let mut invocation_result = None;
                    let mut envelope = ScopedHostEnvelope::Direct;
                    let retained =
                        host.adapter
                            .with_scoped_host_return(&requests, &mut |leases| match function(
                                leases,
                                args.to_vec(),
                            ) {
                                Ok(ScopedHostNativeOutcome::Direct(returned)) => Ok(Some(
                                    vela_host::adapter::ScopedHostReturns::Single(returned),
                                )),
                                Ok(ScopedHostNativeOutcome::OptionSome(returned)) => {
                                    envelope = ScopedHostEnvelope::OptionSome;
                                    Ok(Some(vela_host::adapter::ScopedHostReturns::Single(
                                        returned,
                                    )))
                                }
                                Ok(ScopedHostNativeOutcome::ResultOk(returned)) => {
                                    envelope = ScopedHostEnvelope::ResultOk;
                                    Ok(Some(vela_host::adapter::ScopedHostReturns::Single(
                                        returned,
                                    )))
                                }
                                Ok(ScopedHostNativeOutcome::Tuple(returned)) => {
                                    envelope = ScopedHostEnvelope::Tuple;
                                    Ok(Some(vela_host::adapter::ScopedHostReturns::Group(returned)))
                                }
                                Ok(ScopedHostNativeOutcome::OptionSomeTuple(returned)) => {
                                    envelope = ScopedHostEnvelope::OptionSomeTuple;
                                    Ok(Some(vela_host::adapter::ScopedHostReturns::Group(returned)))
                                }
                                Ok(ScopedHostNativeOutcome::ResultOkTuple(returned)) => {
                                    envelope = ScopedHostEnvelope::ResultOkTuple;
                                    Ok(Some(vela_host::adapter::ScopedHostReturns::Group(returned)))
                                }
                                Ok(ScopedHostNativeOutcome::Value(value)) => {
                                    invocation_result = Some(Ok(value));
                                    Ok(None)
                                }
                                Err(error) => {
                                    invocation_result = Some(Err(error));
                                    Ok(None)
                                }
                            })?;
                    match retained {
                        Some(roots) => Ok(envelope.wrap(roots)),
                        None => invocation_result.expect("missing scoped host invocation result"),
                    }
                });
            } else if let Some(entry) = self.async_context_host_native_functions.get(&id) {
                let alias = alias.to_owned();
                let effects = entry.desc.effects;
                let capabilities = self.capabilities;
                let function = Arc::clone(&entry.function);
                let engine = self.clone();
                vm.register_async_host_native_with_id(id, move |args, host, budget| {
                    let capability_error = check_capabilities(&alias, &effects, capabilities).err();
                    let function = Arc::clone(&function);
                    let engine = engine.clone();
                    Box::pin(async move {
                        if let Some(error) = capability_error {
                            return Err(error);
                        }
                        let mut context = crate::context::NativeCallContext::new(
                            &engine,
                            host,
                            budget,
                            None,
                            effects.required_capability_set(),
                        );
                        function(args, &mut context).await
                    })
                });
            } else if let Some(entry) = self.host_native_functions.get(&id) {
                let alias = alias.to_owned();
                let effects = entry.desc.effects;
                let capabilities = self.capabilities;
                let function = Arc::clone(&entry.function);
                vm.register_host_native_with_id(id, move |args, host| {
                    check_capabilities(&alias, &effects, capabilities)?;
                    function(args, host)
                });
            } else if let Some(entry) = self.context_host_native_functions.get(&id) {
                let alias = alias.to_owned();
                let effects = entry.desc.effects;
                let capabilities = self.capabilities;
                let function = Arc::clone(&entry.function);
                let engine = self.clone();
                vm.register_budgeted_host_native_with_id(id, move |args, host, budget| {
                    check_capabilities(&alias, &effects, capabilities)?;
                    let mut context = crate::context::NativeCallContext::new(
                        &engine,
                        host,
                        budget,
                        None,
                        effects.required_capability_set(),
                    );
                    function(args, &mut context)
                });
            }
        }
    }

    fn native_implementation_ids(&self) -> impl Iterator<Item = FunctionId> + '_ {
        self.native_functions
            .keys()
            .copied()
            .chain(self.async_native_functions.keys().copied())
            .chain(self.async_host_native_functions.keys().copied())
            .chain(self.async_direct_host_native_functions.keys().copied())
            .chain(self.scoped_host_native_functions.keys().copied())
            .chain(self.async_context_host_native_functions.keys().copied())
            .chain(self.host_native_functions.keys().copied())
            .chain(self.context_host_native_functions.keys().copied())
            .chain(
                self.standard_natives
                    .then_some(vela_stdlib::STD_FUNCTIONS)
                    .into_iter()
                    .flatten()
                    .map(|spec| spec.id()),
            )
            .chain(
                self.reflection_policy
                    .is_some()
                    .then(vela_stdlib::reflection_native_specs)
                    .into_iter()
                    .flatten()
                    .map(|spec| spec.id()),
            )
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

#[derive(Clone, Copy)]
enum ScopedHostEnvelope {
    Direct,
    OptionSome,
    ResultOk,
    Tuple,
    OptionSomeTuple,
    ResultOkTuple,
}

impl ScopedHostEnvelope {
    fn wrap(self, roots: Vec<vela_host::path::HostRef>) -> OwnedValue {
        match self {
            Self::Direct => OwnedValue::HostRef(single_scoped_root(&roots)),
            Self::OptionSome => {
                let root = single_scoped_root(&roots);
                OwnedValue::enum_variant("Option", "Some", [("0", OwnedValue::HostRef(root))])
            }
            Self::ResultOk => {
                let root = single_scoped_root(&roots);
                OwnedValue::enum_variant("Result", "Ok", [("0", OwnedValue::HostRef(root))])
            }
            Self::Tuple => OwnedValue::tuple(roots.into_iter().map(OwnedValue::HostRef)),
            Self::OptionSomeTuple => OwnedValue::enum_variant(
                "Option",
                "Some",
                [(
                    "0",
                    OwnedValue::tuple(roots.into_iter().map(OwnedValue::HostRef)),
                )],
            ),
            Self::ResultOkTuple => OwnedValue::enum_variant(
                "Result",
                "Ok",
                [(
                    "0",
                    OwnedValue::tuple(roots.into_iter().map(OwnedValue::HostRef)),
                )],
            ),
        }
    }
}

fn single_scoped_root(roots: &[vela_host::path::HostRef]) -> vela_host::path::HostRef {
    let [root] = roots else {
        panic!("single scoped host return must retain exactly one root");
    };
    *root
}

pub(crate) fn check_capabilities(
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

fn check_method_receiver(
    required: ReceiverCapability,
    receiver: &HostPath,
    host: &HostExecution<'_>,
) -> VmResult<()> {
    let available = host.adapter.host_receiver_access(receiver.root);
    let allowed = match required {
        ReceiverCapability::Shared => true,
        ReceiverCapability::Exclusive => available == HostLeaseKind::Exclusive,
        ReceiverCapability::Owned | ReceiverCapability::Construct => false,
    };
    if allowed {
        return Ok(());
    }
    let action = match required {
        ReceiverCapability::Owned => "call owned receiver method",
        ReceiverCapability::Shared => "call shared receiver method",
        ReceiverCapability::Exclusive => "call exclusive receiver method",
        ReceiverCapability::Construct => "call constructor as instance method",
    };
    Err(VmError::new(VmErrorKind::Host(
        HostErrorKind::PermissionDenied {
            path: receiver.clone(),
            action,
        },
    )))
}
