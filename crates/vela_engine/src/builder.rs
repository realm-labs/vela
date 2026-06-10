use vela_hot_reload::policy::HotReloadPolicy;
use vela_reflect::modules::ModuleDesc;
use vela_reflect::permissions::{ReflectPermissionSet, ReflectPolicy};
use vela_reflect::registry::{TypeDesc, TypeRegistry};
use vela_vm::HostExecution;
use vela_vm::error::VmResult;
use vela_vm::owned_value::OwnedValue;

use crate::compiler_registry::definition_registry_from_reflect;
use crate::context::NativeCallContext;
use crate::engine::{Engine, EngineParts};
use crate::error::{EngineError, EngineErrorKind, EngineResult};
use crate::host_type::HostTypeSpec;
use crate::method::{NativeMethodDesc, NativeMethodEntry};
use crate::native::{
    ContextHostNativeFunctionEntry, HostNativeFunctionEntry, NativeFunctionDesc,
    NativeFunctionEntry,
};
use crate::permission::{Capability, CapabilitySet, ExecutionProfile};
use crate::schema::{ScriptHostMethodMetadata, ScriptHostSchema, ScriptReflectSchema};
use crate::typed::{
    TypedContextHostNativeFunction, TypedHostNativeFunction, TypedNativeFunction,
    TypedNativeMethodFunction,
};
use crate::{metadata, validation};

#[derive(Clone, Default)]
pub struct EngineBuilder {
    types: Vec<TypeDesc>,
    modules: Vec<ModuleDesc>,
    native_functions: Vec<NativeFunctionEntry>,
    host_native_functions: Vec<HostNativeFunctionEntry>,
    context_host_native_functions: Vec<ContextHostNativeFunctionEntry>,
    host_method_metadata: Vec<NativeMethodDesc>,
    native_methods: Vec<NativeMethodEntry>,
    capabilities: CapabilitySet,
    reflection_policy: Option<ReflectPolicy>,
    hot_reload_policy: HotReloadPolicy,
    standard_natives: bool,
    time_clock: bool,
    controlled_random: bool,
    stdio: bool,
    fs_io: bool,
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
    pub fn register_module(mut self, desc: ModuleDesc) -> Self {
        self.modules.push(desc);
        self
    }

    #[must_use]
    pub fn register_host_type<T: ScriptHostSchema>(self) -> Self {
        self.register_type(T::script_host_type_desc())
    }

    #[must_use]
    pub fn register_host_type_spec(mut self, spec: impl Into<HostTypeSpec>) -> Self {
        let (type_desc, method_metadata, native_methods) = spec.into().into_parts();
        self.types.push(type_desc);
        self.host_method_metadata.extend(method_metadata);
        self.native_methods.extend(native_methods);
        self
    }

    #[must_use]
    pub fn register_script_host<T>(self) -> Self
    where
        T: ScriptHostSchema + ScriptHostMethodMetadata,
    {
        self.register_host_type::<T>().register_host_methods::<T>()
    }

    #[must_use]
    pub fn register_reflect_schema<T: ScriptReflectSchema>(self) -> Self {
        self.register_type(T::script_reflect_type_desc())
    }

    #[must_use]
    pub fn register_host_method_desc(mut self, desc: NativeMethodDesc) -> Self {
        self.host_method_metadata.push(desc);
        self
    }

    #[must_use]
    pub fn register_host_method_metadata<T: ScriptHostMethodMetadata>(mut self) -> Self {
        self.host_method_metadata
            .extend(T::script_host_method_descs());
        self
    }

    #[must_use]
    pub fn register_host_methods<T: ScriptHostMethodMetadata>(self) -> Self {
        T::register_script_host_methods(self)
    }

    #[must_use]
    pub const fn capability(mut self, capability: Capability) -> Self {
        self.capabilities = self.capabilities.with(capability);
        self
    }

    #[must_use]
    pub const fn capabilities(mut self, capabilities: CapabilitySet) -> Self {
        self.capabilities = capabilities;
        self
    }

    #[must_use]
    pub const fn execution_profile(mut self, profile: ExecutionProfile) -> Self {
        self.capabilities = profile.capabilities();
        self
    }

    #[must_use]
    pub fn reflection_permissions(mut self, permissions: ReflectPermissionSet) -> Self {
        let policy = self
            .reflection_policy
            .take()
            .unwrap_or_default()
            .with_permissions(permissions);
        self.reflection_policy = Some(policy);
        self
    }

    #[must_use]
    pub fn reflection_lookup_budget(mut self, limit: u64) -> Self {
        let policy = self
            .reflection_policy
            .take()
            .unwrap_or_default()
            .with_lookup_limit(limit);
        self.reflection_policy = Some(policy);
        self
    }

    #[must_use]
    pub fn reflection_policy(mut self, policy: ReflectPolicy) -> Self {
        self.reflection_policy = Some(policy);
        self
    }

    #[must_use]
    pub fn hot_reload_policy(mut self, policy: HotReloadPolicy) -> Self {
        self.hot_reload_policy = policy;
        self
    }

    #[must_use]
    pub fn with_controlled_random(mut self, seed: u64) -> Self {
        self.controlled_random = true;
        self.native_functions
            .push(crate::random::controlled_math_random(seed));
        self
    }

    #[must_use]
    pub fn with_time_clock(mut self, now: i64, tick: i64) -> Self {
        self.time_clock = true;
        self.native_functions
            .extend(crate::clock::time_clock_functions(now, tick));
        self
    }

    #[must_use]
    pub fn with_stdio(mut self) -> Self {
        self.stdio = true;
        self.native_functions.extend(crate::io::stdio_functions());
        self
    }

    #[must_use]
    pub fn with_fs_io(mut self, root: impl Into<std::path::PathBuf>) -> Self {
        self.fs_io = true;
        self.native_functions
            .extend(crate::io::fs_functions(crate::io::FsSandbox::new(root)));
        self
    }

    #[must_use]
    pub fn with_context_host_schema(self) -> Self {
        self.register_type(crate::context_schema::context_host_type_desc())
    }

    #[must_use]
    pub const fn with_standard_natives(mut self) -> Self {
        self.standard_natives = true;
        self
    }

    #[must_use]
    pub fn register_native_fn(
        mut self,
        desc: NativeFunctionDesc,
        function: impl Fn(&[OwnedValue]) -> VmResult<OwnedValue> + Send + Sync + 'static,
    ) -> Self {
        self.native_functions
            .push(NativeFunctionEntry::new(desc, function));
        self
    }

    #[must_use]
    pub fn register_typed_native_fn<Args, F>(self, desc: NativeFunctionDesc, function: F) -> Self
    where
        F: TypedNativeFunction<Args>,
    {
        self.register_native_fn(desc, move |args| function.call(args))
    }

    #[must_use]
    pub fn register_host_native_fn(
        mut self,
        desc: NativeFunctionDesc,
        function: impl for<'host> Fn(&[OwnedValue], &mut HostExecution<'host>) -> VmResult<OwnedValue>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.host_native_functions
            .push(HostNativeFunctionEntry::new(desc, function));
        self
    }

    #[must_use]
    pub fn register_typed_host_native_fn<Args, F>(
        self,
        desc: NativeFunctionDesc,
        function: F,
    ) -> Self
    where
        F: TypedHostNativeFunction<Args>,
    {
        self.register_host_native_fn(desc, move |args, host| function.call_host(args, host))
    }

    #[must_use]
    pub fn register_context_host_native_fn(
        mut self,
        desc: NativeFunctionDesc,
        function: impl for<'ctx, 'host> Fn(
            &[OwnedValue],
            &mut NativeCallContext<'ctx, 'host>,
        ) -> VmResult<OwnedValue>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.context_host_native_functions
            .push(ContextHostNativeFunctionEntry::new(desc, function));
        self
    }

    #[must_use]
    pub fn register_typed_context_host_native_fn<Args, F>(
        self,
        desc: NativeFunctionDesc,
        function: F,
    ) -> Self
    where
        F: TypedContextHostNativeFunction<Args>,
    {
        self.register_context_host_native_fn(desc, move |args, ctx| {
            function.call_context(args, ctx)
        })
    }

    #[must_use]
    pub fn register_native_method_fn(
        mut self,
        desc: NativeMethodDesc,
        function: impl for<'host> Fn(
            &vela_host::path::HostPath,
            &[OwnedValue],
            &mut HostExecution<'host>,
        ) -> VmResult<OwnedValue>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.native_methods
            .push(NativeMethodEntry::new(desc, function));
        self
    }

    #[must_use]
    pub fn register_typed_native_method_fn<Args, F>(
        self,
        desc: NativeMethodDesc,
        function: F,
    ) -> Self
    where
        F: TypedNativeMethodFunction<Args>,
    {
        self.register_native_method_fn(desc, move |receiver, args, host| {
            function.call_method(receiver, args, host)
        })
    }

    pub fn build(self) -> EngineResult<Engine> {
        let mut types = self.types;
        metadata::inject_host_method_metadata(
            &mut types,
            &self.host_method_metadata,
            &self.native_methods,
        )?;
        validation::validate_native_method_type_hints(
            &self.host_method_metadata,
            &self.native_methods,
            &types,
            self.standard_natives,
        )?;
        validation::validate_types(&types, self.standard_natives)?;
        let module_options = validation::ModuleValidationOptions::default()
            .include_standard_modules(self.standard_natives)
            .include_time_module(self.time_clock)
            .include_math_module(self.controlled_random)
            .include_io_module(self.stdio)
            .include_fs_module(self.fs_io);
        validation::validate_modules(&self.modules, module_options)?;
        validation::validate_native_functions(
            &self.native_functions,
            &self.host_native_functions,
            &self.context_host_native_functions,
            &types,
            self.standard_natives,
        )?;

        let mut registry = TypeRegistry::new();
        for desc in types {
            registry.register(desc);
        }
        for module in self.modules {
            registry.register_module(module);
        }
        if self.standard_natives {
            metadata::inject_standard_native_metadata(&mut registry);
        }
        metadata::inject_native_function_metadata(
            &mut registry,
            &self.native_functions,
            &self.host_native_functions,
            &self.context_host_native_functions,
        );
        let definition_registry =
            definition_registry_from_reflect(&registry, self.reflection_policy.is_some()).map_err(
                |error| {
                    EngineError::new(EngineErrorKind::DefinitionRegistry {
                        message: error.to_string(),
                    })
                },
            )?;

        Ok(Engine::new(EngineParts {
            registry,
            definition_registry,
            native_functions: self.native_functions,
            host_native_functions: self.host_native_functions,
            context_host_native_functions: self.context_host_native_functions,
            native_methods: self.native_methods,
            capabilities: self.capabilities,
            reflection_policy: self.reflection_policy,
            hot_reload_policy: self.hot_reload_policy,
            standard_natives: self.standard_natives,
        }))
    }
}
