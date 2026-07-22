use std::any::TypeId as RustTypeId;
use std::collections::HashMap;

use vela_common::TypeAbiFingerprint;
use vela_hot_reload::policy::HotReloadPolicy;
use vela_reflect::modules::ModuleDesc;
use vela_reflect::permissions::{ReflectPermissionSet, ReflectPolicy};
use vela_reflect::registry::{TypeDesc, TypeKey, TypeRegistry};
use vela_vm::HostExecution;
use vela_vm::error::VmResult;
use vela_vm::owned_value::OwnedValue;

use crate::compiler_registry::{EngineFunctionEntries, definition_registry_from_engine_parts};
use crate::context::NativeCallContext;
use crate::engine::{Engine, EngineParts};
use crate::error::{EngineError, EngineErrorKind, EngineResult};
use crate::host_type::HostTypeSpec;
use crate::method::{AsyncNativeMethodEntry, NativeMethodDesc, NativeMethodEntry};
use crate::native::{
    AsyncContextHostNativeFunctionEntry, AsyncDirectHostNativeFunctionEntry,
    AsyncHostNativeFunctionEntry, AsyncNativeFunctionEntry, ContextHostNativeFunctionEntry,
    HostNativeFunctionEntry, NativeCallFuture, NativeFunctionDesc, NativeFunctionEntry,
    ScopedHostNativeFunctionEntry,
};
use crate::permission::{Capability, CapabilitySet, ExecutionProfile};
use crate::schema::{ScriptHostMethodMetadata, ScriptHostSchema, ScriptReflectSchema};
use crate::type_binding::{TypeBinding, TypeBindingRegistration, TypeBindingRegistry};
use crate::typed::{
    TypedAsyncContextHostNativeFunction, TypedAsyncHostNativeFunction, TypedAsyncNativeFunction,
    TypedAsyncNativeMethodFunction, TypedContextHostNativeFunction, TypedHostNativeFunction,
    TypedNativeFunction, TypedNativeMethodFunction,
};
use crate::{metadata, validation};

#[derive(Clone, Default)]
pub struct EngineBuilder {
    types: Vec<TypeDesc>,
    type_bindings: Vec<TypeBindingRegistration>,
    rust_type_bindings: HashMap<RustTypeId, (TypeKey, TypeAbiFingerprint)>,
    modules: Vec<ModuleDesc>,
    native_functions: Vec<NativeFunctionEntry>,
    async_native_functions: Vec<AsyncNativeFunctionEntry>,
    async_host_native_functions: Vec<AsyncHostNativeFunctionEntry>,
    async_direct_host_native_functions: Vec<AsyncDirectHostNativeFunctionEntry>,
    scoped_host_native_functions: Vec<ScopedHostNativeFunctionEntry>,
    async_context_host_native_functions: Vec<AsyncContextHostNativeFunctionEntry>,
    host_native_functions: Vec<HostNativeFunctionEntry>,
    context_host_native_functions: Vec<ContextHostNativeFunctionEntry>,
    host_method_metadata: Vec<NativeMethodDesc>,
    native_methods: Vec<NativeMethodEntry>,
    async_native_methods: Vec<AsyncNativeMethodEntry>,
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

    /// Registers one Rust type through the unified Rust/Vela binding model.
    #[must_use]
    pub fn register_rust_type<T: 'static>(self, binding: TypeBinding<T>) -> Self {
        self.push_rust_type::<T>(binding)
    }

    /// Registers the complete owned-Value dependency closure rooted at `T`.
    ///
    /// Generated service bundles use this entrypoint so a signature such as
    /// `BTreeMap<String, Vec<Dto>>` installs every concrete nested binding
    /// without application-authored registration calls.
    #[must_use]
    pub fn register_rust_value_closure<T>(self) -> Self
    where
        T: crate::type_registration::RustValueType,
    {
        T::register_value_type_closure(self)
    }

    /// Installs one generated member of a recursive Value type closure.
    ///
    /// This remains public only because derive and service macros expand in
    /// downstream crates. Exact prior registrations are retained; a different
    /// binding for the same Rust type is deliberately kept as a duplicate so
    /// Engine sealing reports the conflict instead of silently choosing one.
    #[doc(hidden)]
    #[must_use]
    pub fn register_generated_rust_value<T: 'static>(self, binding: TypeBinding<T>) -> Self {
        let rust_type_id = RustTypeId::of::<T>();
        let identity = (binding.type_desc().key.clone(), binding.abi_fingerprint());
        if self.rust_type_bindings.get(&rust_type_id) == Some(&identity) {
            return self;
        }
        self.push_rust_type::<T>(binding)
    }

    fn push_rust_type<T: 'static>(mut self, binding: TypeBinding<T>) -> Self {
        let binding_identity = (binding.type_desc().key.clone(), binding.abi_fingerprint());
        let (
            mut registration,
            type_desc,
            method_metadata,
            native_methods,
            async_native_methods,
            constructors,
        ) = binding.into_parts();
        registration.bind_rust_type::<T>();
        self.rust_type_bindings
            .insert(RustTypeId::of::<T>(), binding_identity);
        self.type_bindings.push(registration);
        self.types.push(type_desc);
        self.host_method_metadata.extend(method_metadata);
        self.native_methods.extend(native_methods);
        self.async_native_methods.extend(async_native_methods);
        self.host_native_functions.extend(constructors);
        self
    }

    #[must_use]
    pub fn register_module(mut self, desc: ModuleDesc) -> Self {
        self.modules.push(desc);
        self
    }

    /// Installs one explicitly generated Rust export bundle.
    #[must_use]
    pub fn register_exports(self, bundle: crate::interop::ExportBundle) -> Self {
        bundle.install(self)
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
        self.register_rust_type::<T>(T::script_host_type_binding())
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
    pub fn register_async_fn(
        mut self,
        desc: NativeFunctionDesc,
        function: impl for<'call> Fn(&'call [OwnedValue]) -> NativeCallFuture<'call>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.async_native_functions
            .push(AsyncNativeFunctionEntry::new(desc, function));
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
    pub fn register_typed_async_fn<Args, F>(self, desc: NativeFunctionDesc, function: F) -> Self
    where
        F: TypedAsyncNativeFunction<Args>,
    {
        self.register_async_fn(desc, move |args| function.call_async(args))
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
    pub fn register_async_host_fn(
        mut self,
        desc: NativeFunctionDesc,
        function: impl for<'call, 'host> Fn(
            &'call [OwnedValue],
            &'call mut HostExecution<'host>,
        ) -> NativeCallFuture<'call>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.async_host_native_functions
            .push(AsyncHostNativeFunctionEntry::new(desc, function));
        self
    }

    /// Registers an async native whose complete host borrow set is acquired
    /// atomically before its future is created.
    #[doc(hidden)]
    #[must_use]
    pub fn register_async_direct_host_fn(
        mut self,
        desc: NativeFunctionDesc,
        requests: impl Fn(&[OwnedValue]) -> VmResult<vela_host::lease::HostLeaseRequestSet>
        + Send
        + Sync
        + 'static,
        function: impl for<'invoke, 'lease> Fn(
            &'invoke mut [vela_host::lease::ErasedHostLease<'lease>],
            Vec<OwnedValue>,
        ) -> NativeCallFuture<'invoke>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.async_direct_host_native_functions
            .push(AsyncDirectHostNativeFunctionEntry::new(
                desc, requests, function,
            ));
        self
    }

    /// Registers a synchronous native that returns an owner-frozen host child.
    #[doc(hidden)]
    #[must_use]
    pub fn register_scoped_host_fn(
        mut self,
        desc: NativeFunctionDesc,
        requests: impl Fn(&[OwnedValue]) -> VmResult<vela_host::lease::HostLeaseRequestSet>
        + Send
        + Sync
        + 'static,
        function: impl for<'host> Fn(
            &mut [vela_host::lease::ErasedHostLease<'host>],
            Vec<OwnedValue>,
        ) -> VmResult<crate::native::ScopedHostNativeOutcome<'host>>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.scoped_host_native_functions
            .push(ScopedHostNativeFunctionEntry::new(desc, requests, function));
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
    pub fn register_typed_async_host_fn<Args, F>(
        self,
        desc: NativeFunctionDesc,
        function: F,
    ) -> Self
    where
        F: TypedAsyncHostNativeFunction<Args>,
    {
        self.register_async_host_fn(desc, move |args, host| function.call_async_host(args, host))
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
    pub fn register_async_context_fn(
        mut self,
        desc: NativeFunctionDesc,
        function: impl for<'call, 'host> Fn(
            &'call [OwnedValue],
            &'call mut NativeCallContext<'call, 'host>,
        ) -> NativeCallFuture<'call>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.async_context_host_native_functions
            .push(AsyncContextHostNativeFunctionEntry::new(desc, function));
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
    pub fn register_typed_async_context_fn<Args, F>(
        self,
        desc: NativeFunctionDesc,
        function: F,
    ) -> Self
    where
        F: TypedAsyncContextHostNativeFunction<Args>,
    {
        self.register_async_context_fn(desc, move |args, context| {
            function.call_async_context(args, context)
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
    pub fn register_async_method_fn(
        mut self,
        desc: NativeMethodDesc,
        function: impl for<'call, 'host> Fn(
            &'call vela_host::path::HostPath,
            &'call [OwnedValue],
            &'call mut HostExecution<'host>,
        ) -> NativeCallFuture<'call>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.async_native_methods
            .push(AsyncNativeMethodEntry::new(desc, function));
        self
    }

    #[doc(hidden)]
    #[must_use]
    pub fn register_async_direct_method_fn(
        mut self,
        desc: NativeMethodDesc,
        lease_kind: vela_host::lease::HostLeaseKind,
        function: impl for<'host> Fn(
            vela_host::path::HostRef,
            vela_host::lease::ErasedHostLease<'host>,
            Vec<OwnedValue>,
        ) -> NativeCallFuture<'host>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.async_native_methods
            .push(AsyncNativeMethodEntry::new_direct(
                desc, lease_kind, function,
            ));
        self
    }

    #[doc(hidden)]
    #[must_use]
    pub fn register_async_context_direct_method_fn(
        mut self,
        desc: NativeMethodDesc,
        lease_kind: vela_host::lease::HostLeaseKind,
        param_leases: Vec<(usize, vela_host::lease::HostLeaseKind)>,
        function: impl for<'invoke, 'lease> Fn(
            vela_host::path::HostRef,
            &'invoke mut [vela_host::lease::ErasedHostLease<'lease>],
            Vec<OwnedValue>,
            &'invoke mut crate::context::NativeCallContext<'invoke, 'invoke>,
        ) -> NativeCallFuture<'invoke>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.async_native_methods
            .push(AsyncNativeMethodEntry::new_direct_context(
                desc,
                lease_kind,
                param_leases,
                function,
            ));
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

    #[must_use]
    pub fn register_typed_async_method_fn<Args, F>(
        self,
        desc: NativeMethodDesc,
        function: F,
    ) -> Self
    where
        F: TypedAsyncNativeMethodFunction<Args>,
    {
        self.register_async_method_fn(desc, move |receiver, args, host| {
            function.call_async_method(receiver, args, host)
        })
    }

    pub fn build(mut self) -> EngineResult<Engine> {
        let release_id = vela_def::host_release_function_id();
        self.host_native_functions
            .push(HostNativeFunctionEntry::new(
                NativeFunctionDesc::new("host::release", release_id)
                    .param("value", crate::native::TypeHint::Any)
                    .returns(crate::native::TypeHint::unit())
                    .effects(crate::native::EffectSet::pure())
                    .access(crate::native::FunctionAccess::public())
                    .docs("Releases one call-tree-scoped borrowed host value."),
                |args, host| {
                    let [OwnedValue::HostRef(root)] = args else {
                        return Err(vela_vm::error::VmError::new(
                            vela_vm::error::VmErrorKind::TypeMismatch {
                                operation: "host::release scoped host value",
                            },
                        ));
                    };
                    host.adapter.release_scoped_host(*root)?;
                    Ok(OwnedValue::Unit)
                },
            ));
        let mut types = self.types;
        if self.stdio || self.fs_io {
            types.push(crate::io::io_error_type_desc());
        }
        metadata::inject_host_method_metadata(
            &mut types,
            &self.host_method_metadata,
            &self.native_methods,
            &self.async_native_methods,
        )?;
        validation::validate_native_method_type_hints(
            &self.host_method_metadata,
            &self.native_methods,
            &self.async_native_methods,
            &types,
            self.standard_natives,
        )?;
        validation::validate_types(&types, self.standard_natives)?;
        let type_bindings = TypeBindingRegistry::seal(self.type_bindings, &types)?;
        let module_options = validation::ModuleValidationOptions::default()
            .include_standard_modules(self.standard_natives)
            .include_time_module(self.time_clock)
            .include_math_module(self.controlled_random)
            .include_io_module(self.stdio)
            .include_fs_module(self.fs_io);
        validation::validate_modules(&self.modules, module_options)?;
        validation::validate_native_functions(
            EngineFunctionEntries {
                native: &self.native_functions,
                async_native: &self.async_native_functions,
                async_host: &self.async_host_native_functions,
                async_direct_host: &self.async_direct_host_native_functions,
                scoped_host: &self.scoped_host_native_functions,
                async_context_host: &self.async_context_host_native_functions,
                host: &self.host_native_functions,
                context_host: &self.context_host_native_functions,
            },
            &types,
            &type_bindings,
            self.standard_natives,
        )?;

        let definition_registry = definition_registry_from_engine_parts(
            &types,
            &type_bindings,
            EngineFunctionEntries {
                native: &self.native_functions,
                async_native: &self.async_native_functions,
                async_host: &self.async_host_native_functions,
                async_direct_host: &self.async_direct_host_native_functions,
                scoped_host: &self.scoped_host_native_functions,
                async_context_host: &self.async_context_host_native_functions,
                host: &self.host_native_functions,
                context_host: &self.context_host_native_functions,
            },
            self.reflection_policy.is_some(),
            self.standard_natives,
        )
        .map_err(|error| {
            EngineError::new(EngineErrorKind::DefinitionRegistry {
                message: error.to_string(),
            })
        })?;

        let mut registry = TypeRegistry::new();
        for desc in types {
            registry.register(desc);
        }
        registry.install_type_bindings(type_bindings.iter().cloned(), type_bindings.checksum());
        for module in self.modules {
            registry.register_module(module);
        }
        if self.standard_natives {
            metadata::inject_standard_native_metadata(&mut registry);
        }
        metadata::inject_native_function_metadata(
            &mut registry,
            &self.native_functions,
            &self.async_native_functions,
            &self.async_host_native_functions,
            &self.async_direct_host_native_functions,
            &self.scoped_host_native_functions,
            &self.async_context_host_native_functions,
            &self.host_native_functions,
            &self.context_host_native_functions,
        );

        Ok(Engine::new(EngineParts {
            registry,
            type_bindings,
            definition_registry,
            native_functions: self.native_functions,
            async_native_functions: self.async_native_functions,
            async_host_native_functions: self.async_host_native_functions,
            async_direct_host_native_functions: self.async_direct_host_native_functions,
            scoped_host_native_functions: self.scoped_host_native_functions,
            async_context_host_native_functions: self.async_context_host_native_functions,
            host_native_functions: self.host_native_functions,
            context_host_native_functions: self.context_host_native_functions,
            native_methods: self.native_methods,
            async_native_methods: self.async_native_methods,
            capabilities: self.capabilities,
            reflection_policy: self.reflection_policy,
            hot_reload_policy: self.hot_reload_policy,
            standard_natives: self.standard_natives,
        }))
    }
}
