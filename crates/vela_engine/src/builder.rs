use vela_hot_reload::policy::HotReloadPolicy;
use vela_reflect::modules::ModuleDesc;
use vela_reflect::permissions::{ReflectPermissionSet, ReflectPolicy};
use vela_reflect::registry::{TypeDesc, TypeRegistry};
use vela_vm::HostExecution;
use vela_vm::error::VmResult;
use vela_vm::value::Value;

use crate::context::NativeCallContext;
use crate::engine::{Engine, EngineParts};
use crate::error::EngineResult;
use crate::method::{NativeMethodDesc, NativeMethodEntry};
use crate::native::{
    ContextHostNativeFunctionEntry, HostNativeFunctionEntry, NativeFunctionDesc,
    NativeFunctionEntry,
};
use crate::permission::PermissionSet;
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
    permissions: PermissionSet,
    reflection_policy: Option<ReflectPolicy>,
    hot_reload_policy: HotReloadPolicy,
    standard_natives: bool,
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
    pub fn register_host_schema<T: ScriptHostSchema>(self) -> Self {
        self.register_type(T::script_host_type_desc())
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
    pub fn grant_permission(mut self, permission: impl Into<String>) -> Self {
        self.permissions.insert(permission);
        self
    }

    #[must_use]
    pub fn permissions(mut self, permissions: PermissionSet) -> Self {
        self.permissions = permissions;
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
        self.native_functions
            .push(crate::random::controlled_math_random(seed));
        self
    }

    #[must_use]
    pub fn with_context_clock(mut self, now: i64, tick: i64) -> Self {
        self.native_functions
            .extend(crate::clock::context_clock_functions(now, tick));
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
        function: impl Fn(&[Value]) -> VmResult<Value> + Send + Sync + 'static,
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
        function: impl for<'host> Fn(&[Value], &mut HostExecution<'host>) -> VmResult<Value>
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
            &[Value],
            &mut NativeCallContext<'ctx, 'host>,
        ) -> VmResult<Value>
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
            &[Value],
            &mut HostExecution<'host>,
        ) -> VmResult<Value>
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
        validation::validate_types(&types)?;
        validation::validate_modules(&self.modules)?;
        validation::validate_native_functions(
            &self.native_functions,
            &self.host_native_functions,
            &self.context_host_native_functions,
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

        Ok(Engine::new(EngineParts {
            registry,
            native_functions: self.native_functions,
            host_native_functions: self.host_native_functions,
            context_host_native_functions: self.context_host_native_functions,
            native_methods: self.native_methods,
            permissions: self.permissions,
            reflection_policy: self.reflection_policy,
            hot_reload_policy: self.hot_reload_policy,
            standard_natives: self.standard_natives,
        }))
    }
}
