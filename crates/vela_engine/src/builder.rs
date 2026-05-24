use std::collections::BTreeSet;

use vela_reflect::{
    MethodAccess, MethodDesc, MethodEffectSet, ReflectPermissionSet, ReflectPolicy, TypeDesc,
    TypeKey, TypeRegistry,
};
use vela_vm::{HostExecution, Value, VmResult};

use crate::{
    Engine, EngineError, EngineErrorKind, EngineResult, HostNativeFunctionEntry,
    NativeFunctionDesc, NativeFunctionEntry, NativeMethodDesc, NativeMethodEntry, PermissionSet,
};

#[derive(Clone, Default)]
pub struct EngineBuilder {
    types: Vec<TypeDesc>,
    native_functions: Vec<NativeFunctionEntry>,
    host_native_functions: Vec<HostNativeFunctionEntry>,
    native_methods: Vec<NativeMethodEntry>,
    permissions: PermissionSet,
    reflection_policy: Option<ReflectPolicy>,
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
    pub fn register_native_method_fn(
        mut self,
        desc: NativeMethodDesc,
        function: impl for<'host> Fn(
            &vela_host::HostPath,
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

    pub fn build(self) -> EngineResult<Engine> {
        let mut types = self.types;
        inject_native_method_metadata(&mut types, &self.native_methods)?;
        validate_types(&types)?;
        validate_native_functions(&self.native_functions, &self.host_native_functions)?;

        let mut registry = TypeRegistry::new();
        for desc in types {
            registry.register(desc);
        }

        Ok(Engine::new(
            registry,
            self.native_functions,
            self.host_native_functions,
            self.native_methods,
            self.permissions,
            self.reflection_policy,
        ))
    }
}

fn inject_native_method_metadata(
    types: &mut [TypeDesc],
    native_methods: &[NativeMethodEntry],
) -> EngineResult<()> {
    for entry in native_methods {
        let owner = find_type_mut(types, &entry.desc.owner).ok_or_else(|| {
            EngineError::new(EngineErrorKind::UnknownNativeMethodOwner {
                name: entry.desc.owner.name.clone(),
            })
        })?;
        let mut method = MethodDesc::new(entry.desc.id, entry.desc.name.clone())
            .effects(reflect_effects(&entry.desc.effects))
            .access(reflect_access(&entry.desc.access));
        if let Some(docs) = &entry.desc.docs {
            method = method.docs(docs.clone());
        }
        owner.methods.push(method);
    }
    Ok(())
}

fn reflect_effects(effects: &crate::EffectSet) -> MethodEffectSet {
    MethodEffectSet {
        reads_host: effects.reads_host,
        writes_host: effects.writes_host,
        emits_events: effects.emits_events,
    }
}

fn reflect_access(access: &crate::FunctionAccess) -> MethodAccess {
    access.required_permissions.iter().fold(
        MethodAccess::new()
            .public(access.public)
            .reflect_callable(access.reflect_callable),
        |access, permission| access.require_permission(permission),
    )
}

fn find_type_mut<'a>(types: &'a mut [TypeDesc], key: &TypeKey) -> Option<&'a mut TypeDesc> {
    types.iter_mut().find(|desc| desc.key == *key)
}

fn validate_types(types: &[TypeDesc]) -> EngineResult<()> {
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

fn validate_native_functions(
    functions: &[NativeFunctionEntry],
    host_functions: &[HostNativeFunctionEntry],
) -> EngineResult<()> {
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();

    for desc in functions
        .iter()
        .map(|entry| &entry.desc)
        .chain(host_functions.iter().map(|entry| &entry.desc))
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
